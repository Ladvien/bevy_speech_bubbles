//! What a balloon looks like: its shape channel, and the affect that tints it.

/// Balloon style — drives both the drawn shape and the semantic channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub enum BubbleKind {
    /// Spoken aloud: rounded-rect balloon with a pointed tail. Directed dialogue / story beats.
    Speech,
    /// Inner voice: soft pill balloon with a trailing dot-tail. Ambient feeling / intent / emotion.
    Thought,
}

/// Optional affect on a line — tints the balloon border.
///
/// Grounded in An et al., *AniBalloons* (arXiv:2408.06294): balloon colour and animation reliably
/// convey emotion, which is why this is a first-class axis rather than something a caller bakes into
/// its own colour choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize))]
pub enum Emotion {
    #[default]
    Neutral,
    Joy,
    Anger,
    Sadness,
    Surprise,
    Fear,
    Calm,
}
