//! The server's notifications (`notify`): cards over the navigator that go by themselves.

/// A notification from the server (`notify`): a card over the navigator that goes by itself.
#[derive(Clone, Debug, PartialEq)]
pub struct Notice {
    pub kind: NoticeKind,
    pub text: String,
    /// Seconds it still shows, of `total`.
    pub left: f32,
    pub total: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NoticeKind {
    #[cfg_attr(not(feature = "lan"), allow(dead_code))]
    Info,
    #[cfg_attr(not(feature = "lan"), allow(dead_code))]
    Warn,
    #[cfg_attr(not(feature = "lan"), allow(dead_code))]
    Alert,
}

/// How many notifications show at once (the newest; an older one makes room).
#[cfg_attr(not(feature = "lan"), allow(dead_code))]
pub const NOTICES_SHOWN: usize = 3;

impl Notice {
    /// How opaque it is: fading in for a quarter second, out for its last half second.
    pub fn alpha(&self) -> f32 {
        (self.left / 0.5)
            .min((self.total - self.left) / 0.25)
            .clamp(0.0, 1.0)
    }
}
