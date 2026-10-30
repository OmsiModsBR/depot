use serde::{de::DeserializeOwned, Serialize};

/// Longest frame the reader accepts, so a corrupt length cannot ask for gigabytes.
pub const MAX_FRAME_LEN: usize = 1 << 20;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("frame is {0} bytes, over the {MAX_FRAME_LEN} byte limit")]
    TooLong(usize),
    #[error("frame is incomplete")]
    Incomplete,
    #[error("frame holds a message that cannot be read")]
    Malformed,
}

/// Message as a length-prefixed record: `u32` little endian length, then the payload.
pub fn encode<T: Serialize>(msg: &T) -> Result<Vec<u8>, FrameError> {
    let payload = postcard::to_stdvec(msg).map_err(|_| FrameError::Malformed)?;
    if payload.len() > MAX_FRAME_LEN {
        return Err(FrameError::TooLong(payload.len()));
    }
    let mut out = Vec::with_capacity(4 + payload.len());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

/// Reads one frame from the start of `buf`. Returns the message and how many bytes it used.
/// `Incomplete` means "read more bytes and try again".
pub fn decode<T: DeserializeOwned>(buf: &[u8]) -> Result<(T, usize), FrameError> {
    let head: [u8; 4] = buf
        .get(..4)
        .ok_or(FrameError::Incomplete)?
        .try_into()
        .unwrap();
    let len = u32::from_le_bytes(head) as usize;
    if len > MAX_FRAME_LEN {
        return Err(FrameError::TooLong(len));
    }
    let payload = buf.get(4..4 + len).ok_or(FrameError::Incomplete)?;
    let msg = postcard::from_bytes(payload).map_err(|_| FrameError::Malformed)?;
    Ok((msg, 4 + len))
}
