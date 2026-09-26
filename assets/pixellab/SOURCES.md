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
