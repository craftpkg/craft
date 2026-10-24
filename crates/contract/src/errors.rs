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

#[derive(Error, Debug)]
pub enum ContextError {
    #[error("could not determine home directory; set CRAFT_HOME")]
    NoHomeDir,
}

#[derive(Error, Debug)]
pub enum SpecError {
    #[error("invalid dependency spec: {dependency}")]
    Invalid { dependency: String },

    #[error("{kind} dependencies are not supported yet: {dependency}")]
    Unsupported {
        kind: &'static str,
        dependency: String,
    },
}

#[derive(Error, Debug)]
pub enum PackageError {
    #[error("no package.json found")]
    NoPackageJson,
}
