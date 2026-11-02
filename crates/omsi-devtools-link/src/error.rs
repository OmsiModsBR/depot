use omsi_devtools_proto::{FrameError, HelloError};

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("stream failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("bad frame: {0}")]
    Frame(#[from] FrameError),
    #[error("{0}")]
    Version(#[from] HelloError),
    #[error("the other side did not start with a hello")]
    NotHello,
    #[error("the stream ended")]
    Closed,
}
