# Engine programme — 2026-09-25

One tracker for the requests of 2026-09-25: dark-mode lag, a render-path
audit, shadows from terrain_collision outlines, a competitive lighting/effects
feature set, quartz_forge authoring parity, forcefield generator art, and
research against industry 2D engines. Earlier docs it supersedes in part:
`rendering-optimization.md`, `optimization-plan.md`.

## 1. Dark-mode lag — ROOT CAUSE MEASURED

Per-darkness light-cap A/B on the Motorola (device11.log, 8 darknesses):

| cap | lights | present | fps |
|---|---|---|---|
| all | up to 43 | 14-19 ms | 36-42 |
| 0 | 0 | 0.4 ms | 62 |
| 8 | 8 | 0.4 ms | 62 |

The cost was the lit shader looping every light for every lit fragment.

CORRECTION (found later the same day): post-processing was hard-disabled on
Android (`has_post = if cfg!(target_os = "android") { false }`, commit
3a8132a, 2026-09-07, no note of why). So on the phone the night-mode bloom,
vignette and CA NEVER RAN — the bloom downsample could not move a number
because its code never executed, and "the post pass is effectively free" was
never measured. The lights conclusion stands: the A/B varied only the light
count. Post is now a runtime switch, on by default, and each frame line
reports `post N` (frames in the window that ran it).

Fix (DONE, tests green, awaiting device measurement):
- `LightTileUniform` (wgpu_canvas/src/gpu_types.rs): 16x8 screen tiles, a
  256-bit light mask per tile, built on the CPU each frame. The lit shader
  loops only its tile's lights. Exact — attenuation is zero past `radius`.
- Separate uniform binding (group 1, binding 1): LightUniform is 12 KB and
  the device uses WebGL2-level limits, 16 KB per uniform binding.
- tests/light_tiles.rs — builder exactness + WGSL/Rust parity (verified to
  fail against a shrunken radius test and a wrong mask-word constant).
- tests/tiled_lighting_readback.rs — renders 70 lights on a real GPU and
  compares EVERY pixel to a CPU sum over all lights (verified to fail against
  a dropped mask word and a transposed tile index: ~29k bad pixels each).
- Frame log now carries `lights N occl M` (peak per window).
- `device.on_uncaptured_error` now logs and panics: a rejected pipeline is no
  longer silent on device.

Still to measure: install the tiled build, fight Sun Devourer, read
`present` under `night: ON ... tiled_lights`.

Also seen in device11.log, not yet explained:
- Outside darkness, 17-22 lights stay on in the boss arena (5-12 ms before
  tiling). Tiling should cover it; re-check.
- 22:50:19 onward: game tick lines STOP (pause/death screen?) while frames
  keep being DRAWN (not skipped) at 39 fps with full lighting. An idle
  screen should hit the unchanged-list skip. Something is animating or
  invalidating the list every frame. Phone was at thermal status 1.

### Device builds

- A `scratchpad/apk_tiled/main_tiled_lighting.apk` — tiled lighting only
  (post still off on Android). Not yet played.
- B `scratchpad/apk_b/main_b_post_shadow_ab.apk` — superseded by C, never
  installed.
- C, D, E — superseded, never installed.
- **F `scratchpad/apk_f/main_f.apk` — INSTALL THIS ONE.** Everything below
  (incl. node lights -> unlit nodes, floating generators + tethers, depth
  store Discard) + the DIAGNOSTIC described for C. Only on the Motorola
  (ZA223HJX4F), never the Quest (2G0YC5ZG7706YV).
- C `scratchpad/apk_c/main_c.apk` — everything below + DIAGNOSTIC
  per-darkness cycle (post, lamp shadows) = (on,off) (off,off) (on,on)
  (off,on), post toggled through the engine switch. Remove
  `night_diag_variant` from eclipse.rs after the capture. Read `post N` and
  `lights N` on each frame line and the `surface:` line at startup.

### RESULT — build F on the Motorola, 2026-09-26 (device12.log)

Mali-G615 MC2, Vulkan, surface `Rgba8UnormSrgb` (sRGB — the forge preview's
sRGB output matches). 8 darknesses, no panics or GPU errors:

| darkness setting | fps avg / min | present avg | lights max |
|---|---|---|---|
| post on, shadows off | 62.3 / 61 | 0.56 ms | 11 |
| post off, shadows off | 62.4 / 62 | 0.60 ms | 11 |
| post on, shadows on | 62.3 / 62 | 0.48 ms | 11 |
| post off, shadows on | 62.0 / 62 | 0.53 ms | 10 |
| daytime (whole run) | 61.8 / 48 | 0.50 ms | 11 |

Before: 36-42 fps, present 14-19 ms, 43 lights. Post and lamp shadows are
both free here — but the arena had only 0-1 occluders, so lamp shadows were
measured with almost nothing to shadow; re-measure in a scene with many
`outline_shadow` casters. The daytime 48 is the one second of menu -> game
texture creation. Diagnostic removed; post and lamp shadows now on
everywhere. Build G (`scratchpad/apk_g`) installed 00:28:59.

### Found in play, fixed 2026-09-26 (build H, `scratchpad/apk_h`)

- **Generators rendered dark** (27% of their art's brightness with lighting
  on). Images draw before lit sprites and transparent pixels wrote DEPTH, so
  the outline glow — a quad bigger than the sprite, transparent in the
  middle — hid the lit sprite it surrounded. Fix: alpha test (discard at
  a < 0.004) in the image and lit shaders. Also the lit sampler was nearest
  (the unlit one linear). Tests: wgpu_canvas tests/lit_sampling.rs, game
  src/render_tests.rs (renders a real generator through GameObject draw ->
  GPU and compares with its PNG). Generators are also `unlit` now.
- **Tethers drew as horizontal bands.** Attached mega effects never took
  their object's rotation (always 0), and the vertex shader rotated in UV
  space (skewed on a wide screen). Fix: rotation copied every frame in
  `resolve_mega_fx`; shaders rotate in pixel space using a new
  `Globals.aspect`. Also fixes the Colossus attack lanes, which were drawn
  unrotated. Tests: tests/mega_rotation.rs (2:1 frame), quartz
  hud_effect_placement.

### Found in play, fixed 2026-09-26 (build I, `scratchpad/apk_i`)

- **Tethers offset from generator and dome.** An attached effect was centred
  on `layout offset + size/2`, but a rotated object's layout offset is the
  top-left of its ROTATED AABB — a 1000x70 strip at 20 degrees drew ~170px
  off-centre. Fix: `physics::rotated_extent`, shared by
  `rotation_adjusted_offset` and `resolve_mega_fx`. Test through the real
  camera stage: quartz hud_effect_placement
  `a_rotated_objects_effect_is_centred_on_the_object` (old code: 85px off).
- **Colossus propulsion plumes** (new): energy jetting from the hands'
  forearm cuffs, the torso's waist and the head's neck, rotating with the
  hands, flaring with speed. New mega bit 31 `BIT_THRUST_PLUME` (the LAST
  flag bit). Effect tints are LINEAR: `COLOSSUS_PLUME_RGB` is the circuit
  orange decoded (1.0, 0.27, 0.03).

## 2. Shadows from terrain_collision outlines

Pipeline exists (quartz core.rs -> `dynamic_outline_world_hulls` -> polygon
occluder shape 3 -> lit shader), but:

STATUS: 1-4 DONE with tests (see list below); 5 open; 6 under device A/B.

1. **Concave sprites cast one convex chunk.** The outline is convex-decomposed
   for SAT collision; the shadow path takes `max_by_key(len)` of the hulls,
   i.e. the largest convex piece. Fix: keep the simplified concave silhouette
   in `CollisionOutlineData` and hand THAT to shadows.
2. **16 points per occluder, index-uniform sampling.** `sample_polygon_for_shadow`
   takes every k-th vertex, which clips spikes and features. Fix: shape-aware
   decimation (RDP to budget), and pack 2 points per vec4 so the same 8 KB
   holds 1024 points; raise the per-occluder budget.
3. **No culling, arbitrary drop past 32.** Every visible `shadow_caster` in the
   world is uploaded in store order; beyond 32 they are dropped arbitrarily.
   Fix: keep only occluders overlapping a shadow-casting light's circle
   (exact), nearest first.
4. **Per-fragment cost has no early-out.** Every shadowing light tests every
   occluder's every edge. Fix: bounding circle per occluder; ray-vs-circle
   reject before the polygon.
5. Shadows are binary (hard). Competitive engines offer soft penumbrae
   (Godot 2D: PCF filter + smoothing). See section 4.
6. Lamp shadows were turned OFF on Android during the bloom hunt, on a
   hypothesis the A/B later disproved. Re-enable after 1-4 and measure.

Done for items 1-4:
- `CollisionOutlineData::silhouette` (concave, decimated once to 64 pts);
  `dynamic_outline_world_silhouette`. tests/shadow_silhouette.rs.
- Packed polygon points (2 per vec4, 1024 total, 64 per occluder);
  Visvalingam decimation; `bound_radius` + `segment_near_circle` early-out.
  tests/polygon_shadow_readback.rs (GPU), renderer::shadow_polygon_tests.
- Occluders culled to shadow-casting lights' reach, nearest first; none at
  all when no light casts shadows.
- NEW API `GameObjectBuilder::outline_shadow()` / `obj.shadow_outline`:
  engine traces the sprite's outline for shadows automatically, keyed on the
  image Arc (Weak — never pins atlas textures), SEPARATE from collision
  outlines (a collision outline switches terrain collision to SAT).
  tests/outline_shadow.rs.
- `Canvas::frame_items()` split out of draw_pre; test hooks
  `frame_items_for_test`, `tick_lighting_for_test`.

## 2b. Glow and tint follow visible pixels — DONE

`set_glow` on an image sprite now builds a halo from the sprite's alpha
(chamfer distance field: crisp rim + soft falloff, zero inside the art) and
`set_tint` masks to the visible pixels. Cached per source image by Weak
identity; animation frames refresh via `update_animation`. Boxed glow, text,
shapes and stretched tiny fills keep the stroked shape. New builder
`.glow_boxed()`. tests/outline_highlight.rs, tests/glow_shape.rs.

## 2c. Emissive glow layer — DONE

`Item::EmissiveImage`: drawn UNLIT in the scene and again into the
quarter-size bloom layer; composited by the night shader, or by the new
built-in `BLOOM_COMPOSITE` when nothing else runs. Object glows emit it by
default (`set_glow_bloom(false)` opts out). `Canvas::set_glow_bloom_strength`
(wgpu) sets its strength. tests/frame_passes.rs, tests/outline_highlight.rs.

## 2d. Frame pass sequence — REBUILT (wgpu_canvas/src/frame.rs)

The window's pass sequence moved into `FrameTargets::encode`, shared with
`HeadlessRenderer` (which gained `new_with_samples`), so tests run the SAME
passes a player's frame does. Faults found and fixed on the way, each with a
test verified to fail against the original fault:
- Bloom downsample pipeline built for 4x MSAA, drawn into a 1x texture —
  a validation error (crash) on the first darkness on DESKTOP.
- ONE params buffer shared by every post pass: in a frame with two passes,
  both ran with the second one's parameters (night + air barrier).
- Post sampler was NEAREST: the "bilinear" bloom taps were 4-texel blocks.
- Post passes now 1x (a full-screen triangle has no edges to antialias).
- Unused full-res targets no longer allocated: bloom_msaa at 1x, post
  intermediates until a frame chains passes (~40 MB on the phone).
- Targets rebuild lazily when the surface size OR FORMAT changes (Android
  resume can return a different format).
- `EnvelopePayload::SetPostProcessing` + quartz
  `Canvas::set_post_processing_enabled` — a real quality switch.

ramp (app shell): item scaling extracted to `__private::to_physical` and
tested. It left occluder `polygon_points` and `corner_radius` in LOGICAL
pixels, so outline and rounded shadows were misplaced by the display scale on
2x desktops and most phones. ramp/tests/to_physical.rs.

DONE: node lights replaced — see §2e.

## 2e. Darkness without 40 lights — DONE

Two node-light systems stacked: eclipse `drive_node_lights` (one per hook
slot) AND arena `tick_boss_lights` (one per live hook, always on, never
disabled). Now: `show_nodes_in_dark` draws visible hooks UNLIT in the dark;
buff/shield hook glows bloom. Arena per-hook lights removed (they added
nothing visible at ambient 1.0 and cost 5-12 ms/frame).

## 2f. Typed Effect API — DONE (2026-09-26)

`wgpu_canvas::Effect` (re-exported by quartz): one enum variant per effect
with named, typed parameters; `Effect::pack` is the only place words are
written. Quartz: `obj.set_effect(effect, colour, size)`,
`c.attach_effect(id, ..)`, `c.clear_effect(id)`, `c.push_effect(..)`.
- Exclusive effects (dome, shield, rings, markers, lane, impact, tether,
  plume, ...) moved from flag bits 20..31 to an effect ID in `bitmask.w`.
  Bits 20..31 are free again for combinable looks; ids are unlimited.
- `EffectColor::srgb/srgb8/From<Color>` decode, `linear` passes through; no
  `From<(f32,f32,f32)>` on purpose (the peach-orange bug).
- Proof: all 20 effect encodings rendered byte-identical before/after the
  move (headless GPU); `effect::tests` pin every id and flag against the WGSL
  text (checked by breaking one); `tests/effect_ids.rs` proves dispatch.
- Game: every `MEGA_BIT_*`, `STATE_MARKER_*`, `IMPACT_*`, `TETHER_*`
  constant and every hand-packed wrapper deleted; fx.rs keeps only game
  policy (`attach_state_marker` scale, `lin()`).
- `fullscreen_effect_cost` was hand-packing the old beat-field bit and would
  have silently measured nothing; now uses the typed API.

## 6b. Sun Devourer generators — DONE (art + code)

Two designs, `DEVOURER_GENERATOR_DESIGN` = Floating (Pylon available):
idle loop, damaged state after the first hit, cyan outline glow that
blooms, energy TETHER (new mega bit 30) from each live generator into the
dome with packets flowing toward the dome, hit flash, tether SNAP on death,
dome dims per generator lost and flickers on a snap. Verified in a real
fight loop: `DEVOURER_GEN_CHECK=1 headless --boss-weakpoint-check` walks
hit -> damaged art -> snap -> barrier drop, 0 panics. Provenance in
assets/pixellab/SOURCES.md.

## 3b. Audit fixes landed

- Depth and resolved-MSAA attachments StoreOp::Discard (tile GPUs).
- `tick_lighting` no longer builds a map of every object per tick.
- `set_image` / `set_drawable` / `set_animation` refresh an active glow or
  tint (a swapped sprite kept its OLD outline).
Open: idle redraw (frames drawn, not skipped, while ticks stopped);
per-pass bind group creation in dynamic_post; `discard`-based clipping.

## Research

`docs/engine-research-2026-09-26.md` — Godot 4 / Unity URP 2D comparison,
tile-GPU rules, API proposals, prioritised roadmap.

NEXT (old note): replace the ~40 tether-node LIGHTS in
darkness with emissive glows (they exist only to make nodes visible) —
that removes most of the lights from the lit shader.

## 3. Render-path audit — TODO

Found so far:
- `tick_lighting` builds a HashMap<String,(f32,f32)> of EVERY object every
  tick (~700 String clones) to move a handful of attached lights.
- Post-processing disabled on Android (fixed, see §1).
- Surface format is `formats[0]`, whatever the platform lists first: sRGB on
  some devices, linear on others. Now logged at startup (`surface:` line).
  Decide after a device capture whether to prefer sRGB explicitly.

Walk the frame end to end: tick -> item build -> prepare -> passes -> present.
Candidates already known: idle redraw (above), per-frame occluder upload,
ShadowUniform 10 KB rewritten every frame, lit path taking every Image while
lighting is on (HUD/text included), pooled off-screen objects.

## 4. Lighting & effects feature set — TODO (research first)

Compare against Godot 2D (Light2D, LightOccluder2D, CanvasModulate, normal
+ specular maps, shadow filter/smoothing, light masks/cull layers, blend
modes), Unity URP 2D (Light2D types incl. freeform/sprite/global, shadow
caster 2D, light blend styles, normal maps, light render texture scale),
GameMaker, Defold. Then list gaps and the API each needs.

## 5. quartz_forge authoring parity — IN PROGRESS

quartz_forge does NOT link the engine; parity = its domain model, UI,
codegen, import and CPU previews knowing each feature.
- DONE: lighting preview matched to the shader (ndl 0.447 was missing —
  editor showed point lights 2.2x brighter than the game; directional used a
  fake 0.3; spots had no cone; output was linear not sRGB). Pinned to the
  WGSL text by `the_shader_still_computes_what_this_preview_assumes`.
- DONE: shadow shape (box / circle / pixel outline), `glow_boxed`,
  `glow_bloom` — domain, UI, codegen, import, round-trip test.
- TODO: outline glow + bloom in the forge's CPU preview; post toggle and
  glow bloom strength authoring; object effects as authorable (UNBLOCKED:
  the typed Effect enum is the domain model to mirror).

Every engine change this cycle needs an authoring surface or an explicit
"not authorable" note: tiled lighting (transparent), light count/occluder
stats, shadow silhouettes from outlines, `set_glow_boxed`, night-mode post
(downsampled bloom), GPU error reporting, `actual_size_virtual` fix.

## 6. Sun Devourer forcefield generators — TODO

Current `devourer_generator.png` reads as a rocket pod. Replace with a pylon
emitter (cyan crystal core matching the dome, 0.55/0.85/1.0), animated core,
damaged state (HP 2). Add an energy tether from each live generator into the
dome, pulses flowing generator -> boss; on death the tether snaps and the
dome flickers.

## 7. API quality — TODO

- quartz doctests fail (plugin/mod.rs x5, background, terrain_collision,
  scroll.rs): example code in docs does not compile.
