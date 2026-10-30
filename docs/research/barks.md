# Barks: pacing, and one voice under several speaker keys

The bark layer is `Bark` → `queue_barks` → `BarkQueue` → `release_bark` (`src/conversation.rs`): one
global FIFO, released at most one balloon per `BarkSettings::gap`, aged out after `stale`, capped at
`queue_cap`, and a new bark over an entity replaces the one already over it. This note covers the
pacing model and speaker aliases (issue #5): two speaker keys that are one person.

## Sources

- **Model:** Schegloff, *Overlapping talk and the organization of turn-taking for conversation*,
  Language in Society 29 (2000), stem `10.1017_s0047404500001019`.
- **Shipping game:** Gregory, *Game Engine Architecture*, 3rd ed. (2018), §14.6.2.3–14.6.2.4 (Naughty
  Dog's dialog system in Uncharted 3 and The Last of Us), stem `Game Engine Architecture, Third
  Edition, …`. Also Orkin, *Combat Dialogue in FEAR* (Game AI Pro 2, ch. 2).
- **Technical:** Gregory, §6.4.3 *Unique identifiers* and §6.4.3.2 (hashed string ids, interning
  once), same stem.

## Craft principle

Conversation is organised so that one party talks at a time, and a listener reads simultaneous talk as
a departure to be repaired, not as information:

> "Talk by more than one person at a time in the same conversation is one of the two major departures
> that occur from what appears to be a basic design feature of conversation, and of
> talk-in-interaction more generally, namely “one at a time” (the other departure is silence, i.e.
> fewer than one at a time)." (Schegloff 2000, Prolegomena)

and the unit a turn goes to is the *party*, which need not be one name:

> "they are systematic products of a turn-taking organization which allocates turns to parties, but
> not necessarily among party co-incumbents." (Schegloff 2000, endnotes, on overlap between spouses)

For a bark kit this is two rules. Balloons over the world appear one at a time (the global gap), and
the unit that may hold only one balloon is the person on screen, however many keys the host's systems
address that person by. Two balloons over one head read as two people talking over each other.

## How shipping games do it

Naughty Dog's dialog system resolves a logical line through the character's identity, then arbitrates
per character:

> "Given this data, the dialog system can easily convert a request for a logical line of dialog such
> as 'line-out-of-ammo' into a specific audio clip. It simply looks up the character's specific voice
> id in the table, and then makes a random choice amongst the various possible lines for that
> character." (Gregory 2018, §14.6.2.3)

> "What happens if a character is asked to speak while he's already speaking? What if he receives
> more than one speech command on the same frame? In both cases, a priority system is a good way to
> resolve ambiguities. […] The dialog system in The Last of Us doesn't do any of these fancy things.
> It simply stops the current line and immediately plays the new one. This sounds pretty good most of
> the time." (Gregory 2018, §14.6.2.4)

and keeps the caller's side "fire-and-forget":

> "If an AI programmer or game designer has to jump through a lot of hoops just to get a line of
> dialog to play, you may discover that your characters are uncannily silent! It's best to provide a
> simple, fire-and-forget interface." (Gregory 2018, §14.6.2.3)

This crate already does the arbitration The Last of Us does (a new bark over an entity replaces the
old one) and adds a global gap. What it lacks is the identity table: a host whose two systems address
one villager as `person:7` and `society:7` must register two `Speaker` records and keep their entity,
name and portrait in step.

## Model adopted

- `Speakers` keeps its public key → `Speaker` map and gains a private alias table:
  `Speakers::alias(alias, target)` makes `alias` speak as `target`; `Speakers::unalias(alias)` forgets
  it; `Speakers::get(key)` is the one resolver every lookup in the crate goes through (a
  conversation line's balloon, the dialog box's name tag and portrait, a bark).
- **One hop.** `get` looks an alias up once and then reads the target's own registration; it never
  follows the target's alias. A chain `a → b → c` resolves `a` to `b`'s record, and a cycle is
  unrepresentable.
- **The alias wins.** A key that is both registered and aliased resolves to its target: the alias is
  the host's explicit statement that this key is someone else (bevy_society registers `society:<id>`
  on its own, and the alias must override that to unify the name tag).
- **Barks resolve when they are queued** (`queue_barks`), as the issue asks: the queue holds only real
  keys, and re-pointing an alias does not move a bark already waiting. Release reads the resolved key
  directly, without a second hop.
- **A removed target silences its aliases.** `Speakers.0.remove(target)` stays the one removal path;
  while the target is absent the alias resolves to nothing, so its barks are dropped like an unknown
  speaker's and the gap is not started. Registering the target again resumes the alias.
- Write-time resolution is one `HashMap` lookup per bark (Gregory §6.4.3.2: intern once, keep the
  result); nothing runs per frame.

## Parameters

| Parameter | Value | Source |
|---|---|---|
| `BarkSettings::gap` | 1.0 s (unchanged) | design default; `BarkSettings` rustdoc (Stivers et al. 2009) |
| `BarkSettings::stale` | 3.0 s (unchanged) | design default |
| `BarkSettings::queue_cap` | 4 (unchanged) | design default |
| alias hops | 1 | issue #5 ("does not follow chains") |

## Validation targets

1. `alias("b", "a")`, barks for `b` and `a` on one frame: at most one bark balloon is live at any
   frame, the second appears no earlier than `gap` after the first, and both float over `a`'s entity.
2. An aliased bark anchors at the target's entity even when the alias key is registered to another
   entity of its own.
3. An alias whose target is absent: no balloon, no panic, and the next bark is not delayed.
4. Two keys on one entity with no alias: one balloon at a time over that head (main's behaviour,
   pinned).
5. Re-pointing an alias after its bark is queued does not move the bark.
6. A conversation line by an alias key shows the target's name on the dialog box's name tag.
7. Bench: no case regresses over 5%; the 20-bark burst's worst frame and the 200-balloon frame do not
   regress; `Assets<Image>` and `Assets<StandardMaterial>` return to their start after 1000 barks.

## What great looks like

A villager two systems talk about reads as one person: one balloon over one head, the second line a
beat after the first, the same name tag and portrait whichever key the line came from. A host writes
one line per person (`speakers.alias(format!("society:{id}"), format!("person:{id}"))`) and nothing
per frame. A despawned villager's aliased barks stop silently instead of floating over nothing.

## Deviations

- **Issue #5's acceptance 4 is not main's behaviour.** It asks that two keys on one entity without an
  alias "still show the stacking". Main has one global `BarkQueue` (since `85b75ba`) and replaces a
  bark balloon over an entity that already has one, so two keys on one entity never stack. The pinned
  behaviour is the one main has (target 4). Assumed decision, recorded on #5.
- **Removing a target leaves the alias dormant rather than deleting it** (the issue: "drops the
  aliases that name it"). `Speakers.0` is public and removal does not pass through the crate; the
  observable result the issue wants (the dead villager's aliased barks stop) holds. Assumed decision.
- **Aliases apply to conversation lines too** (the issue: a conversation is "unaffected"). One resolver
  for every lookup is the general form; a script line by `society:7` floating over a different head
  than that person's barks would be the same two-people defect. Assumed decision.

## Pending index

- Brysbaert 2019, *How many words do we read per minute?* (10.1016/j.jml.2019.104047): no open-access
  PDF reachable; `dwell_secs` cites it from the abstract.
- Cohn 2013, *Navigating comics* (`10.3389_fpsyg.2013.00186`): downloaded 2026-10-30, waiting for
  scribe (big's VLM backend unavailable, bmb converting a 894-document queue).
