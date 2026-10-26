use crate::BlockId;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Loro(#[from] loro::LoroError),
    #[error(transparent)]
    Encode(#[from] loro::LoroEncodeError),
    #[error("block {0} does not exist")]
    NoSuchBlock(BlockId),
    #[error("edit past the end of block {0}'s text")]
    OutOfRange(BlockId),
    #[error("block {0} has no text container")]
    MissingText(BlockId),
    #[error("text update timed out")]
    TextUpdate(#[from] loro::UpdateTimeoutError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
