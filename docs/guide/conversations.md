# Conversations, barks and presenters

Moved from the README on 2026-10-31, unchanged, when the README was brought to the [org documentation guide](https://github.com/bevy-coven/.github/blob/main/DOCUMENTATION.md).

```rust,ignore
app.add_plugins(SpeechBubblesPlugin::<MyCamera>::default())
    .insert_resource(Presenter::DialogBox) // or Presenter::Balloons, the default
    .insert_resource(my_dialog_skin)       // a DialogSkin built from your theme
    .insert_resource(my_script);           // a DialogueScript, validated

// Balloons need a font (yours to load) and a quad:
commands.insert_resource(BubbleAssets {
    quad: meshes.add(Rectangle::new(1.0, 1.0)),
    font: FontArc::try_from_vec(std::fs::read("assets/fonts/Something.ttf")?)?,
});

speakers.0.insert("keeper".into(), Speaker { entity: Some(keeper), name: "The Keeper".into(), portrait: None });
starts.write(StartConversation { id: "gate".into() });
barks.write(Bark { speaker: "keeper".into(), kind: BubbleKind::Speech, emotion: Emotion::Neutral, text: "Mind the step.".into() });
```

While a conversation runs the resource `ConversationLock` exists: pause on it, and yield your own click handling to it. A script looks like this (RON; any serde format works):

```ron
(conversations: {
  "gate": (start: "greet", nodes: {
    "greet": Line(speaker: "keeper", text: "The gate has held three nights.", next: Some("ask")),
    "ask": Choice(speaker: "keeper", prompt: "Take the watch?", options: [
      (text: "I will.", next: "yes"),
      (text: "Ask someone else.", next: "no"),
    ]),
    "yes": Line(speaker: "keeper", text: "Good.", next: None),
    "no": Line(speaker: "keeper", emotion: Sadness, text: "Then I will ask the boy.", next: None),
  }),
})
```

A script, a `Bark` and a `BarkSettings` are refused when they carry a key the schema does not have: `emtion: Fear` on a line is a parse error naming the key, the type and the spellings it could have been (`2:48-2:54: Unexpected field named `emtion` in `Line`, expected one of `speaker`, `kind`, `emotion`, `text`, or `next` instead`, with `ron`), not a balloon quietly drawn in the default tint; likewise a `ttl` on a bark or a `max_live` in bark pacing. `emotion` and `kind` stay optional, so a line that omits them still loads. `validate_script` then checks the graph (a dangling `next`, a missing start node, a choice with no options).

`Presenter::Balloons` puts every line over its speaker's head and auto-advances after a reading-speed dwell; a speaker with no entity (a narrator) is skipped quietly, while a key that is not in `Speakers` at all (a misspelt or not-yet-registered one) is skipped with a `warn!` naming it: once per key for a bark, once per node for a line. `Presenter::DialogBox` shows the screen box and waits for the player: `Space`, `Enter` or a click completes a typing line and then advances it; `1`-`9`, the arrows with `Enter`, or the mouse pick an option. Barks are always world balloons.
