use std::{
    fs,
    path::Path,
    time::Instant,
};

use image::DynamicImage;
use ort::{
    session::{Session, builder::GraphOptimizationLevel},
};
use serde::Deserialize;

use crate::inference::{
    InferenceResult, decoder_loop::autoregressive_decode, inference_error,
    postprocessor::LatexPostProcessor, preprocessor::preprocess_dynamic_image,
    tokenizer::LatexTokenizer,
};

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct ModelMetadata {
    pub id: String,
    pub task: String,
    pub image_size: (u32, u32),
    pub max_length: usize,
    pub bos_token_id: i64,
    pub eos_token_id: i64,
    pub pad_token_id: i64,
}

#[derive(Debug, Clone)]
pub struct Candidate {
    pub latex: String,
    pub score: f32,
    pub warnings: Vec<String>,
    pub token_probs: Vec<f32>,
    pub tokens: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct RecognitionResult {
    pub primary: Candidate,
    pub alternatives: Vec<Candidate>,
    pub latency_ms: u64,
    pub provider: String,
    pub model_id: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct InferenceJob {
    pub image: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub top_k: usize,
    pub timeout_ms: u64,
}

#[allow(dead_code)]
pub trait Recognizer {
    fn metadata(&self) -> &ModelMetadata;
    fn warmup(&mut self) -> InferenceResult<()>;
    fn infer(&mut self, job: InferenceJob) -> InferenceResult<RecognitionResult>;
    fn infer_batch(&mut self, jobs: Vec<InferenceJob>) -> InferenceResult<Vec<RecognitionResult>>;
}

#[derive(Debug, Deserialize)]
struct ModelManifest {
    id: String,
    task: String,
    inputs: ManifestInputs,
    files: ManifestFiles,
    generation: ManifestGeneration,
}

#[derive(Debug, Deserialize)]
struct ManifestInputs {
    image_size: [u32; 2],
}

#[derive(Debug, Deserialize)]
struct ManifestFiles {
    encoder: String,
    decoder: String,
    tokenizer: String,
}

#[derive(Debug, Deserialize)]
struct ManifestGeneration {
    bos_token_id: i64,
    eos_token_id: i64,
    pad_token_id: i64,
    max_length: usize,
}

pub struct TrOCRRecognizer {
    metadata: ModelMetadata,
    encoder_session: Session,
    decoder_session: Session,
    tokenizer: LatexTokenizer,
    postprocessor: LatexPostProcessor,
}

impl TrOCRRecognizer {
    pub fn from_model_dir(model_dir: &Path) -> InferenceResult<Self> {
        let manifest = load_manifest(model_dir)?;
        let encoder_path = model_dir.join(&manifest.files.encoder);
        let decoder_path = model_dir.join(&manifest.files.decoder);
        let tokenizer_path = model_dir.join(&manifest.files.tokenizer);

        let encoder_session = build_session(&encoder_path)?;
        let decoder_session = build_session(&decoder_path)?;
        let tokenizer = LatexTokenizer::from_file(&tokenizer_path)?;
        let postprocessor = LatexPostProcessor::new()?;

        let metadata = ModelMetadata {
            id: manifest.id,
            task: manifest.task,
            image_size: (manifest.inputs.image_size[0], manifest.inputs.image_size[1]),
            max_length: manifest.generation.max_length,
            bos_token_id: manifest.generation.bos_token_id,
            eos_token_id: manifest.generation.eos_token_id,
            pad_token_id: manifest.generation.pad_token_id,
        };

        Ok(Self {
            metadata,
            encoder_session,
            decoder_session,
            tokenizer,
            postprocessor,
        })
    }

    pub fn infer_image(&mut self, image_path: &Path) -> InferenceResult<RecognitionResult> {
        let image = image::open(image_path)?;
        self.infer_dynamic_image_with_options(&image, 1)
    }

    pub fn infer_image_bytes(
        &mut self,
        image_bytes: &[u8],
        num_beams: usize,
    ) -> InferenceResult<RecognitionResult> {
        let image = image::load_from_memory(image_bytes)?;
        self.infer_dynamic_image_with_options(&image, num_beams)
    }

    fn infer_dynamic_image(&mut self, image: &DynamicImage) -> InferenceResult<RecognitionResult> {
        self.infer_dynamic_image_with_options(image, 1)
    }

    fn infer_dynamic_image_with_options(
        &mut self,
        image: &DynamicImage,
        num_beams: usize,
    ) -> InferenceResult<RecognitionResult> {
        let started = Instant::now();
        let pixel_values = preprocess_dynamic_image(image)?;
        let raw_candidates = autoregressive_decode(
            &mut self.encoder_session,
            &mut self.decoder_session,
            &pixel_values,
            &self.metadata,
            self.tokenizer.inner(),
            num_beams,
        )?;

        let mut candidates = raw_candidates
            .into_iter()
            .map(|candidate| Candidate {
                latex: self.postprocessor.process(&candidate.latex),
                score: candidate.score,
                warnings: candidate.warnings,
                token_probs: candidate.token_probs,
                tokens: candidate.tokens,
            })
            .collect::<Vec<_>>();

        if candidates.is_empty() {
            return Err(inference_error("decoder did not produce any candidates"));
        }

        let primary = candidates.remove(0);
        Ok(RecognitionResult {
            primary,
            alternatives: candidates,
            latency_ms: started.elapsed().as_millis() as u64,
            provider: "cpu".to_string(),
            model_id: self.metadata.id.clone(),
        })
    }
}

impl Recognizer for TrOCRRecognizer {
    fn metadata(&self) -> &ModelMetadata {
        &self.metadata
    }

    fn warmup(&mut self) -> InferenceResult<()> {
        let blank = DynamicImage::new_rgb8(self.metadata.image_size.0, self.metadata.image_size.1);
        let _ = self.infer_dynamic_image(&blank)?;
        Ok(())
    }

    fn infer(&mut self, job: InferenceJob) -> InferenceResult<RecognitionResult> {
        let image = image::RgbImage::from_raw(job.width, job.height, job.image)
            .ok_or_else(|| inference_error("invalid RGB image buffer"))?;
        self.infer_dynamic_image(&DynamicImage::ImageRgb8(image))
    }

    fn infer_batch(&mut self, jobs: Vec<InferenceJob>) -> InferenceResult<Vec<RecognitionResult>> {
        jobs.into_iter().map(|job| self.infer(job)).collect()
    }
}

fn load_manifest(model_dir: &Path) -> InferenceResult<ModelManifest> {
    let manifest_path = model_dir.join("model_manifest.json");
    let manifest_text = fs::read_to_string(&manifest_path)?;
    Ok(serde_json::from_str(&manifest_text)?)
}

fn build_session(model_path: &Path) -> InferenceResult<Session> {
    let builder = Session::builder().map_err(|err| inference_error(err.to_string()))?;
    let builder = builder
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|err| inference_error(err.to_string()))?;
    let builder = builder
        .with_intra_threads(1)
        .map_err(|err| inference_error(err.to_string()))?;
    let mut builder = builder
        .with_parallel_execution(false)
        .map_err(|err| inference_error(err.to_string()))?;
    Ok(builder.commit_from_file(model_path)?)
}
