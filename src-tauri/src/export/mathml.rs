use latex2mathml::{DisplayStyle, latex_to_mathml as convert_latex_to_mathml};
use regex::{Captures, Regex};
use std::collections::HashMap;
use std::sync::OnceLock;

use super::{ExportResult, latex::normalize_latex_for_export};

const SUM_SYMBOL: &str = "\u{2211}";
const INFINITY_SYMBOL: &str = "\u{221E}";

fn math_root_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"<math\b([^>]*)>"#).expect("math root regex"))
}

fn display_attr_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"\bdisplay\s*="#).expect("display attr regex"))
}

fn assignment_pair_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r#"<mo>\s*:</mo>\s*<mo>\s*=\s*</mo>"#).expect("assignment regex"))
}

fn infinity_variant_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"<mi>\s*(?:&#x221E;|&#X221E;|∞)\s*</mi>"#).expect("infinity regex")
    })
}

fn legacy_bold_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\\bf\b").expect("legacy bold regex"))
}

fn legacy_rm_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\\rm\b").expect("legacy rm regex"))
}

fn legacy_it_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\\it\b").expect("legacy it regex"))
}

fn xrightarrow_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r#"\\xrightarrow(?:\[[^\]]*\])?\{([^{}]*)\}"#).expect("xrightarrow regex")
    })
}

fn begin_array_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\\begin\{array\}\{[^{}]*\}").expect("begin array regex"))
}

fn ensure_mathml_block(mathml: &str) -> String {
    let Some(captures) = math_root_re().captures(mathml) else {
        return mathml.to_string();
    };
    let Some(matched) = captures.get(0) else {
        return mathml.to_string();
    };
    let attrs = captures.get(1).map(|m| m.as_str()).unwrap_or_default();
    if display_attr_re().is_match(attrs) {
        return mathml.to_string();
    }
    let separator = if attrs.is_empty() || attrs.ends_with(' ') {
        ""
    } else {
        " "
    };
    let replacement = format!(r#"<math{attrs}{separator}display="block">"#);
    format!("{}{}{}", &mathml[..matched.start()], replacement, &mathml[matched.end()..])
}

pub fn mathml_standardize(mathml: &str) -> String {
    let mut value = ensure_mathml_block(mathml);
    value = assignment_pair_re().replace_all(&value, "<mo>:=</mo>").into_owned();
    value = infinity_variant_re()
        .replace_all(&value, r#"<mi mathvariant="normal">&#x221E;</mi>"#)
        .into_owned();
    value = value
        .replace(SUM_SYMBOL, "&#x2211;")
        .replace(INFINITY_SYMBOL, "&#x221E;");
    value
}

pub(crate) fn preprocess_latex_for_mathml(latex: &str) -> String {
    let mut result = latex.to_string();
    result = normalize_legacy_commands(&result);
    result = replace_command_group(&result, "text", "mathrm");
    result = replace_command_group(&result, "operatorname", "mathrm");
    result = convert_cases_to_supported(&result);
    result = convert_aligned_to_supported(&result);
    result = convert_array_to_supported(&result);
    result = convert_matrix_to_supported(&result);
    result = replace_math_font_command(&result, "mathcal", mathcal_map());
    result = replace_math_font_command(&result, "mathscr", mathscr_map());
    result = replace_math_font_command(&result, "mathfrak", mathfrak_map());
    result = replace_math_font_command(&result, "mathbb", mathbb_map());
    result = replace_stretchy_arrow_commands(&result);
    result = replace_symbol_commands(&result);
    result
}

pub fn latex_to_mathml(latex: &str) -> ExportResult<String> {
    let normalized = normalize_latex_for_export(latex);
    let preprocessed = preprocess_latex_for_mathml(&normalized);
    eprintln!(
        "mathml conversion start: input={normalized:?}, preprocessed={preprocessed:?}"
    );
    match convert_latex_to_mathml(&preprocessed, DisplayStyle::Block) {
        Ok(mathml) => {
            let standardized = mathml_standardize(&mathml);
            eprintln!(
                "mathml conversion ok: input={normalized:?}, preprocessed={preprocessed:?}, output_len={}",
                standardized.len()
            );
            Ok(standardized)
        }
        Err(err) => {
            eprintln!(
                "mathml failed after preprocess: input={latex:?}, normalized={normalized:?}, preprocessed={preprocessed:?}, error={err:?}"
            );
            Err(err.into())
        }
    }
}

fn normalize_legacy_commands(latex: &str) -> String {
    let mut result = legacy_bold_re().replace_all(latex, r"\mathbf").into_owned();
    result = legacy_rm_re().replace_all(&result, r"\mathrm").into_owned();
    result = legacy_it_re().replace_all(&result, r"\mathit").into_owned();
    result
}

fn replace_stretchy_arrow_commands(latex: &str) -> String {
    xrightarrow_re()
        .replace_all(latex, |captures: &Captures<'_>| {
            let label = captures.get(1).map(|m| m.as_str()).unwrap_or_default().trim();
            if label.is_empty() {
                "→".to_string()
            } else {
                format!(r"\overset{{{label}}}{{→}}")
            }
        })
        .into_owned()
}

fn replace_symbol_commands(latex: &str) -> String {
    let mut result = latex.to_string();
    for (from, to) in symbol_replacements() {
        result = result.replace(from, to);
    }
    result
}

fn symbol_replacements() -> &'static [(&'static str, &'static str)] {
    &[
        (r"\triangleq", "≜"),
        (r"\coloneqq", "≔"),
        (r"\eqqcolon", "≕"),
        (r"\approxeq", "≊"),
        (r"\lesssim", "≲"),
        (r"\gtrsim", "≳"),
        (r"\preceq", "⪯"),
        (r"\succeq", "⪰"),
        (r"\vdash", "⊢"),
        (r"\dashv", "⊣"),
        (r"\models", "⊨"),
        (r"\hookrightarrow", "↪"),
        (r"\hookleftarrow", "↩"),
        (r"\twoheadrightarrow", "↠"),
        (r"\rightsquigarrow", "⇝"),
        (r"\leadsto", "⇝"),
        (r"\langle", "⟨"),
        (r"\rangle", "⟩"),
        (r"\lVert", "‖"),
        (r"\rVert", "‖"),
        (r"\ldots", "…"),
        (r"\cdots", "⋯"),
        (r"\vdots", "⋮"),
        (r"\ddots", "⋱"),
    ]
}

fn replace_math_font_command(
    latex: &str,
    command: &str,
    map: &'static HashMap<char, &'static str>,
) -> String {
    let needle = format!(r"\{command}");
    let mut output = String::new();
    let mut index = 0usize;

    while let Some(relative) = latex[index..].find(&needle) {
        let start = index + relative;
        output.push_str(&latex[index..start]);
        let mut cursor = start + needle.len();
        while cursor < latex.len() && latex.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if let Some((group, end)) = parse_braced_group(latex, cursor) {
            output.push_str(&map_font_group(&group, map));
            index = end;
            continue;
        }
        output.push_str(&needle);
        index = start + needle.len();
    }

    output.push_str(&latex[index..]);
    output
}

fn map_font_group(group: &str, map: &'static HashMap<char, &'static str>) -> String {
    let mut output = String::new();
    let mut chars = group.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            output.push(ch);
            if let Some(next) = chars.next() {
                output.push(next);
            }
            continue;
        }
        if let Some(mapped) = map.get(&ch) {
            output.push_str(mapped);
        } else {
            output.push(ch);
        }
    }
    output
}

fn mathcal_map() -> &'static HashMap<char, &'static str> {
    static MAP: OnceLock<HashMap<char, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ('A', "𝒜"), ('B', "ℬ"), ('C', "𝒞"), ('D', "𝒟"), ('E', "ℰ"),
            ('F', "ℱ"), ('G', "𝒢"), ('H', "ℋ"), ('I', "ℐ"), ('J', "𝒥"),
            ('K', "𝒦"), ('L', "ℒ"), ('M', "ℳ"), ('N', "𝒩"), ('O', "𝒪"),
            ('P', "𝒫"), ('Q', "𝒬"), ('R', "ℛ"), ('S', "𝒮"), ('T', "𝒯"),
            ('U', "𝒰"), ('V', "𝒱"), ('W', "𝒲"), ('X', "𝒳"), ('Y', "𝒴"),
            ('Z', "𝒵"),
        ])
    })
}

fn mathscr_map() -> &'static HashMap<char, &'static str> {
    static MAP: OnceLock<HashMap<char, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ('A', "𝒜"), ('B', "ℬ"), ('C', "𝒞"), ('D', "𝒟"), ('E', "ℰ"),
            ('F', "ℱ"), ('G', "𝒢"), ('H', "ℋ"), ('I', "ℐ"), ('J', "𝒥"),
            ('K', "𝒦"), ('L', "ℒ"), ('M', "ℳ"), ('N', "𝒩"), ('O', "𝒪"),
            ('P', "𝒫"), ('Q', "𝒬"), ('R', "ℛ"), ('S', "𝒮"), ('T', "𝒯"),
            ('U', "𝒰"), ('V', "𝒱"), ('W', "𝒲"), ('X', "𝒳"), ('Y', "𝒴"),
            ('Z', "𝒵"),
        ])
    })
}

fn mathfrak_map() -> &'static HashMap<char, &'static str> {
    static MAP: OnceLock<HashMap<char, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ('A', "𝔄"), ('B', "𝔅"), ('C', "ℭ"), ('D', "𝔇"), ('E', "𝔈"),
            ('F', "𝔉"), ('G', "𝔊"), ('H', "ℌ"), ('I', "ℑ"), ('J', "𝔍"),
            ('K', "𝔎"), ('L', "𝔏"), ('M', "𝔐"), ('N', "𝔑"), ('O', "𝔒"),
            ('P', "𝔓"), ('Q', "𝔔"), ('R', "ℜ"), ('S', "𝔖"), ('T', "𝔗"),
            ('U', "𝔘"), ('V', "𝔙"), ('W', "𝔚"), ('X', "𝔛"), ('Y', "𝔜"),
            ('Z', "ℨ"),
            ('a', "𝔞"), ('b', "𝔟"), ('c', "𝔠"), ('d', "𝔡"), ('e', "𝔢"),
            ('f', "𝔣"), ('g', "𝔤"), ('h', "𝔥"), ('i', "𝔦"), ('j', "𝔧"),
            ('k', "𝔨"), ('l', "𝔩"), ('m', "𝔪"), ('n', "𝔫"), ('o', "𝔬"),
            ('p', "𝔭"), ('q', "𝔮"), ('r', "𝔯"), ('s', "𝔰"), ('t', "𝔱"),
            ('u', "𝔲"), ('v', "𝔳"), ('w', "𝔴"), ('x', "𝔵"), ('y', "𝔶"),
            ('z', "𝔷"),
        ])
    })
}

fn mathbb_map() -> &'static HashMap<char, &'static str> {
    static MAP: OnceLock<HashMap<char, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        HashMap::from([
            ('A', "𝔸"), ('B', "𝔹"), ('C', "ℂ"), ('D', "𝔻"), ('E', "𝔼"),
            ('F', "𝔽"), ('G', "𝔾"), ('H', "ℍ"), ('I', "𝕀"), ('J', "𝕁"),
            ('K', "𝕂"), ('L', "𝕃"), ('M', "𝕄"), ('N', "ℕ"), ('O', "𝕆"),
            ('P', "ℙ"), ('Q', "ℚ"), ('R', "ℝ"), ('S', "𝕊"), ('T', "𝕋"),
            ('U', "𝕌"), ('V', "𝕍"), ('W', "𝕎"), ('X', "𝕏"), ('Y', "𝕐"),
            ('Z', "ℤ"),
        ])
    })
}

fn replace_command_group(input: &str, command: &str, replacement: &str) -> String {
    let needle = format!(r"\{command}");
    let mut output = String::new();
    let mut index = 0usize;

    while let Some(relative) = input[index..].find(&needle) {
        let start = index + relative;
        output.push_str(&input[index..start]);
        let mut cursor = start + needle.len();
        while cursor < input.len() && input.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor < input.len() && input.as_bytes()[cursor] as char == '{' {
            if let Some((group, end)) = parse_braced_group(input, cursor) {
                output.push('\\');
                output.push_str(replacement);
                output.push('{');
                output.push_str(&group);
                output.push('}');
                index = end;
                continue;
            }
        }
        output.push_str(&needle);
        index = start + needle.len();
    }

    output.push_str(&input[index..]);
    output
}

fn convert_cases_to_supported(latex: &str) -> String {
    replace_environment(latex, "cases", |body| {
        format!(
            r"\left\{{ \begin{{matrix}} {} \end{{matrix}} \right.",
            trim_trailing_row_breaks(body)
        )
    })
}

fn convert_aligned_to_supported(latex: &str) -> String {
    replace_environment(latex, "aligned", |body| {
        format!(r"\begin{{align}} {} \end{{align}}", trim_trailing_row_breaks(body))
    })
}

fn convert_array_to_supported(latex: &str) -> String {
    let value = begin_array_re()
        .replace_all(latex, r"\begin{matrix}")
        .into_owned();
    value.replace(r"\end{array}", r"\end{matrix}")
}

fn convert_matrix_to_supported(latex: &str) -> String {
    let mut result = latex.to_string();
    result = replace_environment(&result, "Bmatrix", |body| {
        format!(
            r"\left\{{ \begin{{matrix}} {} \end{{matrix}} \right\}}",
            trim_trailing_row_breaks(body)
        )
    });
    result = replace_environment(&result, "Vmatrix", |body| {
        format!(
            r"\left\Vert \begin{{matrix}} {} \end{{matrix}} \right\Vert",
            trim_trailing_row_breaks(body)
        )
    });
    result
}

fn replace_environment(input: &str, environment: &str, mapper: impl Fn(&str) -> String) -> String {
    replace_environment_with_prefix(input, environment, |_, body| mapper(body))
}

fn replace_environment_with_prefix(
    input: &str,
    environment: &str,
    mapper: impl Fn(&str, &str) -> String,
) -> String {
    let begin_tag = format!(r"\begin{{{environment}}}");
    let end_tag = format!(r"\end{{{environment}}}");
    let mut output = String::new();
    let mut index = 0usize;

    while let Some(relative) = input[index..].find(&begin_tag) {
        let start = index + relative;
        output.push_str(&input[index..start]);
        let body_start = start + begin_tag.len();
        if let Some(end_relative) = input[body_start..].find(&end_tag) {
            let end_start = body_start + end_relative;
            let body = &input[body_start..end_start];
            let replacement = mapper(&input[start..body_start], body);
            output.push_str(&replacement);
            index = end_start + end_tag.len();
        } else {
            output.push_str(&input[start..]);
            return output;
        }
    }

    output.push_str(&input[index..]);
    output
}

fn trim_trailing_row_breaks(body: &str) -> String {
    let mut value = body.trim().to_string();
    loop {
        let trimmed = value.trim_end();
        if let Some(stripped) = trimmed.strip_suffix(r"\\") {
            value = stripped.trim_end().to_string();
        } else {
            return trimmed.to_string();
        }
    }
}

fn parse_braced_group(input: &str, index: usize) -> Option<(String, usize)> {
    if index >= input.len() || input.as_bytes()[index] as char != '{' {
        return None;
    }
    let mut depth = 0usize;
    let mut cursor = index;
    let mut escaped = false;
    while cursor < input.len() {
        let ch = input.as_bytes()[cursor] as char;
        if escaped {
            escaped = false;
            cursor += 1;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((input[index + 1..cursor].to_string(), cursor + 1));
                }
            }
            _ => {}
        }
        cursor += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{latex_to_mathml, preprocess_latex_for_mathml};

    #[test]
    fn test_basic_mathml() {
        let result = latex_to_mathml(r"\frac{1}{2}");
        println!("mathml result: {:?}", result);
        assert!(result.is_ok(), "mathml conversion failed: {:?}", result.err());
        let mathml = result.unwrap();
        assert!(mathml.contains("display=\"block\""));
        assert!(mathml.contains("<math"), "output doesn't contain <math>: {mathml}");
        assert!(mathml.contains("<mfrac>"));
    }

    #[test]
    fn test_quadratic() {
        let result = latex_to_mathml(r"x=\frac{-b\pm\sqrt{b^{2}-4ac}}{2a}");
        println!("mathml result: {:?}", result);
        assert!(result.is_ok(), "mathml conversion failed: {:?}", result.err());
    }

    #[test]
    fn test_cases_environment() {
        let input = r"\begin{cases} x+1 & \text{if } x > 0 \\ -x & \text{otherwise} \end{cases}";
        let result = latex_to_mathml(input);
        assert!(result.is_ok(), "cases failed: {:?}", result.err());
    }

    #[test]
    fn test_pmatrix() {
        let input = r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}";
        let result = latex_to_mathml(input);
        assert!(result.is_ok(), "pmatrix failed: {:?}", result.err());
    }

    #[test]
    fn test_aligned() {
        let input = r"\begin{aligned} x &= 1 \\ y &= 2 \end{aligned}";
        let result = latex_to_mathml(input);
        assert!(result.is_ok(), "aligned failed: {:?}", result.err());
    }

    #[test]
    fn test_original_failing_case() {
        let input = r"\begin{cases}{{\bf z}=x+iy} \\{| {\bf z} |=\sqrt{x^{2}+y^{2}}=1} \\\end{cases}";
        let result = latex_to_mathml(input);
        assert!(result.is_ok(), "original cases failed: {:?}", result.err());
    }

    #[test]
    fn preprocesses_cases_and_legacy_commands() {
        let output = preprocess_latex_for_mathml(
            r"\begin{cases} {\bf z} & \text{if } x > 0 \end{cases}",
        );
        assert!(output.contains(r"\left\{"));
        assert!(output.contains(r"\mathbf"));
        assert!(output.contains(r"\mathrm{if }"));
    }

    #[test]
    fn test_mathcal_mathfrak() {
        let input = r"\mathfrak{m} \triangleq T_{\mathcal{E}} \mathcal{M}";
        let result = latex_to_mathml(input);
        assert!(result.is_ok(), "failed: {:?}", result.err());
        let mathml = result.unwrap();
        assert!(!mathml.contains("PARSE ERROR"), "mathml contains parse error: {mathml}");
        assert!(!mathml.contains("Undefined"), "mathml contains undefined: {mathml}");
    }

    #[test]
    fn test_common_symbols() {
        let input = r"A \triangleq B \vdash C \hookrightarrow D";
        let result = latex_to_mathml(input);
        assert!(result.is_ok(), "failed: {:?}", result.err());
    }
}
