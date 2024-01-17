use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    IO(#[from] io::Error),

    #[error("unknown key")]
    UnknownKey,

    #[error("failed serialization/deserizalization")]
    Serde,
}
