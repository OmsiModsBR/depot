use crate::LinkError;
use omsi_devtools_proto::{decode, encode, FrameError, MAX_FRAME_LEN};
use serde::{de::DeserializeOwned, Serialize};
use std::io::{ErrorKind, Read, Write};

/// Reads one whole frame, however the stream splits it. A stream that ends before the first
/// byte of a frame is `Closed`; one that ends inside a frame is an incomplete frame.
pub(crate) fn read_frame<T: DeserializeOwned, R: Read>(r: &mut R) -> Result<T, LinkError> {
    let mut head = [0u8; 4];
    match r.read_exact(&mut head) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Err(LinkError::Closed),
        Err(e) => return Err(e.into()),
    }
    let len = u32::from_le_bytes(head) as usize;
    if len > MAX_FRAME_LEN {
        return Err(FrameError::TooLong(len).into());
    }
    let mut buf = Vec::with_capacity(4 + len);
    buf.extend_from_slice(&head);
    buf.resize(4 + len, 0);
    match r.read_exact(&mut buf[4..]) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Err(FrameError::Incomplete.into()),
        Err(e) => return Err(e.into()),
    }
    Ok(decode(&buf)?.0)
}

pub(crate) fn write_frame<T: Serialize, W: Write>(w: &mut W, msg: &T) -> Result<(), LinkError> {
    w.write_all(&encode(msg)?)?;
    w.flush()?;
    Ok(())
}
