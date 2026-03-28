pub mod latex;
pub mod markdown;
pub mod mathml;
pub mod typst;

use serde::Serialize;

pub type ExportError = Box<dyn std::error::Error + Send + Sync>;
pub type ExportResult<T> = Result<T, ExportError>;

#[derive(Debug, Clone, Serialize)]
pub struct ExportBundle {
    pub latex: String,
    pub markdown: String,
    pub mathml: Option<String>,
    pub typst: Option<String>,
}

pub fn build_export_formats(latex: &str) -> ExportBundle {
    let normalized = self::latex::normalize_latex_for_export(latex);
    eprintln!("build_export_formats start: latex={normalized:?}");
    if normalized.is_empty() {
        return ExportBundle {
            latex: String::new(),
            markdown: String::new(),
            mathml: None,
            typst: None,
        };
    }

    let mathml = match self::mathml::latex_to_mathml(&normalized) {
        Ok(value) => Some(value),
        Err(err) => {
            eprintln!("build_export_formats: mathml failed: latex={normalized:?}, error={err}");
            None
        }
    };
    let typst = Some(self::typst::latex_to_typst(&normalized));
    let markdown = self::markdown::latex_to_markdown(&normalized);

    eprintln!(
        "build_export_formats done: latex_len={}, mathml={}, typst={}",
        normalized.len(),
        mathml.as_ref().map(|value| value.len()).unwrap_or(0),
        typst.as_ref().map(|value| value.len()).unwrap_or(0),
    );

    ExportBundle {
        latex: normalized,
        markdown,
        mathml,
        typst,
    }
}

#[cfg(test)]
mod tests {
    use super::build_export_formats;

    #[test]
    fn bundle_contains_common_formats() {
        let bundle = build_export_formats("$$x^2 + y^2 = z^2$$");
        assert_eq!(bundle.latex, "x^2 + y^2 = z^2");
        assert_eq!(bundle.markdown, "$$\nx^2 + y^2 = z^2\n$$");
        assert!(bundle.mathml.as_deref().unwrap_or_default().contains("<math"));
        assert!(bundle.typst.as_deref().unwrap_or_default().contains("x^2"));
    }
}
