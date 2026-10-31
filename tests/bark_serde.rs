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

/// A bark file or a pacing file is authored by hand and hot-reloaded, so a key this crate does not
/// have (`ttl`, `max_live`) must fail the parse naming the key: it used to load and the value was
/// dropped without a word (#12).
#[test]
fn an_unknown_bark_field_is_refused() {
    let known = r#"(speaker: "miller", kind: Speech, emotion: Anger, text: "Stuck!""#;
    ron::from_str::<Bark>(&format!("{known})")).expect("the known fields load");
    let err = ron::from_str::<Bark>(&format!("{known}, ttl: 5.0)"))
        .expect_err("`ttl` is not a bark field")
        .to_string();
    assert!(err.contains("ttl"), "the diagnostic names the key: {err}");
}

#[test]
fn an_unknown_pacing_field_is_refused() {
    let known = "(gap: 1.0, stale: 3.0, queue_cap: 4";
    ron::from_str::<BarkSettings>(&format!("{known})")).expect("the known fields load");
    let err = ron::from_str::<BarkSettings>(&format!("{known}, max_live: 2)"))
        .expect_err("`max_live` is not a pacing field")
        .to_string();
    assert!(
        err.contains("max_live"),
        "the diagnostic names the key: {err}"
    );
}
