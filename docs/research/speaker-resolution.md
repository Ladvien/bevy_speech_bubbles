# Speaker resolution: a narrator is quiet, an unknown key is a host error

Iteration 4, issue #11. A `Bark`, or a `Presenter::Balloons` line, names a speaker by string. The crate
resolves it through `Speakers`, and today `speaker_entity` returns the same `None` for two different
facts: a registered narrator (`entity: None`, no head to float over, documented and quiet) and a key that
is not in `Speakers` at all (a misspelt or not-yet-registered key, a host error). The bark and the
balloon-mode line drop both without a word.

## Sources

| Kind | Stem / source | Used for |
| --- | --- | --- |
| Model | `gameaipro3-ch37-simulating-character-knowledge-phenomena-talk` (Ryan et al., Game AI Pro 3 ch. 37) | a speech bubble belongs to the character who speaks it |
| Shipping game | `GameAIPro2_Chapter02_Combat_Dialogue_in_FEAR_The_Illusion_of_Communication` (Orkin, Game AI Pro 2 ch. 2) | a line that is not shown must be silent by design or loud; nothing in between |
| Technical | `book_metamorphic_testing_review_chen2018` (configuration-error injection study) | what a misconfiguration diagnostic must name, and why a reaction "without symptoms" is the unstudied failure |
| Technical | `kleppmann_designing_data_intensive_applications` ch. 1 | "configuration errors by operators were the leading cause of outages"; a system that can check itself "can constantly check itself while it is running and raise an alert if a discrepancy is found" |
| Technical | Bevy 0.20.1 source: `bevy_utils/src/once.rs`, `bevy_log/src/once.rs` | the scope of `warn_once!` (per call site) |

## Craft principle

The model source: Ryan et al. describe the dialogue layer's output as "displayed as speech bubbles
emitting from the characters who speak them". A balloon exists because a character, an entity in the
world, speaks it. The script says *who* by a string; the host says *which entity* through `Speakers`. A
string with no entry has no speaker at all, which is a different state from a registered speaker that
deliberately has no head (a narrator, a voice-over), and the crate has to be able to tell the host which
one it met.

## How shipping games do it

Orkin's FEAR chapter starts from its design rule: "If the AI didn't say it, it didn't happen." A bark is
how the player (and the designer reading a playtest) learns something happened. The same chapter shows a
silence that is by design: "When you land that headshot that takes out the enemy, he says ... nothing,
because he's dead. And dead men don't speak." That is the narrator case: the silence is documented, the
author chose it, nothing is wrong. A bark for a speaker the game never registered is the other silence:
nobody chose it, and the designer who wrote `"miler"` for `"miller"` sees no balloon and no reason. The
rule the rest of the kit follows (the dialog box's `speaker '{}' is not in Speakers`, the balloon-mode
`Choice` arm's warning, a missing `DialogSkin` or `BubbleAssets`) is that a host mistake is said once,
loudly, in words that name what to fix.

## Model adopted

- `resolve_speaker(&Speakers, key) -> Resolved` with three states: `Head(Entity)` (a balloon anchors to
  it), `Narrator` (registered, `entity: None`) and `Unknown` (not a key). It replaces the `Option` that
  conflated the last two, so each caller says which it meant.
- `Narrator` stays quiet everywhere it is quiet today: the bark is dropped, the balloon-mode line is
  skipped and the conversation moves on.
- `Unknown` gets a `warn!` that names what the configuration study says a diagnostic must name, "the
  misconfigured parameter's name/value or its location": the key, and the bark's text or the
  conversation and node it came from. The behaviour after the warning is unchanged (the bark is dropped
  and starts no gap; the line is skipped and the conversation ends on the cursor's own rules), so a
  registered-narrator script runs bit-identically and an unknown key now says why.
- A bark repeats: a table with one misspelt key fires it every time its trigger does, and Sanctuary's keys
  are per-entity (`person:<id>`), so a speaker that despawned between the trigger and the release is a
  new unknown key each time. `warn_once!` dedups per call site ("Call some expression only once per call
  site"), so it would hide the second misspelt key. The bark path keeps a per-key set in a
  `Local<HashSet<String>>` and warns once per key. A balloon-mode line needs no set: `present_current`
  runs once per node visit, and each visit names its own node.
- The set is bounded (64 keys, cleared when full) so a long game that mints a new per-entity key per
  despawned villager cannot grow it without limit; a cleared set can warn a key again, which is the
  quiet-but-not-lost direction.

## Parameters

| Name | Value | Why |
| --- | --- | --- |
| `MAX_WARNED_SPEAKERS` | 64 | design default: more distinct unknown keys than any authored bark table has typos; bounds the set |

## Validation targets

From the issue, through a headless `App` (`tests/walk.rs`), with a `tracing` layer that records WARN and
ERROR events on the test thread:

1. A bark for `"nobody"` gives exactly one WARN naming `nobody`, no `Bubble` and no panic.
2. A bark for the registered narrator gives no WARN.
3. `Presenter::Balloons`, a line by the unregistered `"ghost"`: one WARN naming `ghost` and the node,
   and `ConversationEnded` written once.
4. A line by the registered narrator, in the same setup: no WARN, one `ConversationEnded`.
5. Two barks for `"nobody"` warn once; a bark for a second unknown key warns again (per key, not per
   call site).

All of 1, 3 and 5 record no event on `main` (the issue's own reproduction).

## Deviations

None to a documented default: `CLAUDE.md` and the issue already say an unknown speaker gets a `warn!`.
The 64-key bound is a design default.

## What great looks like

A designer who misspells a speaker key in a bark RON file runs the game, sees no balloon, and finds one
line in the log: `bevy_speech_bubbles: bark speaker 'miler' is not in Speakers; dropping the bark ("Stuck!")`.
A script with a narrator and no heads runs silent, as before.

## Pending index

`kleppmann_designing_data_intensive_applications` and `book_metamorphic_testing_review_chen2018` are
indexed (found by `distill_search`). No dialogue-authoring-validation source (Yarn Spinner, Ink) is in
the corpus; the gap is recorded in `corpus.md`.
