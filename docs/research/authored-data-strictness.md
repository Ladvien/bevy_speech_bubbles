# Authored data is strict: a misspelt field is an error, never a default

Iteration 5, issue #12. The crate's authored-data types disagree about `#[serde(deny_unknown_fields)]`:
`Choice`, `Conversation` and `DialogueScript` refuse a field they do not know; `Node`, `Bark` and
`BarkSettings` skip it. `Node::Line`'s `kind` and `emotion` and `Node::Choice`'s `emotion` are
`#[serde(default)]`, so `Line(speaker: "scout", kind: Thought, emtion: Fear, ...)` parses, passes
`validate_script` (which sees a `DialogueScript`, not the text) and draws a Neutral balloon; `knid:
Thought` draws a speech balloon; `Bark { ttl: 5.0 }` and `BarkSettings { max_live: 2 }` drop the field
without a word. bevy_grimoire's editor (bevy-coven/bevy_grimoire#106) writes these as hot-reloaded RON, so the
author who typed the typo is the one who sees nothing.

## Sources

| Kind | Stem / source | Used for |
| --- | --- | --- |
| Model | `Procedural Storytelling in Game Design -- Anna's Archive` (Short & Adams eds., ch. 10, *The Sims 4*); An et al., *AniBalloons*, arXiv:2408.06294 (cited in `src/style.rs`, not in the corpus) | `kind` and `emotion` are the balloon's semantic channels; a channel the player cannot see "effectively" is not there |
| Shipping game | Yarn Spinner documentation, *Errors* (`docs.yarnspinner.dev/write-yarn-scripts/yarn-spinner-editor/errors`); `GameAIPro2_Chapter02_Combat_Dialogue_in_FEAR_The_Illusion_of_Communication` (Orkin) | a name the script uses that the tool does not know is a reported error, not a default |
| Technical | `book_metamorphic_testing_review_chen2018` (configuration-error injection study) | a misconfiguration the system accepts without a diagnostic is a "silent resolution" |
| Technical | `kleppmann_designing_data_intensive_applications` ch. 8 | guard against "weak forms of lying", including misconfiguration, with "simple and pragmatic steps" |
| Technical | serde.rs *Container attributes*; `serde_derive-1.0.229/src/de/struct_.rs:580-586`; `ron-0.12.3/src/de/mod.rs:917-922` | what `deny_unknown_fields` does on an enum's struct variants, and why RON accepts the typo without it |

## Craft principle

The model source: Short & Adams, on what a player can and cannot use in *The Sims 4*: "If players can't
see NPC traits or interact with them, then they effectively don't exist." The same chapter lists "speech
and thought balloons over the Sim's heads when they talk" among the things that make a character
watchable. A balloon's shape (`BubbleKind`) and tint (`Emotion`) are the kit's two authored channels
(`style.rs` grounds the tint in An et al.: balloon colour reliably conveys emotion). An author who writes
`emotion: Fear` has decided what the player must feel; when the key is misspelt the decision never
reaches the screen and nothing says so. The failure is not a wrong balloon, it is an author who cannot
tell, from the game, that the script they wrote is not the script that ran.

## How shipping games do it

Yarn Spinner's published error table is the shape of the rule. Every name a script uses that the tool
does not know is an error with a code and a line, not a quiet default: "YS0014 Invalid or unknown
command. A command isn't recognised. Check spelling or make sure it's defined in your definitions
file."; "YS0012 Jump to undefined node. A `<<jump>>` or `<<detour>>` targets a node that doesn't
exist."; "YS0025 Unknown enum member. The value isn't a known member of the expected enum type."; and,
as a warning, "YS0016 Unknown character. A character name isn't defined in the project." Orkin's rule
for FEAR's dialogue ("If the AI didn't say it, it didn't happen") is why a designer reading a playtest
has to be able to trust that what the script says is what was shown. This crate already follows the
rule for values (`emotion: Fer` is an unknown variant and fails), for graph edges (`validate_script`:
`next 'zz' not found`) and for speakers (iteration 4); the unknown *key* is the one name left unchecked.

## Model adopted

- `#[serde(deny_unknown_fields)]` on `Node`, `Bark` and `BarkSettings`. serde applies an enum's container
  attribute to its struct variants (`enum_externally.rs` hands the enum's attributes to the struct
  deserializer), so one attribute on `Node` covers `Line` and `Choice`. Without it the generated visitor
  matches an unknown key to `__ignore` and consumes the value as `IgnoredAny`
  (`serde_derive-1.0.229/src/de/struct_.rs:580-586`); RON 0.12 accepts any value there (a bare `Fear`
  becomes `visit_unit`, `ron-0.12.3/src/de/mod.rs:917-922`), which is why the parse succeeds.
- No new error type and no change to `validate_script`: the check belongs to the parse, because after the
  parse the typo is gone. serde's own message already carries what Chen et al. say a diagnostic must:
  `unknown field `emtion`, expected one of `speaker`, `kind`, `emotion`, `text`, `next``, and RON adds
  the line and column. The tests assert the message names the offending key.
- Nothing else about the schema changes. `kind` and `emotion` stay optional on `Node`; `Bark`'s fields
  stay required. Serialized output contains only known fields, so every script and bark a host already
  wrote with this crate's own types still loads.
- All six serde types of the crate that carry fields (`Choice`, `Conversation` and `DialogueScript`
  already; now `Node`, `Bark` and `BarkSettings`) then refuse an unknown key, and `BubbleKind` and
  `Emotion` are unit enums that refuse an unknown variant by construction.

## Technical sources, quoted

Chen et al.'s configuration-error injection study classifies a system's reaction to an injected bad
value, and names the failure this issue is: "we inspect the source code and test code to find if it is a
silent resolution or inadequate tests." A parse that drops `emtion` is a silent resolution. Kleppmann
(*Designing Data-Intensive Applications*, "Weak forms of lying"): "it can be worth adding mechanisms to
software that guard against weak forms of “lying”—for example, invalid messages due to hardware issues,
software bugs, and misconfiguration ... they are nevertheless simple and pragmatic steps toward better
reliability." A one-line attribute is that kind of step. serde.rs documents the attribute: "`#[serde(deny_unknown_fields)]`
Always error during deserialization when encountering unknown fields. When this attribute is not present,
by default unknown fields are ignored for self-describing formats like JSON." (It is not supported with
`flatten`; none of these types flattens.)

## Parameters

None: no number, default or threshold changes.

## Validation targets

From the issue, parsed with `ron` as a host would. Each fails on `main` (the four `is_err()` assertions
in the issue) and passes after:

1. `Line(speaker: "s", emtion: Fear, text: "hi", next: None)` is an error, and the message names `emtion`.
2. `Choice(speaker: "s", prompt: "?", emotoin: Anger, options: [...])` is an error.
3. `Line(speaker: "s", knid: Thought, text: "hm", next: None)` is an error.
4. `(speaker: "m", kind: Speech, emotion: Anger, text: "x", ttl: 5.0)` does not load as a `Bark`.
5. `(gap: 1.0, stale: 3.0, queue_cap: 4, max_live: 2)` does not load as `BarkSettings`.
6. The correct spellings load unchanged: a script with `kind` and `emotion` omitted, a script that
   round-trips through RON, a bark and bark pacing that round-trip (the existing tests keep passing).

## Deviations

None to a documented default. The change makes `Node`, `Bark` and `BarkSettings` agree with the
documented behaviour of `Choice`, `Conversation` and `DialogueScript`. A host whose RON carries an
unknown key that used to be ignored now gets a parse error; that is the intent of the issue, and it is
noted in the changelog.

## What great looks like

A designer types `emtion: Fear` in a hot-reloaded conversation file. The reload fails at once with
`unknown field `emtion`, expected one of ...` and the position of the key, naming both the typo and the
legal spellings, and the previous good script stays in force. No balloon is ever drawn in the wrong
colour because of a spelling. The same holds for a bark file with a `ttl` the kit does not have and a
pacing file with a `max_live` the kit does not have.

## Pending index

No dialogue-authoring-tool documentation (Yarn Spinner, Ink) is in the home-still corpus; the Yarn
Spinner error table above was read from its published page (`docs.yarnspinner.dev`), not from a stem.
`10.48550_arxiv.2306.17469` and `10.3390_jimaging4070087` still await conversion (carried from
`balloon-anchoring.md`). `catalog_recent` (limit 200) lists no dialogue, comics, legibility, text-layout
or game-UI document newer than `corpus.md`, so there is no `Research: corpus` commit this pass.
