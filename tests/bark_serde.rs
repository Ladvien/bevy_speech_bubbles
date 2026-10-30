//! **A bark is data**: `Bark` and `BarkSettings` round-trip through RON, so a game project or a
//! tool (the bevy_emerge editor, Ladvien/bevy_emerge#106) stores barks with this crate's own types
//! and never a second schema. Fixes #9.

use bevy_speech_bubbles::conversation::{Bark, BarkSettings};
use bevy_speech_bubbles::style::{BubbleKind, Emotion};

#[test]
fn a_bark_round_trips_through_ron() {
    let bark = Bark {
        speaker: "miller".to_owned(),
        kind: BubbleKind::Speech,
        emotion: Emotion::Anger,
        text: "The wheel's stuck again!".to_owned(),
    };
    let text = ron::to_string(&bark).expect("a bark serialises");
    let back: Bark = ron::from_str(&text).expect("the RON a bark wrote reads back");
    assert_eq!(back, bark, "round trip through {text}");
}

#[test]
fn bark_pacing_round_trips_through_ron() {
    let settings = BarkSettings { gap: 0.5, stale: 2.0, queue_cap: 8 };
    let back: BarkSettings = ron::from_str(&ron::to_string(&settings).unwrap()).unwrap();
    assert_eq!(back, settings);
}
