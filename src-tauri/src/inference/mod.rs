pub mod decoder_loop;
pub mod postprocessor;
pub mod preprocessor;
pub mod recognizer;
pub mod tokenizer;

pub type InferenceError = Box<dyn std::error::Error + Send + Sync>;
pub type InferenceResult<T> = Result<T, InferenceError>;

pub fn inference_error(message: impl Into<String>) -> InferenceError {
    std::io::Error::other(message.into()).into()
}
