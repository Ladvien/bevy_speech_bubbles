//! The one plugin: registers the messages and resources and schedules every system, generic over the
//! camera marker the balloons face.

use std::marker::PhantomData;

use bevy::prelude::*;

use crate::balloon::{DefaultBalloonSkin, expire_bubbles, track_bubbles};
use crate::conversation::{
    Bark, BarkQueue, BarkSettings, ChoicePicked, ConversationEnded, DialogueScript, Presenter,
    Speakers, StartConversation, advance_line, choice_hotkeys, open_conversation, present_current,
    queue_barks, release_bark, resolve_choice,
};
use crate::dialog_box::{
    ChoiceFocus, animate_box, caret_blink, choice_focus_input, paint_choices, present_dialog_box,
    typewriter_tick,
};

/// Dialog kit plugin. `C` is the marker component of the camera the balloons face — name a marker you
/// control (see [`track_bubbles`]); the crate never guesses one.
///
/// Registers [`StartConversation`], [`ChoicePicked`], [`ConversationEnded`] and [`Bark`], and inserts
/// [`Speakers`], [`DialogueScript`], [`BarkQueue`], [`BarkSettings`], [`DefaultBalloonSkin`],
/// [`ChoiceFocus`] and [`Presenter::Balloons`] (insert your own after adding the plugin to override).
/// Everything runs in `Update`, chained. It loads no font and no script: the host inserts
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
            .add_message::<Bark>()
            .init_resource::<Speakers>()
            .init_resource::<DialogueScript>()
            .init_resource::<BarkQueue>()
            .init_resource::<BarkSettings>()
            .init_resource::<DefaultBalloonSkin>()
            .init_resource::<ChoiceFocus>()
            .insert_resource(Presenter::Balloons)
            .add_systems(
                Update,
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
