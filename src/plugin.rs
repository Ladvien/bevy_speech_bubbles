//! The one plugin: registers the messages and resources and schedules every system, generic over the
//! camera marker the balloons face.

use std::marker::PhantomData;

use bevy::input::InputSystems;
use bevy::prelude::*;

use crate::balloon::{BalloonScale, DefaultBalloonSkin, expire_bubbles, track_bubbles};
use crate::conversation::{
    Bark, BarkQueue, BarkSettings, ChoicePicked, ConversationEnded, DialogueNodeEntered,
    DialogueScript, Presenter, Speakers, StartConversation, advance_line, choice_hotkeys,
    open_conversation, present_current, queue_barks, release_bark, resolve_choice,
};
use crate::dialog_box::{
    ChoiceFocus, animate_box, caret_blink, choice_focus_input, paint_choices, present_dialog_box,
    typewriter_tick,
};

/// The conversation runtime's input-reading and state-advancing systems (`PreUpdate`, after
/// [`InputSystems`]). They consume the key and click presses they use, so a host system ordered
/// `.after(DialogInputSet)` in `PreUpdate` no longer sees them as `just_pressed`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct DialogInputSet;

/// Dialog kit plugin. `C` is the marker component of the camera the balloons face — name a marker you
/// control (see [`track_bubbles`]); the crate never guesses one.
///
/// Registers [`StartConversation`], [`ChoicePicked`], [`ConversationEnded`], [`DialogueNodeEntered`] and [`Bark`], and inserts
/// [`Speakers`], [`DialogueScript`], [`BarkQueue`], [`BarkSettings`], [`DefaultBalloonSkin`], [`BalloonScale`],
/// [`ChoiceFocus`] and [`Presenter::Balloons`] (insert your own after adding the plugin to override).
/// The runtime runs in `PreUpdate` ([`DialogInputSet`]) and consumes the presses it uses; the cosmetic
/// systems (typewriter, caret, balloon tracking and expiry) run in `Update`. It loads no font and no script: the host inserts
/// [`BubbleAssets`](crate::BubbleAssets) and fills [`DialogueScript`] and [`Speakers`]; a
/// [`Presenter::DialogBox`] also needs a [`DialogSkin`](crate::DialogSkin).
pub struct SpeechBubblesPlugin<C: Component>(PhantomData<fn() -> C>);

impl<C: Component> Default for SpeechBubblesPlugin<C> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<C: Component> Plugin for SpeechBubblesPlugin<C> {
    fn build(&self, app: &mut App) {
        app.add_message::<StartConversation>()
            .add_message::<ChoicePicked>()
            .add_message::<ConversationEnded>()
            .add_message::<DialogueNodeEntered>()
            .add_message::<Bark>()
            .init_resource::<Speakers>()
            .init_resource::<DialogueScript>()
            .init_resource::<BarkQueue>()
            .init_resource::<BarkSettings>()
            .init_resource::<DefaultBalloonSkin>()
            .init_resource::<BalloonScale>()
            .init_resource::<ChoiceFocus>()
            .insert_resource(Presenter::Balloons)
            // The runtime reads input and moves the conversation along in `PreUpdate`, right after
            // Bevy's input systems, and consumes the presses it uses (`clear_just_pressed`). A host
            // orders its own `PreUpdate` systems `.after(DialogInputSet)` and never sees a key the
            // dialog took. Real time, so a paused virtual clock does not matter.
            .add_systems(
                PreUpdate,
                (
                    open_conversation,
                    // Input before `resolve_choice`, so a digit or `Enter` pressed this frame
                    // resolves this frame.
                    choice_hotkeys,
                    choice_focus_input,
                    resolve_choice,
                    advance_line,
                    present_current,
                    present_dialog_box,
                    queue_barks,
                    release_bark,
                )
                    .chain()
                    .in_set(DialogInputSet)
                    .after(InputSystems),
            )
            // Everything cosmetic stays in `Update`: balloons must follow their owner after the
            // host's movement, so they track in `Update` like any billboard.
            .add_systems(
                Update,
                (
                    typewriter_tick,
                    paint_choices,
                    caret_blink,
                    animate_box,
                    track_bubbles::<C>,
                    expire_bubbles,
                )
                    .chain(),
            );
    }
}
