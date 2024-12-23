use miette::{Diagnostic, SourceSpan};
use thiserror::Error;

#[derive(Diagnostic, Debug, Error)]
#[error("Unexpected EOF")]
pub struct Eof;

#[derive(Diagnostic, Debug, Error)]
#[error("Unexpected token '{token}'")]
pub struct SingleTokenError {
    #[source_code]
    pub(crate) src: String,

    pub token: char,

    #[label = "this input character"]
    pub(crate) err_span: SourceSpan,
}

impl SingleTokenError {
    pub fn line(&self) -> usize {
        let until_unrecongized = &self.src[..=self.err_span.offset()];
        until_unrecongized.lines().count()
    }
}
