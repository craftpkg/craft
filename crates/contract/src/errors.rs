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
pub enum NetworkError {
    #[error("GET {url} returned HTTP {status}")]
    Status { url: String, status: u16 },
}

#[derive(Error, Debug)]
pub enum IntegrityError {
    #[error("tarball integrity mismatch: expected {expected}, got {actual}")]
    Mismatch { expected: String, actual: String },

    #[error("unsupported or malformed integrity: {integrity}")]
    Invalid { integrity: String },
}

#[derive(Error, Debug)]
pub enum RegistryError {
    #[error("package {name} not found in {registry}")]
    PackageNotFound { name: String, registry: String },

    #[error("{registry} denied access to {name} (HTTP {status}); check your registry credentials")]
    AccessDenied {
        name: String,
        registry: String,
        status: u16,
    },
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
pub enum TarballError {
    #[error("tarball entry escapes the package directory: {path}")]
    UnsafePath { path: String },
}

#[derive(Error, Debug)]
pub enum ScriptError {
    #[error("script \"{name}\" not found{}", available_scripts(.available))]
    NotFound {
        name: String,
        available: Vec<String>,
    },

    #[error("{name} exited with code {code}")]
    Failed { name: String, code: i32 },
}

#[derive(Error, Debug)]
pub enum LockfileError {
    #[error("craft.lock is missing; run `craft install` to create it")]
    Missing,

    #[error("craft.lock is out of date with package.json: {}; run `craft install` to update it", .differences.join(", "))]
    OutOfDate { differences: Vec<String> },

    #[error("{dependency} is not in craft.lock; run `craft install` to update it")]
    NotLocked { dependency: String },

    #[error(
        "craft.lock has lockfileVersion {found}, but this craft reads up to {supported}; upgrade craft"
    )]
    UnsupportedVersion { found: u32, supported: u32 },
}

#[derive(Error, Debug)]
pub enum PackageError {
    #[error("no package.json found")]
    NoPackageJson,

    #[error("package.json must contain a JSON object")]
    NotAnObject,
}

fn available_scripts(available: &[String]) -> String {
    if available.is_empty() {
        return String::new();
    }
    format!("; available scripts: {}", available.join(", "))
}
