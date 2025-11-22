use thiserror::Error;

#[derive(Error, Debug)]
pub enum PipelineError {
    #[error("Network error: {message}")]
    NetworkError { message: String },

    #[error("Unzip error: {message}")]
    UnzipError { message: String },

    #[error("IO error: {message}")]
    IoError { message: String },

    #[error("Parse error: {message}")]
    ParseError { message: String },

    #[error("Download error: {message}")]
    DownloadError { message: String },
}
