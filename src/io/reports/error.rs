use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReportError {
    #[error("Template error: {0}")]
    Template(#[from] handlebars::TemplateError),
    #[error("Render error: {0}")]
    Render(#[from] handlebars::RenderError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
