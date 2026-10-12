//! **A dialog kit for Bevy.**
//!
//! Three things a game says words with, on one conversation model:
//!
//! * **World balloons** ([`balloon`]): text rasterized on the CPU into an `Image`, put on a unit quad as
//!   an unlit alpha-blended material, anchored above an owner entity and turned to face a camera. No 2D
//!   camera, no UI layer — the balloon lives in the world, occludes and is occluded like anything
//!   else, and moves with the thing that said it. A [`BalloonSkin`] sets the palette, and `pixel: true`
//!   draws it without anti-aliasing for pixel-art games.
//! * **A conversation graph** ([`conversation`]): authored lines and choices ([`DialogueScript`],
//!   serde-loadable from YAML or RON), a runtime that walks them ([`StartConversation`],
//!   [`ChoicePicked`], [`ConversationEnded`], [`ConversationLock`]), and paced one-off [`Bark`]s.
//! * **A screen dialog box** ([`dialog_box`]): a Bevy UI surface skinned by the host's [`DialogSkin`],
//!   with a typewriter, a blinking caret, a name tag, a portrait and clickable, keyboard-navigable
//!   choices. [`Presenter`] picks balloons or box for conversations; barks are always balloons.
//!
//! Add [`SpeechBubblesPlugin`] and the three are wired. The crate knows no game: speakers are strings
//! the host resolves through [`Speakers`], and it never names a game's camera.
//!
//! # Why the balloon text is rasterized here
//!
//! Bevy's text stack has no public "rasterize this string into a standalone `Image`" API, and a
//! world-space balloon has no 2D camera to host a `Text2d`. So the glyphs go through `ab_glyph` into an
//! RGBA buffer, once per line rather than per frame, and the buffer becomes a texture.
//!
//! # Name your camera
//!
//! [`SpeechBubblesPlugin`] and [`track_bubbles`] are generic over a camera **marker component**, and
//! that is deliberate. Bevy's `Single<..>` silently skips its system when the query does not match
//! exactly one entity — so a system filtered on `With<Camera3d>` works perfectly until the day
//! something spawns a second 3D camera, at which point every balloon quietly stops tracking and
//! nothing errors. Being generic makes it impossible for this crate to express that filter.
//!
//! ```ignore
//! app.add_plugins(bevy_speech_bubbles::SpeechBubblesPlugin::<MyMainCamera>::default());
//! ```
//!
//! # The font is yours to load
//!
//! [`BubbleAssets`] holds a quad mesh and a `FontArc`, and the crate never reads a file: a library that
//! assumed an asset path would be assuming your project layout. The dialog box takes its fonts as
//! `TextFont`s in the [`DialogSkin`].
//!
//! # Cosmetic by construction
//!
//! Everything runs on `Time<Real>`, so balloons still expire and the typewriter still types while the
//! simulation is paused — a modal conversation that zeroed virtual time would otherwise freeze its
//! own dialogue on screen forever. Hosts pause on [`ConversationLock`].
//!
//! The conversation runtime (input, node walking, presenting, barks) runs in `PreUpdate` after
//! `InputSystems`, in [`DialogInputSet`], and **consumes the presses it uses** with
//! `clear_just_pressed`: the Space that ends a line is gone for any host system ordered
//! `.after(DialogInputSet)`, so it cannot also throw a stone. The cosmetic systems (typewriter, caret,
//! pop animation, balloon tracking and expiry) stay in `Update`, after the host's movement.

pub mod balloon;
pub mod conversation;
pub mod dialog_box;
pub mod plugin;
pub mod style;

pub use balloon::{
    BUBBLE_ANCHOR_Y, BalloonSkin, Bubble, BubbleAssets, BubbleStyle, BubbleTtl, DefaultBalloonSkin,
    RenderedBubble, build_bubble, dwell_secs, expire_bubbles, track_bubbles,
};
pub use conversation::{
    Bark, BarkQueue, BarkSettings, Choice, ChoicePicked, Conversation, ConversationEnded,
    ConversationLock, DialogueScript, Node, Presenter, Speaker, Speakers, StartConversation,
    validate_script,
};
pub use dialog_box::{ChoiceFocus, DEFAULT_CHARS_PER_SECOND, DIALOG_Z, DialogSkin};
pub use plugin::{DialogInputSet, SpeechBubblesPlugin};
pub use style::{BubbleKind, Emotion};
