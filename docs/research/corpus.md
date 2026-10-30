# Research corpus

The reading list per domain, by home-still stem (`distill_search` finds them; `markdown_read <stem>`
reads them). A source that is cited but not yet converted or indexed is listed under its domain with
`(pending)` and in the citing note's `## Pending index`. Add a line when a pass reads a new source;
never remove one.

## Conversation, turn-taking and dialogue structure (model)

- `10.1017_s0047404500001019` — Schegloff, *Overlapping talk and the organization of turn-taking for
  conversation*, Language in Society 2000. "One-party-at-a-time" as the design feature of talk;
  overlap resolution; turns allocated to *parties*, not necessarily to each co-incumbent of a party.
  Grounds the bark queue's one-balloon-at-a-time pacing and speaker aliases.
- `10.1017_s0140525x01003922` — Cowan, *The magical number 4 in short-term memory*, BBS 2001. Capacity
  limit for simultaneously held items; relevant to how many live balloons a player can track.
- `10.1609_aiide.v4i1.18682` — Strong & Mateas, *Talking with NPCs: towards dynamic generation of
  discourse structures*, AIIDE 2008. Hand-authored FSM/tree dialogue in shipped games (Mass Effect:
  ~28,000 lines, 12.5 words per line).
- `10.1017_s1351324900002497` — Hone & Graham, *Towards a tool for the Subjective Assessment of Speech
  System Interfaces (SASSI)*. Usability measures for spoken-dialogue interfaces.

## Barks and dialogue systems in shipped games (shipping game)

- `Game Engine Architecture, Third Edition, …` (Gregory 2018), §14.6.2 *Dialog*: voice-id lookup per
  character, "fire-and-forget" `PlayDialog()` (Uncharted 3), priority and interruption when a
  character is asked to speak while speaking or twice on one frame (The Last of Us).
- `GameAIPro2_Chapter02_Combat_Dialogue_in_FEAR_The_Illusion_of_Communication` — Orkin, barks turned
  into 2–3 line squad dialogues.
- `GameAIPro2_Chapter28_Modeling_Perception_and_Awareness_in_Tom_Clancy's_Splinter_Cell_Blacklist` —
  Walsh, three-tier bark specificity, conversation breakout and rejoin lines.
- `gameaipro3-ch38-procedural-level-story-generation-using-tag` — tag-based content selection versus
  Valve's rule-based dynamic dialog (Ruskin 2012) for barks.
- `Procedural Storytelling in Game Design -- Anna's Archive` — Valve-style rule database: repartee by
  follow-up events, round-robin replay policies for barks.

## Reading speed and legibility (model)

- (pending) Brysbaert, *How many words do we read per minute?*, JML 2019, DOI
  10.1016/j.jml.2019.104047 (preprint 10.31234/osf.io/xynwg). Grounds `dwell_secs`; no open-access
  PDF reachable (2026-10-30).

## Comics lettering and balloon placement (model)

- (pending) `10.3389_fpsyg.2013.00186` — Cohn, *Navigating comics: an empirical and theoretical
  approach to strategies of reading comic page layouts*, Frontiers in Psychology 2013. Downloaded
  2026-10-30, queued for conversion.
- `10.1145_212332.212334` — Christensen, Marks & Shieber, *An empirical study of algorithms for
  point-feature label placement*, ACM TOG 1995. Label overlap as an objective; relevant to balloons
  that keep out of each other's way.
- `10.48550_arxiv.1308.6801` — Bekos et al., *Many-to-one boundary labeling with backbones*, 2013.

## Engine technique: identifiers, allocation, per-frame paths (technical)

- `Game Engine Architecture, Third Edition, …` (Gregory 2018), §6.4.3 *Unique identifiers* and
  §6.4.3.2 *Some implementation ideas*: hashed string ids, intern once and keep the result rather
  than re-interning per call.
