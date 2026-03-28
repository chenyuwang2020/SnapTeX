use std::collections::HashMap;
use std::sync::OnceLock;

use super::latex::normalize_latex_for_export;

pub fn latex_to_typst(latex: &str) -> String {
    let clean = normalize_latex_for_export(latex);
    if clean.is_empty() {
        return String::new();
    }
    let preprocessed = preprocess_latex_for_typst(&clean);
    cleanup_typst_spacing(&transform_expr(&preprocessed))
}

fn preprocess_latex_for_typst(latex: &str) -> String {
    let mut result = latex.to_string();
    result = result.replace(r"\bf", r"\mathbf");
    result = result.replace(r"\rm", r"\mathrm");
    result = result.replace(r"\it", r"\mathit");
    result = result.replace(r"\operatorname", r"\mathrm");
    result = result
        .replace(r"\begin{aligned}", r"\begin{align}")
        .replace(r"\end{aligned}", r"\end{align}");
    result = convert_cases_to_typst(&result);
    result = convert_align_to_typst(&result);
    result = convert_matrix_to_typst(&result, "matrix", None);
    result = convert_matrix_to_typst(&result, "pmatrix", Some(("(", ")")));
    result = convert_matrix_to_typst(&result, "bmatrix", Some(("[", "]")));
    result = convert_matrix_to_typst(&result, "vmatrix", Some(("|", "|")));
    result = convert_matrix_to_typst(&result, "Bmatrix", Some(("{", "}")));
    result = result.replace(r"\text", r"\mathrm");
    result
}

fn transform_expr(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = String::new();
    let mut index = 0usize;

    while index < input.len() {
        let ch = bytes[index] as char;
        match ch {
            '\\' => {
                let (command, next_index) = parse_command(input, index + 1);
                if command.is_empty() {
                    output.push('\\');
                    index += 1;
                    continue;
                }
                index = next_index;
                match command.as_str() {
                    "frac" | "dfrac" | "tfrac" => {
                        let (left, next) = parse_argument(input, index);
                        let (right, next) = parse_argument(input, next);
                        output.push_str(&format!(
                            "({})/({})",
                            transform_expr(&left),
                            transform_expr(&right)
                        ));
                        index = next;
                    }
                    "sqrt" => {
                        let (root, next) = parse_optional_bracket_argument(input, index);
                        let (radicand, next) = parse_argument(input, next);
                        if let Some(root) = root {
                            output.push_str(&format!(
                                "root({}, {})",
                                transform_expr(&root),
                                transform_expr(&radicand)
                            ));
                        } else {
                            output.push_str(&format!("sqrt({})", transform_expr(&radicand)));
                        }
                        index = next;
                    }
                    "left" | "right" | "," | ";" | ":" | "!" | "quad" | "qquad" => {}
                    "text" | "mathrm" | "operatorname" | "mathit" | "mathbf" | "mathsf"
                    | "mathbb" | "mathcal" | "mathfrak" | "textit" | "textbf" => {
                        let (inner, next) = parse_argument(input, index);
                        output.push_str(&transform_expr(&inner));
                        index = next;
                    }
                    _ => output.push_str(map_command(&command)),
                }
            }
            '^' => {
                let (arg, next) = parse_argument(input, index + 1);
                output.push('^');
                output.push_str(&format_script_arg(&arg));
                index = next;
            }
            '_' => {
                let (arg, next) = parse_argument(input, index + 1);
                output.push('_');
                output.push_str(&format_script_arg(&arg));
                index = next;
            }
            '{' => {
                let (group, next) = parse_braced_group(input, index);
                output.push_str(&transform_expr(&group));
                index = next;
            }
            '}' => {
                index += 1;
            }
            _ => {
                output.push(ch);
                index += 1;
            }
        }
    }

    output
}

fn parse_command(input: &str, mut index: usize) -> (String, usize) {
    let bytes = input.as_bytes();
    let start = index;
    while index < input.len() {
        let ch = bytes[index] as char;
        if ch.is_ascii_alphabetic() {
            index += 1;
        } else {
            break;
        }
    }
    if index == start && index < input.len() {
        return ((bytes[index] as char).to_string(), index + 1);
    }
    (input[start..index].to_string(), index)
}

fn parse_argument(input: &str, index: usize) -> (String, usize) {
    let next = skip_spaces(input, index);
    if next >= input.len() {
        return (String::new(), next);
    }
    let ch = input.as_bytes()[next] as char;
    if ch == '{' {
        return parse_braced_group(input, next);
    }
    if ch == '\\' {
        let (command, end) = parse_command(input, next + 1);
        return (format!(r"\{command}"), end);
    }
    (ch.to_string(), next + 1)
}

fn parse_optional_bracket_argument(input: &str, index: usize) -> (Option<String>, usize) {
    let next = skip_spaces(input, index);
    if next >= input.len() || input.as_bytes()[next] as char != '[' {
        return (None, next);
    }
    let mut depth = 1usize;
    let mut cursor = next + 1;
    while cursor < input.len() {
        let ch = input.as_bytes()[cursor] as char;
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return (Some(input[next + 1..cursor].to_string()), cursor + 1);
                }
            }
            _ => {}
        }
        cursor += 1;
    }
    (Some(input[next + 1..].to_string()), input.len())
}

fn parse_braced_group(input: &str, index: usize) -> (String, usize) {
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
                    return (input[index + 1..cursor].to_string(), cursor + 1);
                }
            }
            _ => {}
        }
        cursor += 1;
    }
    (input[index + 1..].to_string(), input.len())
}

fn skip_spaces(input: &str, mut index: usize) -> usize {
    while index < input.len() && (input.as_bytes()[index] as char).is_whitespace() {
        index += 1;
    }
    index
}

fn format_script_arg(arg: &str) -> String {
    let value = cleanup_typst_spacing(&transform_expr(arg));
    if is_simple_token(&value) {
        value
    } else {
        format!("({value})")
    }
}

fn cleanup_typst_spacing(input: &str) -> String {
    input
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("( ", "(")
        .replace("[ ", "[")
        .replace("{ ", "{")
        .replace(" )", ")")
        .replace(" ]", "]")
        .replace(" }", "}")
}

fn is_simple_token(input: &str) -> bool {
    !input.is_empty()
        && input
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '+' | 'α'..='ω' | 'Α'..='Ω'))
}

fn map_command(command: &str) -> &'static str {
    static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    let map = MAP.get_or_init(|| {
        HashMap::from([
            ("alpha", "alpha"),
            ("beta", "beta"),
            ("gamma", "gamma"),
            ("delta", "delta"),
            ("epsilon", "epsilon"),
            ("varepsilon", "epsilon"),
            ("theta", "theta"),
            ("vartheta", "theta"),
            ("lambda", "lambda"),
            ("mu", "mu"),
            ("nu", "nu"),
            ("pi", "pi"),
            ("varphi", "phi"),
            ("phi", "phi"),
            ("rho", "rho"),
            ("sigma", "sigma"),
            ("tau", "tau"),
            ("omega", "omega"),
            ("Gamma", "Gamma"),
            ("Delta", "Delta"),
            ("Theta", "Theta"),
            ("Lambda", "Lambda"),
            ("Pi", "Pi"),
            ("Sigma", "Sigma"),
            ("Omega", "Omega"),
            ("sum", "sum"),
            ("prod", "prod"),
            ("int", "integral"),
            ("iint", "integral integral"),
            ("iiint", "integral integral integral"),
            ("oint", "contour-integral"),
            ("pm", "±"),
            ("mp", "∓"),
            ("times", "times"),
            ("cdot", "dot"),
            ("infty", "infinity"),
            ("leq", "<="),
            ("geq", ">="),
            ("neq", "!="),
            ("to", "->"),
            ("rightarrow", "->"),
            ("leftarrow", "<-"),
            ("ldots", "..."),
            ("cdots", "..."),
            ("sin", "sin"),
            ("cos", "cos"),
            ("tan", "tan"),
            ("log", "log"),
            ("ln", "ln"),
            ("lbrace", "{"),
            ("rbrace", "}"),
            ("Vert", "||"),
        ])
    });
    map.get(command).copied().unwrap_or("")
}

fn convert_cases_to_typst(latex: &str) -> String {
    replace_environment(latex, "cases", |body| {
        let rows = rows_to_typst(body);
        format!("cases({rows})")
    })
}

fn convert_align_to_typst(latex: &str) -> String {
    replace_environment(latex, "align", |body| rows_to_typst(body))
}

fn convert_matrix_to_typst(
    latex: &str,
    environment: &str,
    delimiters: Option<(&str, &str)>,
) -> String {
    replace_environment(latex, environment, |body| {
        let matrix = format!("mat({})", rows_to_typst(body));
        if let Some((left, right)) = delimiters {
            format!("{left}{matrix}{right}")
        } else {
            matrix
        }
    })
}

fn rows_to_typst(body: &str) -> String {
    split_rows(trim_trailing_row_breaks(body))
        .into_iter()
        .map(|row| {
            split_columns(&row)
                .into_iter()
                .map(|column| column.trim().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn split_rows(body: String) -> Vec<String> {
    split_top_level(&body, r"\\")
}

fn split_columns(row: &str) -> Vec<String> {
    split_top_level(row, "&")
}

fn split_top_level(input: &str, separator: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let bytes = input.as_bytes();
    let sep_bytes = separator.as_bytes();
    let mut index = 0usize;
    let mut depth = 0usize;
    let mut escaped = false;

    while index < input.len() {
        let ch = bytes[index] as char;
        if escaped {
            current.push(ch);
            escaped = false;
            index += 1;
            continue;
        }
        match ch {
            '\\' if separator != r"\\" => {
                escaped = true;
                current.push(ch);
                index += 1;
                continue;
            }
            '{' => depth += 1,
            '}' if depth > 0 => depth -= 1,
            _ => {}
        }

        if depth == 0
            && index + sep_bytes.len() <= input.len()
            && &bytes[index..index + sep_bytes.len()] == sep_bytes
        {
            parts.push(current.trim().to_string());
            current.clear();
            index += sep_bytes.len();
            continue;
        }

        current.push(ch);
        index += 1;
    }

    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }

    if parts.is_empty() {
        vec![String::new()]
    } else {
        parts
    }
}

fn replace_environment(input: &str, environment: &str, mapper: impl Fn(&str) -> String) -> String {
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
            output.push_str(&mapper(body));
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

#[cfg(test)]
mod tests {
    use super::latex_to_typst;

    #[test]
    fn converts_fraction_and_sqrt() {
        assert_eq!(latex_to_typst(r"\frac{a}{\sqrt{x}}"), "(a)/(sqrt(x))");
    }

    #[test]
    fn converts_scripts() {
        assert_eq!(latex_to_typst(r"x^{n}_i"), "x^n_i");
    }

    #[test]
    fn converts_cases_environment() {
        let value = latex_to_typst(
            r"\begin{cases} x+1 & \text{if } x > 0 \\ -x & \text{otherwise} \end{cases}",
        );
        assert!(value.contains("cases("));
    }

    #[test]
    fn converts_pmatrix_environment() {
        let value = latex_to_typst(r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}");
        assert!(value.contains("mat("));
    }
}
