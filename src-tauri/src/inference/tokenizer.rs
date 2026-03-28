use std::path::Path;

use tokenizers::Tokenizer;

use crate::inference::InferenceResult;

pub struct LatexTokenizer {
    inner: Tokenizer,
}

impl LatexTokenizer {
    pub fn from_file(path: &Path) -> InferenceResult<Self> {
        let inner = Tokenizer::from_file(path)?;
        Ok(Self { inner })
    }

    #[allow(dead_code)]
    pub fn decode(&self, token_ids: &[i64]) -> InferenceResult<String> {
        let ids = token_ids
            .iter()
            .copied()
            .map(u32::try_from)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self.inner.decode(&ids, true)?)
    }

    pub fn inner(&self) -> &Tokenizer {
        &self.inner
    }
}
