use std::cmp::Ordering;

use ndarray::Array4;
use ort::{
    inputs,
    session::Session,
    value::{Tensor, TensorRef},
};

use crate::inference::{
    InferenceResult, inference_error,
    recognizer::{Candidate, ModelMetadata},
};

#[derive(Debug, Clone)]
struct BeamState {
    token_ids: Vec<i64>,
    cumulative_log_prob: f32,
    per_token_probs: Vec<f32>,
}

#[derive(Debug, Clone)]
struct BeamExpansion {
    state: BeamState,
    finished: bool,
}

pub fn autoregressive_decode(
    encoder_session: &mut Session,
    decoder_session: &mut Session,
    pixel_values: &Array4<f32>,
    config: &ModelMetadata,
    tokenizer: &tokenizers::Tokenizer,
    num_beams: usize,
) -> InferenceResult<Vec<Candidate>> {
    let encoder_input = TensorRef::from_array_view(pixel_values.view())?;
    let encoder_outputs = encoder_session.run(inputs! {
        "pixel_values" => encoder_input
    })?;
    let encoder_hidden_states = encoder_outputs
        .get("last_hidden_state")
        .ok_or_else(|| inference_error("encoder output `last_hidden_state` not found"))?;

    let num_beams = num_beams.clamp(1, 5);
    let mut active = vec![BeamState {
        token_ids: vec![config.bos_token_id],
        cumulative_log_prob: 0.0,
        per_token_probs: Vec::new(),
    }];
    let mut finished = Vec::new();

    for _ in 0..config.max_length {
        if active.is_empty() {
            break;
        }

        let mut expanded = Vec::new();
        for beam in &active {
            let input_ids = Tensor::from_array((vec![1usize, beam.token_ids.len()], beam.token_ids.clone()))?;
            let decoder_outputs = decoder_session.run(inputs! {
                "input_ids" => input_ids,
                "encoder_hidden_states" => encoder_hidden_states
            })?;
            let logits = decoder_outputs
                .get("logits")
                .ok_or_else(|| inference_error("decoder output `logits` not found"))?;

            let timestep_logits = extract_last_timestep_logits(logits)?;
            for (token_id, probability) in top_k_with_probabilities(timestep_logits, num_beams) {
                let mut next = beam.clone();
                next.token_ids.push(token_id as i64);
                next.per_token_probs.push(probability);
                next.cumulative_log_prob += probability.max(1e-8).ln();
                expanded.push(BeamExpansion {
                    finished: token_id as i64 == config.eos_token_id,
                    state: next,
                });
            }
        }

        if expanded.is_empty() {
            break;
        }

        expanded.sort_by(compare_beam_expansions);
        active.clear();
        for item in expanded {
            if item.finished {
                finished.push(item.state);
            } else {
                active.push(item.state);
            }

            if active.len() >= num_beams {
                break;
            }
        }

        if finished.len() >= num_beams || active.iter().all(|beam| beam.token_ids.len() >= config.max_length) {
            break;
        }
    }

    finished.extend(active);
    finished.sort_by(compare_beams);
    finished.truncate(num_beams);

    if finished.is_empty() {
        return Err(inference_error("decoder did not produce any candidates"));
    }

    finished
        .into_iter()
        .map(|beam| beam_to_candidate(beam, tokenizer, config))
        .collect()
}

fn extract_last_timestep_logits<'a>(logits: &'a ort::value::Value) -> InferenceResult<&'a [f32]> {
    let (shape, data) = logits.try_extract_tensor::<f32>()?;
    if shape.len() != 3 {
        return Err(inference_error(format!(
            "expected decoder logits rank 3, got shape {:?}",
            shape
        )));
    }

    let sequence_len = usize::try_from(shape[1])?;
    let vocab_size = usize::try_from(shape[2])?;
    if sequence_len == 0 || vocab_size == 0 {
        return Err(inference_error(
            "decoder logits have an empty sequence or vocabulary dimension",
        ));
    }

    let timestep_offset = (sequence_len - 1) * vocab_size;
    Ok(&data[timestep_offset..timestep_offset + vocab_size])
}

fn beam_to_candidate(
    beam: BeamState,
    tokenizer: &tokenizers::Tokenizer,
    config: &ModelMetadata,
) -> InferenceResult<Candidate> {
    let visible_token_ids = beam
        .token_ids
        .iter()
        .copied()
        .filter(|id| *id != config.bos_token_id && *id != config.eos_token_id && *id != config.pad_token_id)
        .collect::<Vec<_>>();
    let latex = decode_tokens(tokenizer, &beam.token_ids)?;

    let score = if beam.per_token_probs.is_empty() {
        1.0
    } else {
        let avg_log_prob = beam.cumulative_log_prob / beam.per_token_probs.len() as f32;
        avg_log_prob.exp()
    };

    let token_probs = beam
        .token_ids
        .iter()
        .skip(1)
        .zip(beam.per_token_probs.iter().copied())
        .filter_map(|(token_id, probability)| {
            if *token_id == config.eos_token_id || *token_id == config.pad_token_id {
                None
            } else {
                Some(probability)
            }
        })
        .collect::<Vec<_>>();

    let tokens = visible_token_ids
        .iter()
        .map(|token_id| {
            u32::try_from(*token_id)
                .ok()
                .and_then(|id| tokenizer.id_to_token(id))
                .map(|t| t.replace('\u{0120}', " ").replace('\u{010A}', "\n"))
                .unwrap_or_else(|| token_id.to_string())
        })
        .collect::<Vec<_>>();

    Ok(Candidate {
        latex,
        score,
        warnings: Vec::new(),
        token_probs,
        tokens,
    })
}

fn decode_tokens(tokenizer: &tokenizers::Tokenizer, token_ids: &[i64]) -> InferenceResult<String> {
    let ids = token_ids
        .iter()
        .copied()
        .filter(|id| *id >= 0)
        .map(u32::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(tokenizer.decode(&ids, true)?)
}

fn compare_beams(left: &BeamState, right: &BeamState) -> Ordering {
    right
        .cumulative_log_prob
        .partial_cmp(&left.cumulative_log_prob)
        .unwrap_or(Ordering::Equal)
}

fn compare_beam_expansions(left: &BeamExpansion, right: &BeamExpansion) -> Ordering {
    compare_beams(&left.state, &right.state)
}

fn top_k_with_probabilities(logits: &[f32], k: usize) -> Vec<(usize, f32)> {
    let max_logit = logits
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    let exp_values = logits
        .iter()
        .copied()
        .map(|logit| (logit - max_logit).exp())
        .collect::<Vec<_>>();
    let denom = exp_values.iter().copied().sum::<f32>().max(1e-8);

    let mut candidates = exp_values
        .into_iter()
        .enumerate()
        .map(|(index, value)| (index, value / denom))
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(Ordering::Equal)
    });
    candidates.truncate(k.max(1));
    candidates
}

#[allow(dead_code)]
fn argmax_with_probability(logits: &[f32]) -> (usize, f32) {
    top_k_with_probabilities(logits, 1)
        .into_iter()
        .next()
        .unwrap_or((0, 1.0))
}
