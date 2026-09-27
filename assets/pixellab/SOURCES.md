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

## Background decor (2026-09-27)

Parallax scenery for long Normal/Casual stretches (`src/scenes/game/decor.rs`),
tag `ball_swing_bg`, `view="sidescroller"`. Planets, the moon and the galaxy are
128px (one `item_descriptions` call of four per group); the objects are 64px
(one call of sixteen — the star cluster was not kept). The game dims them with a
per-depth tint, so the art itself stays full-colour.

| file | object id | notes |
|---|---|---|
| planet_ringed.png | 67bff101-5bd4-414b-9e5f-f5471261a273 | ringed gas giant |
| planet_ice.png | 99225243-b2ec-4f1a-afd4-01938f3baea2 | icy blue planet |
| planet_desert.png | 412f742f-36fb-4a51-8079-ebf32d976ce7 | rusty red desert planet |
| moon_cratered.png | 5ed74bb0-bf5b-4e58-8b4d-9def891a4a21 | small grey cratered moon |
| planet_ocean.png | 465ea03a-2784-4ad4-82cc-bffcb0eb2193 | blue ocean planet |
| planet_violet.png | b482d12f-f0c6-4952-a59b-a19d241dca0b | purple gas giant |
| planet_lava.png | 239ed34b-4cf0-42c1-969f-ea0c33f1f0b9 | lava planet |
| galaxy_spiral.png | d852f52e-d8a7-4c40-a6df-fc28e1c34409 | distant spiral galaxy |
| satellite.png | 779aa8a8-8ff6-498d-8297-7c6fe22efacf | small satellite |
| comet.png | 8b619c72-c8cb-46ff-83de-c8d866afb9d7 | comet, icy head |
| asteroids.png | 1265af19-0c12-4759-b728-7e42cf0fa956 | cluster of three asteroids |
| station.png | b12489fa-5ccc-4b5c-bb98-d2b1d6582ef8 | distant ring space station |
| derelict.png | 9737024a-f788-4117-97ca-563af43b1dac | derelict cargo ship |
| tiny_moon.png | 10614bf4-bbef-4a60-b2fc-d988a671fc40 | tiny cratered moon |
| ringed_small.png | 1f125695-8bd6-4266-8c14-72a7007f99d4 | tiny ringed planet |
| buoy.png | b70e83fc-2347-448c-bf69-2ab5166763e5 | space buoy with a beacon |
| crystals.png | 30df0154-b36d-4aba-9a15-9fef88c8f9e1 | floating glowing crystals |
| rocket.png | 366ec31a-23d3-4a0d-b72e-6bdc13abdec4 | small rocket ship |
| ufo.png | 3ff09cbd-bd11-429a-9d67-ed4381a0aee8 | RARE: silver flying saucer |
| space_whale.png | 77f7f6ba-c239-4291-bfa8-5f2dc44e4865 | RARE: huge gentle space whale |
| astronaut.png | 6e0f07a1-6aaf-4852-95f9-d3854914c423 | RARE: lone drifting astronaut |
| jellyfish.png | 58bb2019-e91f-4755-bc20-c73b23319315 | RARE: translucent space jellyfish |
| rubber_duck.png | c5b647a1-5e51-4b00-b8d5-e218198f2891 | RARE: yellow rubber duck |

## Rope styles (2026-09-27)

Tiles for `Effect::TiledStrip` (`src/cosmetics.rs`), 64px, tag
`ball_swing_ropes`. Each is drawn as NEUTRAL WHITE light on purpose: the game
gradient-maps it to the chosen rope colour (or the style's signature colour),
so the art must not carry a hue of its own. A tile is a vertical strip that
spans the full height; the game mirrors every other tile, so it need not tile
seamlessly. `<name>_flow/0-8.png` is a 9-frame `animate_object` loop ("the
column stays in place spanning the full height" keeps it tileable).

| file | object id | animation | notes |
|---|---|---|---|
| helix.png, helix_flow/ | 5c9109e1-7b8f-412a-a38d-498ddb6eb707 | 6d667615 | twin strands twisting |
| chain.png, chain_flow/ | 564ca30e-5b71-4f86-af4a-78f4cfc2f27d | b3f6d1bc | chain of glowing links |
| lightning.png, lightning_flow/ | a82c6af6-83cb-41b4-86cc-0a7ee6421edc | df062192 | jagged crackling bolt |
| void.png, void_flow/ | 18fdea06-3bc2-40aa-848f-87ac4b240a38 | 103e7dcc | black thread; frames 6-8 go near-black, only 0-5 are used |
| flame.png, flame_flow/ | 83eeb4fa-9024-4b0b-9935-fad563a14364 | d8c66ee6 | flickering flame column |
| stardust.png, stardust_flow/ | 0d0436a3-abc1-4819-8e39-517e1433cde8 | d1919565 | twinkling star column |
| spectral.png, spectral_flow/ | 3930387f-1419-439e-96bf-19fdd4ffce56 | ad5672e4 | wisps thickening to a solid chain; played back and forth |
| braid.png | 8d1e7136-9afb-447f-a94c-8aa4180a8a24 | — | braided light rope |
| circuit.png | e64d2ed0-5110-4153-ab44-4990396b320d | — | circuit trace |
| vine.png | 33760eb4-bdc4-4927-9312-ade7f8996a1b | — | vine of light with leaves |
| runic.png | 699c6352-8bd6-4894-a71e-0108ded1c8c2 | — | column of runes |
| ribbon.png | 9c3d1302-956e-46f1-80d1-7e8b54716bc1 | — | wave of energy |

## Cat breeds (2026-09-27)

Styled from the calico (`calico_ball_idx.png`, the indexed re-encode), 128px,
tag `ball_swing_cats` on the base. Each cat is one object group of three
states: `base` (curled ball), `open` (spread-eagle) and `half` ("half uncurled
from the ball: its startled face and both front paws popping out, back legs
and tail still tucked in"). The game plays ball → half → open as the ball rises
and back when it hooks.

| cat | ball (base) | open | half |
|---|---|---|---|
| ginger | 1a24d7d0-868c-4728-ae78-df2737b577c1 | 6f8bd81e-1454-4314-8b69-6218164f4442 | 1347b046-2ea5-45a4-b1c0-4b274db3d87d |
| black (MIDNIGHT) | 4c565ce6-717f-4f0d-9201-dbeffce88fd1 | 4e9ebbec-1779-4377-b7d0-e72480067055 | 091979b9-9828-4564-898b-9e231d00a8c4 |
| siamese | 1538bc63-66c2-4ef2-bdd7-fb83338ce1ba | e891b42d-b5f1-4ae2-b7c0-ab3669361ccb | 0f4bb865-8a63-4817-b6cf-9905fdfb7a42 |
| tuxedo | 1306dba3-de26-4fe1-9755-9f56d7617d15 | 9c1e6787-61d4-4a9c-b7a9-f289eeda4080 | 87b5e51d-d6fc-4b03-8d6a-ac2ea03ae9f0 |
| silver (SILVER TABBY) | e59ed501-1fe2-4729-8322-2810b77d22fa | b2d400db-bbd8-4cef-91eb-e2de122fe9e5 | 0902154d-0a3e-4ab5-8f00-d407ecb42f22 |
| mainecoon | 52cfa403-d54c-414e-a30b-bea67637018a | c08554ff-66eb-4f59-a036-b6846a055e17 | f056604f-4b5f-4bcb-b895-d84614c6b559 |
| persian | 6a80fcf8-32cb-48b0-84fe-94ba5217a4e4 | b7280fce-1b0b-4725-aee9-523f4bd47a40 | dfec43f4-7439-462a-b390-4e75e4ff9f4f |
| sphynx | 83e4a168-9405-4c74-b875-a1debbc31d58 | fd93f4ae-a2c0-4cc0-9b89-b0a11c11f9da | f8ca3989-2d9d-4dbe-a557-80d22d8d6574 |
| galaxy | 1030027b-2878-451d-9b5e-766c713448cc | 35fa1131-3ebc-4216-a2c6-eb2652ccb34e | c495af92-59bf-4b75-a1a2-8225d610a906 |
| robot (ROBO-CAT) | 94e2fdec-d07f-4228-8992-150b74ca9674 | 9ee03f4e-08cb-4c16-9e57-03faf30b76b3 | 3fd6ebc2-349f-4f9a-8c5c-41585b358a24 |
| astronaut (ASTRO-CAT) | 0fa420d1-176d-413d-b86d-1d1f9402a1b7 | 84bb9ebd-352e-45f0-906b-fa1c7db2537f | f9233381-7f14-4172-a917-38cea6d25600 |
| magma | 7b92331a-9292-4e7e-b4d6-d6f02de0701a | 4980095c-2a7d-4df4-8efc-f4ab6947acc9 | 9717650c-73ba-4ed1-a894-63c224272c3c |
