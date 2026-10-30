# Balloon anchoring: where a world balloon floats, and when it shows

The system: `track_bubbles::<C>` (`src/balloon.rs`), which every frame puts each balloon (a root quad
with a `Bubble { owner, .. }`) above its owner, turns it to the camera and mirrors the owner's
visibility. Issue #10: it read the owner's **local** `Transform` and its **own** `Visibility`, so a
speaker that is a child entity (a rider on a cart, a person placed under a building, a bone-attached
head) got its balloon at its parent-relative offset from the world origin, and a speaker hidden through
its parent kept a visible balloon.

## Sources

| Kind | Stem / reference | Used for |
| --- | --- | --- |
| Model (comics, reading) | `10.48550_arxiv.2306.17469` — Li et al., *Manga109Dialog: A Large-scale Dialogue Dataset for Comics Speaker Detection* (arXiv 2306.17469, Table 2) | readers attribute a balloon to the character nearest it |
| Model (reading attached text) | `10.1145_3361218` — Sidenmark & Gellersen, *Eye, Head and Torso Coordination During Gaze Shifts in Virtual Reality*, TOCHI 2019, §6 | text attached to a speaker is read by shifting gaze between the two |
| Shipping game | `GameAIPro2_Chapter02_Combat_Dialogue_in_FEAR_The_Illusion_of_Communication` — Orkin, *Combat Dialogue in FEAR*, Game AI Pro 2 ch. 2 | a line the player cannot attribute did not happen |
| Shipping game | `gameaipro3-ch37-simulating-character-knowledge-phenomena-talk` — Ryan et al., *Simulating Character Knowledge Phenomena in Talk of the Town*, Game AI Pro 3 ch. 37 | background conversation as speech bubbles emitted from the speakers |
| Technical | `Game Engine Architecture, Third Edition` — Gregory, §5.3.10 (coordinate-space hierarchy) and §12.3.2 (joint local poses) | a child's transform is relative to its parent; world space is the root |
| Technical | `book_real_time_rendering_4th_akenine` — Akenine-Möller et al., *Real-Time Rendering* 4th ed., ch. 19, "Scene Graphs" (p. 850 of the converted copy) | the world transform is the product of the transforms from the root down |
| Technical | Bevy 0.20 source: `bevy_transform-0.20.1/src/helper.rs` (`TransformHelper`), `bevy_transform-0.20.1/src/systems.rs` (propagation), `bevy_camera-0.20.1/src/visibility/mod.rs` (`Visibility`, `visibility_propagate_system`) | the exact composition and inheritance rules the balloon must agree with |

## Craft principle

A balloon is read as spoken by the character it sits by. In the Manga109Dialog benchmark the simplest
baseline attributes text purely by proximity, and it is right most of the time:

> "The simplest rule-based method is 'shortest distance', where we assume the text is spoken by the
> closest character." (Manga109Dialog, §5.1)

Table 2 scores that rule at 71.43 % on the "Easy" pairs and 63.41 % overall (81.55 % / 71.53 % when the
character in the same frame is preferred). Proximity is the reader's first cue, so a balloon that floats
at the wrong place is not merely mis-placed: it is attributed to whoever happens to stand there.

Text that belongs to a speaker is read together with the speaker. Sidenmark & Gellersen, on placing text
in a 3D view:

> "For example, when placing subtitles that are attached to the speaker, it may be more suitable to place
> them below the speaker to decrease the likelihood of unnecessary head movement when users are shifting
> between the subtitles and speaker." (§6)

The reader's eye moves between the speaker and the text, so the text must stay at a fixed relation to
the speaker as the speaker moves, whatever moves it (its own legs, a cart, a parent transform).

## How shipping games do it

Barks exist to be attributed. Orkin on FEAR's squad dialogue:

> "If the AI didn't say it, it didn't happen. This was the AI-design philosophy behind the squad
> behaviors in FEAR. There is no point in expending significant effort implementing complex AI if the
> player doesn't notice it." (Game AI Pro 2, ch. 2.1)

> "The guiding principle behind combat banter in FEAR is that whenever possible, AI characters should
> talk to each other about what they are doing. Rather than having an individual character react with a
> bark, multiple characters carry on a short dialogue that broadcasts mental state and intentions to the
> player." (ch. 2.2)

Talk of the Town shows its NPC dialogue the way this crate does, as world balloons on the speakers:

> "[…] displayed as speech bubbles emitting from the characters who speak them. By virtue of our dialog
> manager and NLG module, NPCs may engage in background conversation with one another. If the speech
> bubbles they emit are in view, the player can eavesdrop on the conversation" (Game AI Pro 3, ch. 37)

## Model adopted

Engines keep an entity's transform relative to its parent and compose to the root. Gregory:

> "coordinate spaces form a hierarchy—every coordinate space is a child of some other coordinate space,
> and the other space acts as its parent. World space has no parent; it is at the root of the
> coordinate-space tree, and all other coordinate systems are ultimately specified relative to it,
> either as direct children or more-distant relatives." (GEA 3e, §5.3.10)

> "For example, if we rotate the shoulder joint, but leave the parent-relative poses of the elbow, wrist
> and fingers unchanged, the entire arm will rotate about the shoulder in a rigid manner, as we'd
> expect. We sometimes use the term local pose to describe a parent-relative pose." (§12.3.2)

and *Real-Time Rendering*:

> "Transforms are updated on the way from the root toward the leaves. The matrices are multiplied in
> this traversal and stored in relevant nodes." (*Real-Time Rendering* 4th ed., ch. 19, "Scene Graphs")

Bevy stores the product in `GlobalTransform`, but only in `PostUpdate` (`TransformSystems::Propagate`),
while `track_bubbles` runs in `Update`. Its own `TransformHelper` says when to compose by hand:

> "Computing an entity's `GlobalTransform` can be expensive so it is recommended you use the
> `GlobalTransform` component stored on the entity, unless you need a `GlobalTransform` that reflects
> the changes made to any `Transform`s since the last time the transform propagation systems ran."
> (`bevy_transform-0.20.1/src/helper.rs`)

A balloon must reflect this frame's movement (a root owner's balloon already does, reading `Transform`),
so the model is:

1. **Position.** The anchor is the owner's world translation, composed on demand: start from the
   owner's `Transform::translation` and apply each ancestor's `Transform` (`transform_point`: scale,
   rotate, translate) walking `ChildOf` to the root. That is `TransformHelper::compute_global_transform`'s
   translation, without building the full affine and without `TransformHelper`'s `Query<&Transform>`,
   which would conflict with the system's `Query<&mut Transform>` on the balloons. A root owner costs one
   absent-`ChildOf` check more than before. `anchor_y` and the camera-plane offsets stay in world units
   along world +Y and the camera axes: the balloon floats up, not along a tilted parent's up, and its size
   is `BalloonScale`'s, never the parent's scale.
2. **Visibility.** Bevy's rule (`visibility/mod.rs`): `Inherited` takes the parent's, "A root-level
   entity that is set to `Inherited` will be visible", `Hidden` is "unconditionally hidden" and
   `Visible` "will be visible regardless of whether the `ChildOf` target entity is hidden"; a parent
   without the components counts as visible (`visibility_propagate_system`: "fall back to true if no
   parent is found or parent lacks components"). Walking the same chain, the first non-`Inherited`
   value decides; reaching the root decides visible. The balloon (a root) gets `Visibility::Inherited`
   when visible and `Visibility::Hidden` when not, written only when it changes (`set_if_neq`), so a
   still balloon no longer re-triggers visibility propagation every frame.
3. **Schedule.** Unchanged: `Update`, after the host's movement like any billboard. Reading the owner's
   `GlobalTransform` instead would be one frame late for every owner (propagation is in `PostUpdate`),
   regressing today's root owners; moving tracking to `PostUpdate` would change the crate's documented
   schedule and force it to write `GlobalTransform` itself.

## Parameters

None new. `BalloonScale { world_per_px, anchor_y }` keeps its meaning; `anchor_y` is now documented as
above the owner's **world** position.

## Validation targets

1. Issue #10's input: owner `Transform::default()` under a parent at `(10, 0, 0)` → balloon `x` within
   0.05 of 10 (today ≈ 0.004).
2. A grandchild under a rotated, scaled, translated chain: the balloon's anchor equals Bevy's propagated
   `GlobalTransform::translation()` of the owner plus `anchor_y`, to 1e-4.
3. Same frame: a parent moved before `track_bubbles` runs (no propagation in between) moves the balloon
   on that frame.
4. Visibility: parent `Hidden` → the balloon's `InheritedVisibility` is false after propagation; parent
   back to `Inherited` → true; an owner set `Visible` under a hidden parent → true (Bevy's rule).
5. Root owners unchanged: `track_bubbles_follows_a_change_of_balloon_scale` passes as written.
6. Cost: the bench's `frame-live` at 200 balloons within 5 % of before (medians of three runs).
7. `two_balloons` (the thinker is a child of a sliding cart): its thought balloon over the thinker's cube.

## What great looks like

Every balloon sits over its speaker's head in the frame the speaker is drawn, wherever the speaker sits
in the host's hierarchy: a rider on a cart, a villager parented to a building piece in an editor, a head
bone. A player never sees a balloon at the world origin, never sees one over an empty spot after its
speaker's parent was hidden, and never sees one trail its speaker by a frame. The tail points down at the
speaker's head, upright in world space, at the size the host chose, whatever the parent's rotation or
scale.

## Deviations

- **A gap in the hierarchy ends the walk.** An ancestor without a `Transform` is treated as the root:
  the anchor is the owner's position relative to it. Bevy's propagation never reaches such a subtree
  (`propagate_parent_transforms` starts from `Without<ChildOf>` roots), so there is no world position to
  agree with, and `TransformHelper` returns `MissingTransform`. Design default: keep the balloon rather
  than drop a line because of a malformed hierarchy.
- **No cycle guard.** A `ChildOf` cycle (Bevy rejects only self-reference) would make the walk loop, as
  it makes `TransformHelper::compute_global_transform` loop. Same contract as Bevy.
- **The owner still needs `Transform` and `Visibility`.** Unchanged from before; ancestors need neither.

## Pending index

- `10.48550_arxiv.2306.17469` (Manga109Dialog) and `10.3390_jimaging4070087` (Augereau et al., *A Survey
  of Comics Research in Computer Science*) were downloaded this pass and await conversion and indexing
  (the scribe queue held 878). The Manga109Dialog quotes above are from the arXiv HTML (v2).
- Not obtainable: Rigaud et al., *Speech balloon and speaker association for comics and manga
  understanding* (ICDAR 2015, 10.1109/ICDAR.2015.7333782; HAL serves a landing page), Kurlander et al.,
  *Comic Chat* (SIGGRAPH 1996, 10.1145/237170.237260; ACM 403), Chu & Yu, *Optimized speech balloon
  placement for automatic comics generation* (10.1145/2505483.2505486). The corpus has no comics
  lettering text (Eisner, McCloud, Cohn) on balloon tails and placement.
