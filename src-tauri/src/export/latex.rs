use regex::Regex;
use std::sync::OnceLock;

fn superscript_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\^\{([A-Za-z0-9])\}").expect("superscript regex"))
}

fn subscript_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"_\{([A-Za-z0-9])\}").expect("subscript regex"))
}

fn assignment_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r":=").expect("assignment regex"))
}

fn operator_spacing_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(\S)(\\(?:sum|prod|int|oint|frac|dfrac|tfrac))")
            .expect("operator spacing regex")
    })
}

fn whitespace_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[ \t]+").expect("whitespace regex"))
}

pub fn strip_math_delimiters(latex: &str) -> String {
    let text = latex.trim();
    if text.len() >= 4 && text.starts_with("$$") && text.ends_with("$$") {
        return text[2..text.len() - 2].trim().to_string();
    }
    if text.len() >= 2 && text.starts_with('$') && text.ends_with('$') {
        return text[1..text.len() - 1].trim().to_string();
    }
    text.to_string()
}

pub fn normalize_latex_for_export(latex: &str) -> String {
    let mut text = strip_math_delimiters(latex);
    if text.is_empty() {
        return text;
    }

    text = superscript_re().replace_all(&text, "^$1").into_owned();
    text = subscript_re().replace_all(&text, "_$1").into_owned();
    text = assignment_re().replace_all(&text, " := ").into_owned();
    text = operator_spacing_re().replace_all(&text, "$1 $2").into_owned();
    text = whitespace_re().replace_all(&text, " ").into_owned();
    text.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::normalize_latex_for_export;

    #[test]
    fn normalizes_delimiters_and_scripts() {
        let value = normalize_latex_for_export(" $$x^{2}+y_{i}:=\\frac{1}{2}$$ ");
        assert_eq!(value, "x^2+y_i := \\frac{1}{2}");
    }
}
