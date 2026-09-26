# Replacing the placeholder art

Every visual in this game is currently drawn by code in `src/images.rs` — 55
functions producing circles, rounded rectangles and solid fills. That was the
right call to ship a playable build with no art pipeline, and it is why a state
marker is a yellow square. It is not what the game should look like.

The Conductor's core and spars are now generated pixel art, which proves the
pipeline end to end. This is the list of everything else.

## Budget

1916 generations remain this cycle, refilling 7 Oct 2026. An object costs 20,
so the allowance is roughly **95 objects a month**. The list below is about 40
objects — comfortably one cycle, with room for the rejects that are part of
the process.

## Done

| asset | what | notes |
|---|---|---|
| Conductor core | 128px, idle loop (9f) | + `vulnerable` state |
| Conductor spar | 128px, ring loop (9f) | 6 spars, phase-offset |

## Bosses

Ordered by how much the fight depends on reading the art.

| boss | placeholder | needs |
|---|---|---|
| **Colossus** | `colossus_hand`, `colossus_torso`, `colossus_head` | 3 objects + a telegraph/wind-up state for the hand. It is the FIRST boss most players fight, so it sets the impression. |
| **Serpent** | `serpent_head`, `serpent_segment`, `serpent_tail`, `serpent_spine` | 4 objects. The spine is a solid orange rectangle and reads as a missing texture; the segments repeat many times so they must tile cleanly. |
| **Flare Titan** | `flare_front_img`, `flare_overlay_img`, `shield_dome_img` | 3, plus the flare itself is a full-screen effect and may be better as a shader than a sprite. |
| **Sun Devourer** | eclipse sequence | the approach is the game's best set piece and is currently geometric. |
| Gravity Weaver | not built | design the art WITH the fight, not after. |
| Magnetar | not built | as above. |

## State markers — the "yellow square" class

These are the ones that read as unfinished because they are literally a
coloured rectangle. Highest value per generation of anything on this list,
because they are on screen constantly and in every fight.

* `danger_zone` — a flat red block.
* `conductor_weakpoint`, `conductor_pip` — the vulnerability marker.
* `gate_img`, `pad_img`, `spinner_img` — arena furniture.
* `arrow_img` — the direction hint.
* `gravity_well_img`, `black_hole_img` — both currently rings.
* Buff / upgrade / fast-travel node markers.

## HUD

`hearts_img`, `oxygen_bar_img`, `oxygen_canister_img`, and the pause UI
(`pause_overlay_img`, `pause_title_img`, `pause_btn_img`). `create_ui_asset`
exists for exactly this and is not the same tool as the object one.

## What the pipeline is, now that it works

1. `create_1_direction_object`, `view="sidescroller"`, `size=128`.
   128 yields 4 candidates to choose between; it is also the size everything
   else must then match, because a style image's size DETERMINES the output
   size and so the first asset fixes the whole set.
2. Pick candidates with `select_object_frames`. Rejects cost nothing extra —
   the 20 generations buy the batch, not the keeper.
3. `animate_object`, `mode="v3"`, 8 frames → 9 stored (frame 0 is the source).
4. Download the numbered PNGs with curl, drop them in `assets/pixellab/<name>/`.
5. `include_bytes!` the frames and build with `pl_sprite`, which falls back to
   the procedural version if anything fails to decode.

### The traps, all hit for real

* **Base64 gets truncated in transit.** A 6967-byte RGBA PNG (9292 base64
  chars) was silently cut off and rejected. Re-encoding the SAME pixels as an
  indexed PNG gave 2859 bytes / 3812 chars and went through. Pixel art has few
  colours — this one had 46 — so indexing is lossless and roughly a third the
  size. `tools/png.py` does it and verifies the round trip is pixel-identical.
* **A style image constrains SILHOUETTE, not just palette.** Feeding the round
  core as a style reference for an elongated spar produced four more round
  plates. For a differently-shaped part, describe the shape emphatically and
  drop the style image, then unify the palette afterwards.
* **Animation overwrites a swapped image.** `update_animation` rebuilds
  `drawable` from the sprite every frame, so setting an image on an animated
  object does nothing visible. Clear `animated_sprite` first — that is why the
  vulnerable core drops its idle loop.
* **Scale with NEAREST.** Anything else turns 128px pixel art into mush at the
  300px it is drawn at.

## Order of work

1. **State markers.** Cheapest, most visible, on screen in every fight.
2. **Colossus.** First boss, worst current art.
3. **Serpent.** The spine especially.
4. **HUD**, via `create_ui_asset`.
5. **Flare Titan.**
6. Weaver and Magnetar — art designed alongside the fights.
