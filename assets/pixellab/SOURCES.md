# PixelLab-generated assets

Generated through the PixelLab MCP server. Kept with their object IDs so any
one of them can be regenerated, restyled or animated later without guessing
what prompt produced it.

`create_object_state` needs the source object id, and `style_images` needs the
actual PNG, so losing either breaks the ability to make a matching sibling.

| file | object id | prompt summary | size |
|---|---|---|---|
| conductor_core_a.png | c1e88bd6-d379-4488-9052-c7c351009702 | dark charcoal sphere, brass ribs, cyan tuning-fork glyph, coral lights | 128 |
| conductor_core_b.png | 27bfa1ae-0ec3-490f-ad45-30131772db46 | same prompt, candidate 3 — cyan slit "closed eye", coral bolts | 128 |

Both are candidates from one `create_1_direction_object` call (20 generations,
4 candidates at size 128, `view="sidescroller"`), tagged `ball_swing_conductor`.

## Sun Devourer shield generators (2026-09-25)

Both styled from `devourer_body.png` + `devourer_bolt.png` (indexed-PNG
re-encodes via `tools/png.py`, so the base64 fits an MCP call), 128px,
`view="sidescroller"`, tag `ball_swing_devourer`. Selected in code by
`DEVOURER_GENERATOR_DESIGN` (constants.rs). The original
`devourer_generator.png` is kept, unused, as the backup.

| file | object id | notes |
|---|---|---|
| devourer_gen_pylon.png | 959ee80f-966a-4802-aa2e-a13bcd07c74e | grounded tesla obelisk, caged cyan crystal (candidate 1 of f2279906) |
| devourer_gen_pylon_damaged.png | 46d4d457-d8a6-4900-8d64-a86cdd04b69c | state "damaged": cracked crystal, bent prong, sparks |
| devourer_gen_pylon_idle/0-8.png | anim 5e260e87 on 959ee80f | v3, 8 frames (+ref = 9): crystal pulse, arcs; body still |
| devourer_gen_float.png | e2f4fa7e-cc74-4400-a7c2-743551d7be26 | floating drone, orbiting cyan rings, thrusters (candidate 3 of 93937842) |
| devourer_gen_float_damaged.png | 8729da1b-d006-4744-a446-90d1cd4d7b31 | state "damaged": cracked orb, broken ring. Thrusters came out as long beams to the frame edge — re-roll if they read as legs |
| devourer_gen_float_idle/0-8.png | anim 5e29dd7b on e2f4fa7e | v3: rings orbit, core pulses, bob |

Alternates kept on PixelLab, not downloaded as assets: pylon candidate 0
(f0c41893-06fb-4f85-b0fd-4b39efb32476), floating candidates 2 and 0
(fffa6f4c-e41c-40e7-a262-ded733d130cc, 691a3c47-13f1-40fd-ae71-f69f4619e0bd).

## Conventions that matter (learned from `agent_help`, 2026-09-24)

* `create_1_direction_object` with `view="sidescroller"` — NOT
  `create_8_direction_object` (8 rotations are wasted on a side-on game where
  the boss always faces the camera) and NOT `create_map_object` (1 generation,
  but it AUTO-DELETES after 8 hours and cannot be animated).
* Generate the FIRST asset at the size everything should share: passing
  `style_images` forbids setting `size`, and the largest style image then
  determines the output size.
* Keep siblings on-model by passing finished PNGs as `style_images`; four or
  more references match palette and outline better than one.
* `animate_object` with `mode="v3"` for frames — better and cheaper than the
  default.
* Frame URLs are public but behind Cloudflare: fetching them with Python's
  stdlib `urllib` returns error 1010. Use curl, requests or httpx.
* Never feed an upscaled render back as a style reference — `unzoom_image`
  first, or the style drifts toward the upscaling artefacts.
* If frames drift in palette across an animation, run `reduce_colors` over all
  of them in ONE call so they share a palette.
* `select_object_frames` returns the new ids in the order of `indices` — but
  confirm with `get_object` before building on one; it costs nothing.
* Style references in their ORIGINAL palette pull new objects toward it. That
  is the point for a boss's parts, and it still let an off-palette accent
  through (cyan cores on a bronze/black boss) when the prompt named it.

## Cost

20 generations per 128px object call, against a 2000/month Tier 1 allowance.
A three-part boss is roughly 60-120. `get_balance` reports what is left.

## The Gravity Weaver (2026-09-26)

Both styled from `colossus_head.png` (indexed via `tools/png.py`), 128px,
`view="sidescroller"`, tag `ball_swing_weaver`. Violet named in the prompt
survived the bronze/cyan style pull, as the conventions below predicted.

| file | object id | notes |
|---|---|---|
| weaver_loom.png | 0d78462c-c5f7-45d7-9f65-d6cddad2ac7e | iron ring loom, black lens, violet rim, brass weights (candidate 0 of c3045710) |
| weaver_loom_damaged.png | 475d8281-4a78-4667-86c7-997466de09b1 | state "damaged": cracked frame, snapped threads, lens flickering |
| weaver_loom_idle/0-8.png | anim d5530e84 on 0d78462c | v3, 8 frames (+ref): lens swirls, threads shimmer, weights sway |
| weaver_spindle.png | 7355c447-02c3-4387-a2ca-ee48546b6dfa | iron bobbin, brass tips, violet thread, loose end (candidate 0 of 1f05ead1) |
| weaver_spindle_damaged.png | f9675f9f-b1a0-42b4-8a5e-4683ca09a2b6 | state "damaged": cracked, thread unravelling, chipped tip |
| weaver_spindle_idle/0-8.png | anim f594db6a on 7355c447 | v3: spins on its axis, core pulses, thread sways |

Alternates kept on PixelLab, not downloaded: loom candidate 2 (eye-like
lens, 2db3ddc0-89ef-4cd2-aece-fdf4c33c501d) and spindle candidate 2 (curled
hook of thread, 0717d934-12c2-482c-96e1-a42ee945d697).

## Colossus hand cuff (2026-09-26)

`colossus_hand.png` is now the ORIGINAL hand (kept as `colossus_hand_v1.png`)
padded 40 px on the left and inpainted (`inpaint_image`, job
441ca11f-cb09-4716-8c37-5e62b1324ecf, mask 62x86 at (0,24)) with a rounded
iron cuff and an orange thruster opening, then padded to 168 square. The old
sprite was cut off at the forearm by the canvas edge; the cuff makes the cut
deliberate and gives the thrust plume a nozzle. Two full regenerations styled
on the old hand reproduced the crop (dismissed); a prompt-only pack
(eed4fb0f) produced a good alternate, candidate 0 kept on PixelLab.

## Re-rolls (2026-09-26): Serpent segment, Devourer bolt

| file | object id | notes |
|---|---|---|
| serpent_segment.png | 5f2d93d9-4146-40c9-932f-3a93a706e1af | silver plated barrel, violet vein; styled on `serpent_head.png` (candidate 0 of b2b20850). Old art kept as `serpent_segment_v1.png`; alternate (armoured, violet spine) 2e8af546 on PixelLab |
| devourer_bolt.png | dde093cb-b215-487a-81a6-98087e6adf4f | fireball with trailing corona, flying right; styled on `devourer_body.png` (candidate 0 of ebfcb148). Old art kept as `devourer_bolt_v1.png`; alternate 5559ebb4 on PixelLab. Bolt object made square (92) to fit |

## The Flare Titan (2026-09-27)

The core is styled from `colossus_torso.png` (indexed via `tools/png.py`);
the vent is styled from the chosen core, so the two share its dark iron and
orange seams. 128px, `view="sidescroller"`. A first vent pack styled on the
torso came back as four chest plates (dismissed, a0f4e32b); a front-facing
"porthole" pack mostly reproduced the core (dismissed, 255e0a31).

| file | object id | notes |
|---|---|---|
| titan_core.png | b8651517-2642-4aee-900f-e4e336941dd5 | caged star: dark iron sphere, four curved plates, flame jets at the diagonals (candidate 0 of f44f4e07) |
| titan_core_damaged.png | 3df15b9f-3dba-45f2-8797-a3dd31d2dc2f | state "damaged": plates cracked, star dimmed deep red, jets ragged |
| titan_core_idle/0-8.png | anim 92184728 on b8651517 | v3: plasma churns, jets flicker, seams shimmer |
| titan_vent.png | 3d9eff3b-ed55-4fed-8a77-c8c6745b039b | round iron furnace pot, orange seams, fire in its open mouth (candidate 2 of 517b250c); the MOUTH is its aim |
| titan_vent_damaged.png | 3013973f-9979-4c88-8192-c5d0afd7ac56 | state "damaged": cracked, seams dull red, fire low and smoking |
| titan_vent_idle/0-8.png | anim 8ec10627 on 3d9eff3b | v3: fire licks up, core pulses |

## The Magnetar (2026-09-27)

Styled from `conductor_core_a.png`, which pulled a tuning-fork emblem into
three of the four core candidates (not used). The pole's art is DIAGONAL
(tip at the bottom-left, aperture top-right): rotate by its axis angle, not
by zero.

| file | object id | notes |
|---|---|---|
| magnetar_core.png | 69cbc5a5-22fb-4d16-b37b-fe61e643e89f | cracked dark crust, cyan-white light through the cracks, field lines, brass clamps (candidate 0 of 5663f63e) |
| magnetar_core_damaged.png | 21810760-4084-40a1-89fb-278123788f6f | state "damaged": crust shattered, clamps broken, light gone violet |
| magnetar_core_idle/0-8.png | anim 4fc3565e on 69cbc5a5 | v3: cracks throb, crackles run along them, field lines ripple |
| magnetar_pole.png | 5ded5029-9081-4557-ae66-4ac974e51aa3 | crystal capsule in a dark cage, energy arcs (candidate 2 of 046f9634) |
| magnetar_pole_damaged.png | a4ee4e1d-22d8-4973-8249-70834fe0921d | state "damaged": crystal chipped, cage broken, glow dim |
| magnetar_pole_idle/0-8.png | anim 24ec12c3 on 5ded5029 | v3: light throbs, arcs crackle round the cage |
