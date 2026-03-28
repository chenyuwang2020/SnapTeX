use super::latex::normalize_latex_for_export;

pub fn latex_to_markdown(latex: &str) -> String {
    let text = normalize_latex_for_export(latex);
    if text.is_empty() {
        return String::new();
    }
    format!("$$\n{text}\n$$")
}

#[cfg(test)]
mod tests {
    use super::latex_to_markdown;

    #[test]
    fn wraps_math_block() {
        assert_eq!(latex_to_markdown("x^2"), "$$\nx^2\n$$");
    }
}
