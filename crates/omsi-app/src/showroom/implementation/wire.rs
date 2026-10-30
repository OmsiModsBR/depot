//! The messages as bytes, over the helper's standard input and output.

use super::super::domain::{message::Msg, shape::Shape};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

#[derive(Serialize, Deserialize)]
struct Status {
    busy: bool,
    error: Option<String>,
}

/// Every message starts with these bytes, then its kind and length: a line written to the
/// output by anything else is passed over, not taken for a picture.
const MAGIC: &[u8; 4] = b"DPV1";

pub fn write_msg(w: &mut impl Write, m: &Msg) -> std::io::Result<()> {
    let (kind, body) = match m {
        Msg::Frame(fw, fh, tag, px) => {
            let mut b = Vec::with_capacity(16 + px.len());
            b.extend_from_slice(&fw.to_le_bytes());
            b.extend_from_slice(&fh.to_le_bytes());
            b.extend_from_slice(&tag.to_le_bytes());
            b.extend_from_slice(px);
            (b'F', b)
        }
        Msg::Status { busy, error } => (
            b'S',
            serde_json::to_vec(&Status {
                busy: *busy,
                error: error.clone(),
            })
            .unwrap_or_default(),
        ),
        Msg::Kept(tag, shape) => {
            let mut b = Vec::with_capacity(16);
            b.extend_from_slice(&tag.to_le_bytes());
            b.extend_from_slice(&shape.0.to_le_bytes());
            b.extend_from_slice(&shape.1.to_le_bytes());
            (b'K', b)
        }
    };
    // (in one write: a line another thread prints to the same output comes before or after the
    // message, never in the middle of it - the output is locked for each write)
    let mut all = Vec::with_capacity(9 + body.len());
    all.extend_from_slice(MAGIC);
    all.push(kind);
    all.extend_from_slice(&(body.len() as u32).to_le_bytes());
    all.extend_from_slice(&body);
    w.write_all(&all)?;
    w.flush()
}

/// The largest picture taken (a 4K window's preview), against a stream gone wrong.
const MAX_BODY: usize = 16 + 4096 * 2304 * 4;

pub fn read_msg(r: &mut impl Read) -> std::io::Result<Msg> {
    let bad = |what: &str| std::io::Error::new(std::io::ErrorKind::InvalidData, what.to_string());
    // (what is not a message - a line some library printed - is passed over up to the next)
    let mut head = [0u8; 9];
    r.read_exact(&mut head[..4])?;
    let mut skipped = 0usize;
    while &head[..4] != MAGIC {
        head.copy_within(1..4, 0);
        r.read_exact(&mut head[3..4])?;
        skipped += 1;
    }
    if skipped > 0 {
        log::debug!("preview helper: {skipped} bytes that were not a message passed over");
    }
    r.read_exact(&mut head[4..])?;
    let len = u32::from_le_bytes([head[5], head[6], head[7], head[8]]) as usize;
    if len > MAX_BODY {
        return Err(bad("preview message too long"));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body)?;
    match head[4] {
        b'F' if len >= 16 => {
            let w = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
            let h = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
            let tag = u64::from_le_bytes([
                body[8], body[9], body[10], body[11], body[12], body[13], body[14], body[15],
            ]);
            if w == 0
                || h == 0
                || w > 8192
                || h > 8192
                || (w as usize) * (h as usize) * 4 != len - 16
            {
                return Err(bad("preview picture of the wrong size"));
            }
            body.drain(..16);
            Ok(Msg::Frame(w, h, tag, body))
        }
        b'S' => {
            let s: Status = serde_json::from_slice(&body).map_err(|_| bad("preview status"))?;
            Ok(Msg::Status {
                busy: s.busy,
                error: s.error,
            })
        }
        b'K' if len == 16 => {
            let n = |i: usize| u32::from_le_bytes([body[i], body[i + 1], body[i + 2], body[i + 3]]);
            Ok(Msg::Kept(
                u64::from_le_bytes([
                    body[0], body[1], body[2], body[3], body[4], body[5], body[6], body[7],
                ]),
                Shape(n(8), n(12)),
            ))
        }
        _ => Err(bad("unknown preview message")),
    }
}
