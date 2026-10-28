//! Reading a notification and keeping the list of them.

use super::super::domain::{Notice, NoticeKind, NOTICES_SHOWN};

impl Notice {
    /// `notify`'s argument: `<id> <seconds> <info|warn|alert> <text>` → the id and the notice
    /// (seconds held to 2 .. 30; an unknown kind is information).
    #[cfg_attr(not(feature = "lan"), allow(dead_code))]
    pub fn parse(arg: &str) -> Option<(String, Notice)> {
        let mut it = arg.trim().splitn(4, ' ');
        let id = it.next().filter(|s| !s.is_empty())?.to_string();
        let secs: f32 = it.next()?.parse().ok().filter(|s: &f32| s.is_finite())?;
        let kind = match it.next()? {
            "warn" => NoticeKind::Warn,
            "alert" => NoticeKind::Alert,
            _ => NoticeKind::Info,
        };
        let text = it
            .next()
            .map(str::trim)
            .filter(|t| !t.is_empty())?
            .to_string();
        let total = secs.clamp(2.0, 30.0);
        Some((
            id,
            Notice {
                kind,
                text,
                left: total,
                total,
            },
        ))
    }
}

/// A new notice in the list, the oldest going when there are more than `NOTICES_SHOWN`.
#[cfg_attr(not(feature = "lan"), allow(dead_code))]
pub fn push_notice(list: &mut Vec<Notice>, n: Notice) {
    list.push(n);
    while list.len() > NOTICES_SHOWN {
        list.remove(0);
    }
}
