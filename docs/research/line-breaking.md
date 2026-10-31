# Line breaking: a word longer than the line is broken, never spilled

Iteration 6, the backlog bug carried from iterations 3-5 (no filed issue). `wrap_lines` in
`src/balloon.rs` is a greedy wrap on whitespace. A word that is wider than the skin's maximum text
width has no break point, so it goes on a line of its own and that line sets the balloon's width.
Measured on iteration 3's probe: a single 60-character word makes the balloon 1269x136 px at `crt()` and
506x53 at `ink_on_paper()`, against 568 / 234 px for a wrapped 240-character line. The text stays inside
the cap (the padding is solved from the text width), but the balloon is more than twice the width the
skin promises, so it covers the scene it was meant to sit in front of. Text with no spaces (Chinese,
Japanese) is one "word" to this code, so any such line, however short, is a single unbroken run.

The screen dialog box has the same fault one layer up: its body text and choice rows are Bevy `Text`
with the default `TextLayout`, whose `LineBreak::WordBoundary` leaves an over-long word to run out of
the paper (Bevy 0.20, `bevy_text/src/text.rs`, `LineBreak`).

## Sources

| Kind | Stem / source | Used for |
| --- | --- | --- |
| Model | `10.1007_s10055-024-00949-6` (Cauz, Clarinval & Dumas, *Text readability in augmented reality: a multivocal literature review*, Virtual Reality 2024) | text anchored in a 3D world is read on short lines; the width cap is the legibility contract |
| Shipping game | `Game Engine Architecture, Third Edition` (Gregory) section 6.4, Naughty Dog's *The Last of Us* localization tool | a line of dialog reaches the renderer as a UTF-8 string in any of the game's languages, so layout cannot assume spaces between words |
| Technical | Unicode Standard Annex #14, *Unicode Line Breaking Algorithm* (unicode.org/reports/tr14), section "Introduction" | the emergency mode for a line with no break opportunity; keep grapheme clusters together |
| Technical | CSS Text Module Level 3 / MDN `overflow-wrap` (developer.mozilla.org/docs/Web/CSS/overflow-wrap) | the browser rule this adopts: a word is moved to a fresh line first, and only then broken |
| Technical | Bevy 0.20 source, `bevy_text/src/text.rs` (`LineBreak::WordOrCharacter`) and `bevy_text/src/pipeline.rs` | what the dialog box can ask of Bevy's own layout |

## Craft principle

The model source is a review of text legibility in head-mounted displays, where text is anchored in the
world (a "world-anchor") as a balloon is. Its summary of the grey literature: "Oculus (2023a,b)
recommends left-aligning the text without justifying it, making text lines short, and subdividing text
into sections." A balloon's maximum text width (`MAX_TEXT_W`, 13 em) is this crate's version of "making
text lines short", and `wrap_lines` is the code that has to keep it. A line that exceeds the cap is
therefore not a cosmetic overrun: it is the one place the contract is silently dropped, and it is
dropped by exactly the input (a long name, a URL, a German compound, a line of Chinese) a host cannot
see in the script it tested with.

## How shipping games do it

Gregory describes the dialogue path of a shipped engine (Naughty Dog's tooling): "If a line of dialog
must be played, we likewise look up the audio clip by its id and use the data in[ ]engine to look up its
corresponding subtitle (if any). The subtitle is treated just like a menu or HUD string, in that it is
returned by the localization tool's API as a UTF-8 string suitable for display." His figure caption is
"Text translations of the string 'NEW GAME' into all languages supported by Naughty Dog's *The Last of
Us*". The renderer is handed whatever the translator wrote, so a text layout has to bound its own
output: a game cannot ship a different balloon per language, and cannot rely on the author having seen
the line in the language that breaks it.

Browsers solve the identical problem with `overflow-wrap`. MDN: `break-word` is "the same as the
`anywhere` value, with normally unbreakable words allowed to be broken at arbitrary points if there are
no otherwise acceptable break points in the line". In practice the soft wrap opportunity before the word
is taken first, so the word starts a line of its own, and only a word that still does not fit is broken.

## Model adopted

- **Break an over-wide word at the last grapheme cluster that fits.** UAX #14: "text layout systems may
  support an emergency mode that handles the case of an unusual line that contains no otherwise
  permitted line break opportunities. In such line layout emergencies, line breaks may be placed with no
  regard to the ordinary line breaking behavior of the characters involved ... it is recommended that
  grapheme clusters be kept together." A cluster is what UAX #29 calls a user-perceived character
  (`e` + combining acute, a flag, an emoji ZWJ sequence), so the rule is `unicode-segmentation`'s
  `graphemes(true)`.
- **Fresh line first, then break** (CSS `break-word`). A word that fits on a line of its own is never
  broken: ordinary text wraps exactly as before. A word that does not fit starts a new line, is filled
  to the cap, and its last fragment stays on the current line so the next word continues after it.
- **The cap is `Metrics::max_text_w`, the same number the greedy wrap already uses.** No new parameter.
  A single cluster wider than the cap (a font with a 14 em ligature) takes a line to itself; every other
  line is at most the cap, so the balloon is never wider than a full line of ordinary text.
- **No hyphen is drawn.** CSS `break-word` draws none and a game's font may have no U+2010; the text on
  screen stays the text the author wrote.
- **Unspaced scripts fall out of the same rule.** UAX #14 puts a break opportunity between ideographs
  (class ID: "lines can ordinarily break before and after and between pairs of ideographic
  characters"), so breaking at the last cluster that fits is the UAX #14 result for them, apart from the
  prohibited positions (kinsoku) which the Annex leaves to local custom.
- **The dialog box asks Bevy for the same rule.** `TextLayout` with `LineBreak::WordOrCharacter` ("Wraps
  at the word level, or fallback to character level if a word can't fit on a line by itself") on the body
  text and the choice rows; Bevy maps it to parley's `OverflowWrap::Anywhere`. This is one path per
  feature: both presenters follow the one rule.

## Parameters

| Name | Value | Why |
| --- | --- | --- |
| line cap | `Metrics::max_text_w` (520 px at `text_px` 40, scaled; whole texels in pixel mode) | unchanged; the existing wrap width |
| break unit | grapheme cluster (UAX #29) | UAX #14: keep clusters together in emergency mode |
| hyphen glyph | none | CSS `break-word`; the font may lack one; the text is the author's |
| dependency | `unicode-segmentation` 1 (no dependencies of its own) | the standard UAX #29 implementation; design default |

## Validation targets

1. `wrap_lines` on a 60-character word (`W`), the same word between short words, and 40 unspaced
   ideographs: every line is at most the cap (to 1e-3 px), at `crt()` and `ink_on_paper()` metrics.
2. Ordinary text is unchanged: lines of a sentence with no over-wide word equal the old greedy result.
3. No cluster is split: `e` + U+0301, a flag, and a ZWJ family emoji are never separated across lines.
4. The word after a broken word continues on the last fragment's line.
5. A 60-`W` balloon (`rasterize`) is no wider than a wrapped 240-character balloon of the same kind
   and skin, its glyph ink sits inside the cap with the border clearance, and every texel at
   `ink_on_paper()` is still ink, border or background (0 in-between alpha values).
6. Both for `Speech` and `Thought`.
7. The dialog box: a screenshot of a boxed line with a 40-character word before and after: the word
   spills out of the paper before and stays inside it after.
8. The bench: no raster row regresses more than 5 % (the normal path adds one comparison per word).

## Deviations

- **A new runtime dependency, `unicode-segmentation` 1.** The crate had none beyond bevy, `ab_glyph`
  and `serde`. It is a pure-Rust table crate with no dependencies; it is in this repository's
  `Cargo.lock` only through a proc-macro (`convert_case`), so a consumer's runtime graph gains it. A
  hand-written "do not break before a combining mark" would be a partial UAX #29 that the Annex tells
  implementers to use whole. Design default. `tests/leaf.rs`'s `ALLOWED_DEPS` gains the name in the same
  commit (`assumed owner exception`, recorded in that commit's body and in the loop log): an additive
  entry, no assertion loosened.
- **Only the emergency break, not full UAX #14.** Breaking after a hyphen, before a closing quotation
  mark or between a digit and a unit are UAX #14 opportunities this does not implement: a long
  hyphenated compound breaks at the last cluster that fits, not after its last hyphen. Recorded in the
  loop log's backlog.

## What great looks like

A villager says `Donaudampfschifffahrtsgesellschaftskapitaen!` (45 letters) next to a short question. The
balloon is the same width as any other full balloon in the scene, the word runs over three lines inside
the cap, nothing touches the border, and at `ink_on_paper()` the glyphs are still crisp whole texels.
In the screen box the same word wraps inside the paper. A line of Chinese wraps at the cap like
English prose. Nobody who tested with short English words sees any difference.

## Pending index

No text-layout, line-breaking or typography source is indexed in the corpus (`distill_search` for
"line breaking", "Knuth Plass", "UAX 14", "line length reading speed" returned unrelated documents), so the
two technical sources are published specifications read from the web, not stems. Index the Unicode
annexes #14 and #29 and a legibility handbook when a pass can fetch them as documents. Not
obtainable here: Knuth & Plass, *Breaking Paragraphs into Lines* (1981), the model for the optimal wrap
this crate does not do.
