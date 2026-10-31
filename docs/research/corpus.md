# Research corpus: the reading list per domain

What this crate's research notes cite, and what the home-still corpus holds for each domain a dialog kit
touches. A stem is the home-still document id (`distill_search` / `markdown_read`); a DOI without a stem
is cited but not in the corpus. Add a row when a pass reads a new source; mark what is pending.

## Dialogue structure, barks and conversation design (shipping games)

| Stem / DOI | Source | Notes |
| --- | --- | --- |
| `GameAIPro2_Chapter02_Combat_Dialogue_in_FEAR_The_Illusion_of_Communication` (also `Orkin_FEAR_Combat_Dialogue_GAIP2`, `gameaipro2-ch02-combat-dialogue-fear-illusion-communication`) | Orkin, *Combat Dialogue in FEAR: The Illusion of Communication*, Game AI Pro 2 ch. 2 | barks turned into 2–3 line dialogues; "If the AI didn't say it, it didn't happen" |
| `gameaipro3-ch37-simulating-character-knowledge-phenomena-talk` | Ryan et al., *Simulating Character Knowledge Phenomena in Talk of the Town*, Game AI Pro 3 ch. 37 | NPC dialogue as speech bubbles emitted from the speakers; eavesdropping on background talk |
| `GameAIPro2_Chapter11_Smart_Zones_to_Create_the_Ambience_of_Life` (also `gameaipro2-ch11-smart-zones-create-ambience-life`) | Game AI Pro 2 ch. 11, *Smart Zones to Create the Ambience of Life* | ambient "living scenes"; roles for main, supporting and extra characters |
| `GameAIPro2_Chapter28_Modeling_Perception_and_Awareness_in_Tom_Clancy's_Splinter_` | Game AI Pro 2 ch. 28, Splinter Cell perception | NPCs announcing what they noticed |
| `Procedural Storytelling in Game Design -- Anna's Archive` | Short & Adams (eds.), *Procedural Storytelling in Game Design* | narrative structure, environmental text |

## Comics lettering and speech balloons (model)

| Stem / DOI | Source | Notes |
| --- | --- | --- |
| `10.48550_arxiv.2306.17469` (downloaded 2026-10-30, pending conversion) | Li et al., *Manga109Dialog: A Large-scale Dialogue Dataset for Comics Speaker Detection* | proximity ("shortest distance") attributes 71.43 % of easy balloon–speaker pairs |
| `10.3390_jimaging4070087` (downloaded 2026-10-30, pending conversion) | Augereau et al., *A Survey of Comics Research in Computer Science*, J. Imaging 2018 | survey: balloon detection, speaker association, generation |
| arXiv 2408.06294 (not in corpus) | An et al., *AniBalloons* | balloon colour carries emotion (`examples/chatter.rs`) |
| 10.1109/ICDAR.2015.7333782 (not obtainable) | Rigaud et al., *Speech balloon and speaker association for comics and manga understanding*, ICDAR 2015 | tail and anchor point association |
| 10.1145/237170.237260 (not obtainable) | Kurlander, Skelly & Salesin, *Comic Chat*, SIGGRAPH 1996 | automatic balloon layout over speakers |
| 10.1145/2505483.2505486 (not obtainable) | Chu & Yu, *Optimized speech balloon placement for automatic comics generation* | balloon placement as optimisation |

Gap: no lettering handbook (Eisner, McCloud, Cohn) is in the corpus.

## Reading speed and legibility (model)

| Stem / DOI | Source | Notes |
| --- | --- | --- |
| 10.1016/j.jml.2019.104047 (cited by `dwell_secs`) | Brysbaert, *How many words do we read per minute?*, JML 2019 | ~238 wpm silent mean; the crate uses 200 wpm and a 1.8 s floor |
| `10.1145_3361218` | Sidenmark & Gellersen, *Eye, Head and Torso Coordination During Gaze Shifts in Virtual Reality*, TOCHI 2019 | text attached to a speaker; gaze shifts between text and speaker |

## Game UI, diegesis and placement

| Stem / DOI | Source | Notes |
| --- | --- | --- |
| `10.1145_2793107.2793120` | Kennedy et al., *Removing the HUD*, CHI PLAY 2015 | diegetic vs non-diegetic elements; Dead Space 2's diegetic UI |
| `10.1109_CCNC.2016.7444811` | Casanova et al., *Assessing the Efficacy of a Diegetic Game Interface with Oculus Rift*, CCNC 2016 | diegetic interface field trial |
| `10.1145_212332.212334` | Christensen, Marks & Shieber, *An Empirical Study of Algorithms for Point-Feature Label Placement* | label overlap objective; for balloons that keep out of each other's way |
| `10.48550_arxiv.1308.6801` | Bekos et al., *Many-to-One Boundary Labeling with Backbones* | leader-line labelling |

## Transforms, scene graphs and engine practice (technical)

| Stem / DOI | Source | Notes |
| --- | --- | --- |
| `Game Engine Architecture, Third Edition, 3rd Edition -- Jason Gregory -- …` | Gregory, *Game Engine Architecture* 3e | §5.3.10 coordinate-space hierarchy; §12.3.2 local poses |
| `book_real_time_rendering_4th_akenine` | Akenine-Möller et al., *Real-Time Rendering* 4e | ch. 19 scene graphs: transforms composed root to leaves |
| Bevy 0.20 source (vendored) | `bevy_transform` (`TransformHelper`, propagation), `bevy_camera` (`Visibility`) | the rules the crate's balloons must agree with |

## Diagnostics and configuration errors (technical)

| Stem / DOI | Source | Notes |
| --- | --- | --- |
| `book_metamorphic_testing_review_chen2018` | configuration-error injection study (metamorphic-testing review, Chen 2018) | a diagnostic names "the misconfigured parameter's name/value or its location"; a reaction with no symptom is the unstudied failure |
| `kleppmann_designing_data_intensive_applications` | Kleppmann, *Designing Data-Intensive Applications* ch. 1 | configuration errors by operators are the leading cause of outages; a system can check itself and raise an alert |
| Bevy 0.20.1 source (vendored) | `bevy_utils/src/once.rs`, `bevy_log/src/once.rs` | `warn_once!` is "once per call site", not per key |
| Yarn Spinner documentation, *Errors* (web, `docs.yarnspinner.dev`; not a corpus stem) | Yarn Spinner error codes | an unknown command, node, enum member or character is a reported error with a code and a line (YS0012, YS0014, YS0016, YS0025), never a silent default; read in iteration 5 |
| serde.rs *Container attributes*; `serde_derive-1.0.229`, `ron-0.12.3` (vendored) | `deny_unknown_fields` | an unknown key is skipped as `IgnoredAny` unless the container refuses it; RON accepts a bare identifier there; read in iteration 5 |
| `Procedural Storytelling in Game Design -- Anna's Archive` | Short & Adams (eds.), ch. 10 on *The Sims 4* | "If players can't see NPC traits or interact with them, then they effectively don't exist"; speech and thought balloons over the speaker |

Gap: no dialogue-authoring tool documentation (Yarn Spinner, Ink) is in the corpus; Yarn Spinner's error
table was read from its published page, and Ink's compile errors are unsurveyed. Index the Yarn Spinner
and Ink manuals when a pass can fetch them as documents.

## Glyph rasterization and text layout (technical)

Not yet surveyed in the corpus: SDF text (Green 2007), coverage rasterizers, UAX #14 line breaking,
Knuth–Plass. The first pass that touches `rasterize` or wrapping searches for them.
