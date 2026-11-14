use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("config: {0}")]
    Config(String),
    #[error("vision: {0}")]
    Vision(String),
    #[error("guides api: {0}")]
    Api(String),
    #[error("build order: {0}")]
    Build(String),
    #[error("platform: {0}")]
    Platform(String),
    #[error("{0}")]
    Message(String),
}

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
