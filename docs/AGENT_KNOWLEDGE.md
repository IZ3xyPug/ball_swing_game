# ball_swing_game — knowledge base for coding agents

A handoff of what months of Claude Code sessions on this game (2026-05 to
2026-09-28) learned, so another agent does not have to rediscover it. Almost
every line here was measured, reproduced or fixed, not guessed. The code is the
final authority: verify a name before relying on it, and if something here is
wrong, fix this file.

Start with `CLAUDE.md` (repo root) for the rules; use this file as the manual.

1. Repos and layout
2. Build, run, test, ship
3. Game architecture
4. Engine and rendering
5. Traps, by area (symptom, cause, fix)
6. Proven patterns
7. Art pipeline (PixelLab)
8. How to verify a change
9. Open decisions and next steps
10. Other docs in this repo
11. Glossary

---

## 1. Repos and layout

Three git repos, nested on disk:

```
synful_quartz/          github.com/Artistsyn/synful_quartz   branch merge_test  (the engine)
  quartz/               engine: Canvas, GameObject, scenes, actions, camera, plugins, physics bridge
  wgpu_canvas/          renderer: mega shader, typed Effect enum, FrameTargets, HeadlessRenderer
  prism/ ramp/ crystalline/ maverick_os/   UI/runtime/physics/platform layers
  ball_swing_game/      github.com/IZ3xyPug/ball_swing_game  branch revive     (this game, its own repo)
  quartz_forge/         github.com/Artistsyn/quartz_forge    branch synful     (editor + codegen, its own repo)
```

- The game builds against the engine BY PATH (`quartz = { path = "../quartz" }`,
  `ramp = { package = "ramp2", path = "../ramp" }`), so this repo must sit inside
  a synful_quartz checkout. An engine change the game needs (a new effect, an API)
  is a commit in synful_quartz; push both, or the other side will not build.
- ball_swing_game and quartz_forge are separate repos inside the engine's folder.
  Never `git add` them from the parent.
- quartz_forge has a parity test over the engine's `Effect` enum: add a new effect
  there too (author it, or list it in `not_authorable` in `src/core/object_effect.rs`).
- Never commit: `debug.keystore`, `.DS_Store`, `target/`.

## 2. Build, run, test, ship

### Desktop
- `cargo run --release` from this folder. Boots to the profile screen (mouse
  only). `BSG_PROFILE_SLOT=0 cargo run --release` goes straight into slot 0;
  add `QUARTZ_TICK_PROFILE=1` for per-stage tick timings (`tick:` lines).
- Controls: hold Space (or press/touch) to grab the nearest node, release to let
  go. Dev keys in `scenes/game/build_scene.rs` `on_key_press`: `0` god mode,
  `1` zero-g, `2` flip gravity, `5` spawn a rocket pad, `g` spawn a gravity
  cannon, `f` accept cannon hyper-transit, `3`/`4` death-sound mode, `7`/`8`/`9`
  music tracks, `p` pause.
- Main menu, BOSS ORDER: a testing override of the boss order (in memory only).

### Tests
- `cargo test --release` (about 175 in the game; `sim_tests.rs` holds scenario
  tests, including a buff-deadlock guard that takes ~30 s). The engine crates
  (`wgpu_canvas`, `quartz`) have their own suites, including GPU readback tests.
  Readback tests SKIP without an adapter and still report ok, so confirm they ran.
- quartz doc examples are real doctests now (`cargo test --release --doc`).

### The headless driver (the most useful tool here)
`cargo build --release --bin headless`, then `./target/release/headless ...`.
It runs the real game with an autopilot bot and no window. It is deterministic:
the same inputs hash identically, which makes A/B tests possible.

- `--episodes N --frames N`: length of the run.
- `--at-minute M`: seed the run M minutes into the difficulty curve.
- `--boss-kind <colossus|conductor|weaver|titan|magnetar|serpent|devourer>`:
  warp straight into one fight. Always add `--boss-stasis-down`, or the fight
  sits in its entry stasis and never reaches its clock.
- `--boss`, `--boss-warp`, `--boss-weakpoint-check` (forces the Sun Devourer with
  a buff), `--fall-test`, `--flare-test`, `--flare-shelter-check`.
- `HEADLESS_SHOT_DIR=<dir>` with `HEADLESS_SHOT_EVERY=N`: save rendered frames
  (PNG, half size) every N frames.
- `HEADLESS_IMMORTAL=1`: hearts never run out, so a capture reaches late content.
- `HEADLESS_CHAR` / `HEADLESS_ROPE_STYLE` / `HEADLESS_ROPE_COLOR`: cosmetic indices.
- `HEADLESS_SHOP=<dir>`: screenshots of the shop screens;
  with `HEADLESS_SHOP_SEQ=<style>` it films that rope style's preview instead.
- `QUARTZ_COUNT_ITEMS=1`: draw-item counts every 300 frames.
- Boss check modes print `*-check:` lines and step the fight: `MAGNETAR_CHECK`,
  `TITAN_CHECK`, `WEAVER_CHECK`; `*_REACH_CHECK` logs whether each damage
  window is reachable in time; `DEVOURER_GEN_CHECK`, `DEVOURER_DEBUG`.
- The summary prints a hazard census (PEAK live hooks, pads, spinners, wells,
  turrets, asteroids), starved frames (no reachable node), the flare audit
  (`nearest_shelter_at_flare`), boss kills, hearts lost.
- The autopilot is a weak player. Its deaths measure its own swinging, not
  difficulty; use the census for difficulty.

### Android
- `./build_android.sh --no-install` produces `target/release/apk/main.apk`
  (cargo-apk; package `com.orange.pkg`; aarch64 and armv7; landscape). It needs
  cargo-apk and the Android SDK/NDK variables EXPORTED in the shell that runs it.
  A non-interactive shell does not read `~/.zshrc`, so a build that works in a
  terminal can fail from a script.
- Signing: `Cargo.toml` points at `debug.keystore` in this folder. That is the
  standard Android debug keystore (copy `~/.android/debug.keystore`; password
  `android`). It is not a secret, but never commit it, and Play rejects it.
  Without it cargo-apk compiles everything and then silently produces no APK.
- Install with the device serial every time: `adb -s <serial> install -r target/release/apk/main.apk`.
  A VR headset on the same USB bus once received the game because `-s` was left off.
  Check with `adb -s <serial> shell dumpsys package com.orange.pkg | grep lastUpdateTime`.
- A failed build leaves the OLD apk in place. Before saying a build shipped,
  check that `target/aarch64-linux-android/release/libmain.so` is newer than
  your last source edit. In zsh use `$pipestatus[1]`; `PIPESTATUS` is empty
  there, so an exit-code check silently reads nothing.
- Logs: `adb -s <serial> logcat | grep -E "frame:|tick:|PANIC"`. A panic hook
  logs `PANIC:`. An ANR (not responding) is a DEADLOCK, not a panic: start at locks.

## 3. Game architecture

### Module map (`src/`)
| File | What it owns |
|---|---|
| `lib.rs` | app start: canvas, scenes, input handlers that must survive scene changes |
| `constants.rs` | every tuning number, boss roster, `deal_roster`, `FINAL_BOSS` |
| `state.rs` | `State`, the whole run's state, shared as `Arc<Mutex<State>>` |
| `difficulty.rs` / `hazards.rs` / `level_gen.rs` / `mode.rs` | the curve, hazard stages, node generation, game modes |
| `scenes/game/build_scene.rs` | scene wiring, `State` init, THE per-frame tick order |
| `scenes/game/bootstrap.rs` | all pooled objects and HUD objects, with their layers |
| `scenes/game/physics.rs` | rope constraint, rope drawing, momentum cap and write-back |
| `scenes/game/spawning.rs` / `culling.rs` | per-frame spawn and cull of every pooled type |
| `scenes/game/solar.rs` / `eclipse.rs` / `space_zone.rs` / `gravity_cannon.rs` | flares and shelters, pre-boss eclipse, space zone, cannons |
| `scenes/game/hearts.rs` / `pickups.rs` / `upgrades.rs` | hearts and buff, coins and power-ups, in-run upgrade nodes |
| `scenes/game/boss/*.rs` | `arena.rs` (entry, nodes, walls, finish), `common.rs` (hits, hit-stop), one file per boss |
| `scenes/game/decor.rs` | parallax background planets and oddities |
| `shop.rs` / `cosmetics.rs` / `profile.rs` / `menu.rs` | shop UI, rope styles and cat breeds, save files, menus |
| `achievements.rs` | the current (small) achievement set |
| `headless.rs` / `bin/headless.rs` | the headless driver |
| `sim_tests.rs` | scenario tests over the real tick |

### The frame (order matters)
`build_scene.rs` runs, each tick:
1. `read_player_from_engine` (`s.px`/`py`/`vx`/`vy` from the engine object);
2. `tick_rope_constraint`;
3. spawning;
4. culling;
5. the boss tick;
6. upgrades;
7. the eclipse;
8. ...;
9. `cap_momentum_and_write_back` (copies `s.vx`/`s.vy` onto the player object,
   capped at `MOMENTUM_CAP` unless `boss_knockback_ticks` is set);
10. decor.

So a sustained force goes on `s.vx`/`s.vy`. Writing `obj.momentum` directly is
overwritten, except for one-shot throws that also set `boss_knockback_ticks`.

### State and canvas vars
- `State` (src/state.rs) holds:
  - player physics (`px`, `py`, `vx`, `vy`, `hooked`, hook position, `rope_len`);
  - pools (`*_live`, `*_free`, `*_rightmost`) and the hook generator (`gen_head_*`,
    `live_hooks`, `pool_free`);
  - boss state (`boss_kind`, `boss_parts`, `boss_roster`, per-boss clocks);
  - flare fields;
  - the buff (read it ONLY through `buff_active()` / `retire_buff()`);
  - `grab_lockout_ticks`;
  - decor.
- Mutex rules: never lock while already holding the guard (a deadlock, and an ANR
  on Android); snapshot reads in one lock; drop the guard before canvas calls
  that can re-enter.
- Canvas vars (`c.set_var` / `get_var` / `get_i32`...) carry UI and cross-system
  flags. Typed getters return a default rather than panic. Anything a scene's
  `on_enter` reads must be set BEFORE `load_scene`.

### World, units, layers
- The virtual screen is `VW x VH` = 3840 x 2160 world px at zoom 1; boss arenas
  zoom out. `ignore_zoom` objects are screen space (HUD, backdrop, decor, the
  flare wash).
- 60 ticks per second. `GRAVITY` 0.82 px/tick^2. In arenas gravity is x0.05
  (= 0.041): size any boss force against THAT. `MOMENTUM_CAP` 50. Rope 200-720 px.
- Draw order is layer, then insertion order:

  | Layer | What |
  |---|---|
  | 0 | backdrop and decor |
  | 1 | flare wash (deliberately under gameplay) |
  | ~2-12 | pads, supports, props |
  | 19-32 | arena dressing and boss parts |
  | 20 | rope |
  | 21 | nodes (`LAYER_SPACE_HOOK`) |
  | 42 | player |
  | 43 | air shield |
  | 50 | zero-g overlay |
  | 61-62 | banners |
  | 100-102 | HUD |
  | 150 | toasts |
  | 9500+ | pause UI |
- Nodes are pooled (`hook_0` .. `hook_67`). A new node spawns HIDDEN and
  off-screen, then animates in; its x is already final while hidden.

### Run structure
- **Modes** (`mode.rs`):
  - Casual: endless, no bosses, 0.35x META.
  - Normal: the 80-minute curve, 7 bosses, the last at the top of the curve.
  - Boss Rush: 6,400 px links, no space zone, 0.85x META.
- **Difficulty** (`difficulty.rs`): `difficulty_t(distance)` is a smoothstep from
  minute 2 to 80 at `ASSUMED_PLAYER_PX_PER_SEC` = 300. That speed was measured
  from real play; it is the one number to retune. Everything scaled goes through
  `ramp()` and `hazard_gap()`.
- **Hazards** (`hazards.rs` STAGES): each hazard is introduced at its rarest and
  matures over ~25 minutes:

  | Hazard | Introduced at |
  |---|---|
  | spinners | 6.6 min |
  | gravity wells | 16 |
  | turrets | 25.4 |
  | drifting asteroids | 37.4 |
  | comets | 48 |
  | solar flares | 58.6 |

  Supports (pads, high asteroids) thin toward a floor.
- **Nodes** (`level_gen.rs`): generated one hop at a time inside an asymmetric
  reach ellipse (`hop_dy_budget`: up 0.85x, down 1.15x of the max rope). Later
  placement passes may move a node, so the reach rule is re-checked at the LAST
  placement stage.
- **Space zone**: rocket pads launch the player there. It has oxygen, a lethal
  sun, and rifts that flip gravity. Distance and META count only the normal zone.
- **Solar flares** (`solar.rs`):
  - A 3 s warning (`FLARE_WARN_TICKS` 180), then 5 s active. Unsheltered, damage
    lands every 2 s after 1 s of grace.
  - Shelter means being TETHERED to a gold shield node. SUNPROOFING wards absorb ticks.
  - A flare only starts if a shield node lies between 2,200 px behind and
    6,000 px ahead. Shield nodes come at least every 4,800 px.
- **Eclipse** (`eclipse.rs`): the world darkens on the approach to each boss;
  it is the multi-beat warning before the warp.
- **Bosses**:
  - Arenas are carved far off the level (`BOSS_ARENA_ORIGIN_X` = 4x the curve
    distance) and the player warps in. Fights trigger at
    `boss_trigger_distance(mode, index)`.
  - Order: `deal_roster(seed)` shuffles six bosses and `FINAL_BOSS` (the Magnetar)
    goes last. The deal is kept in `State::boss_roster`; lookup goes testing
    override, then the dealt roster, then the shipped order (`boss_kind_for_run`).
  - Hits need the BUFF (grab a cyan buff node first). A landed hit gets a
    hit-stop and rebound. Contact damage is gated by `boss_contact_cooldown`
    (45 ticks).
  - Outline states (shielded, vulnerable, winding up) are drawn on each part's
    sprite silhouette.

  | Boss | Parts and hits | Signature |
  |---|---|---|
  | Sun Devourer | generators, then 20 buffed hits | a dome powered by generators; bait it into them |
  | Colossus | hands, torso, head | lunges, claps, meteor storms, vents, a gravity well |
  | Serpent | 8 segments x1, tail 4, head 5 | a chain that swims through rifts |
  | Conductor | 7 on-beat releases arm 5 s of damage | rhythm: let go ON the kick drums (`beatmap.rs`) |
  | Gravity Weaver | 4 spindles x2, loom 14 | shuttle threads; the world flips (`gravity_dir`) |
  | Flare Titan | 4 vents x2, core 12 | flares are its clock; shelter nodes; solar wind |
  | Magnetar | 2 poles x3, core 12 | sweeping beams; pull/push pulses; open only in starquakes |

- **Magnetar pulse**: the pulse tears the rope off and locks grabs for 40 ticks
  (`MAGNETAR_PULSE_LOCKOUT_TICKS`, via `State::grab_lockout_ticks`), like the
  Colossus well.
- **Hearts** (`hearts.rs`): `lose_heart` has no invulnerability window of its own
  and resets the buff, so every caller gates it.
- **Economy**: per run, META = 1 per 5,000 px (normal zone only), +20 for
  visiting space, +50 per boss, all times the mode's multiplier.
  - Permanent upgrades (`profile.rs`): VITALITY, LONG LINE, FLYWHEEL, MAGNETISM,
    SEED FUNDS, SUNPROOFING, SECOND WIND.
  - In-run purple upgrade nodes cost coins.
- **Profiles**: `saves/profile_<N>.txt`, plain text (no serde). It includes
  `owned_cosmetics` (keys like `rope:helix`, `cat:ginger`), `cosmetic_rope_style`
  and achievements.
- **Shop** (`shop.rs`, `cosmetics.rs`):
  - Characters: the calico, 6 colours, and 12 PixelLab breeds (frames ball, half, open).
  - 13 rope STYLES drawn with `Effect::TiledStrip`, and 10 rope COLOURS in their
    own row. ORIGINAL means the style's signature colour.
  - Backgrounds, trails, upgrades.
  - `Price { Free, Meta, Premium }`, where Premium is reserved for a future
    real-money currency.
- **Decor** (`decor.rs`): 23 PixelLab planets and objects, at most 3 on screen,
  one rare oddity at a time: UFO, space whale, astronaut, jellyfish, rubber duck.
  Sightings are recorded in `decor_rare_seen`.
- **Achievements**: the current set is small and mostly trivial. A redesign and
  three bugs are written up in `docs/achievements-brainstorm-2026-09-27.md`.
- **Audio**: one output stream for the whole process. Sounds load from bytes
  (`include_bytes!`); an asset path built from `CARGO_MANIFEST_DIR` names the
  BUILD machine and never plays on a device.

## 4. Engine and rendering

### Effects: the typed `wgpu_canvas::Effect` enum
There are three ways to show an effect:
- **Attach** (`c.attach_effect(id, effect, color, world_size)`): the effect
  follows its object and takes the object's depth. Each object has ONE such slot;
  `attach_overlay_effect` gives it a second.
- **Push** (`c.push_effect(effect, color, world_center, world_size)`): one frame
  only, drawn on top of EVERYTHING, and it trails a moving camera by a frame. Use
  it only for screen-level things such as the flare front.
- **Outline state** (`c.attach_outline_state(...)`): a shield, vulnerable or
  wind-up field that hugs the sprite's visible pixels.

Effect ids (`bitmask.w`):

| Id | Effect | Id | Effect |
|---|---|---|---|
| 1 | EnergyDome | 11 | EnergyTether |
| 2 | SegmentShield | 12 | ThrustPlume |
| 3 | SonicRing | 13 | GravityLens |
| 4 | ResonanceWave | 14 | EnergyWall |
| 5 | BeatField | 15 | DangerZone |
| 6 | EqArcs | 16 | OutlineState |
| 7 | StateMarker | 17 | ProminenceArc |
| 8 | StrikeLane | 18 | PulsarBeam |
| 9 | BeatNode | 19 | TiledStrip (an image repeated along the quad: the rope styles) |
| 10 | Impact | 20 | SolarFront (the flare's plasma column) |

`Image`, `AnimatedImage` and `Animated` are also variants.

Adding an effect takes edits in four files, and the tests fail until they agree:
1. `wgpu_canvas/src/effect.rs`: an `fx_id` constant, an enum variant and a pack arm;
2. `animated_vfx.wgsl`: a `const FX_*` and its branch;
3. `wgpu_canvas/tests/effect_ids.rs`: the ids test and the distinct-picture test;
4. forge parity, in `quartz_forge/src/core/object_effect.rs`.

To SEE an effect before it reaches a device:
`EFFECT_LOOK_DUMP=<dir> cargo test --release --test effect_look` in `wgpu_canvas`.

### Shader rules, each learned from a real bug
- Tints are LINEAR. A picker colour passed straight in renders pale (orange became
  peach); use `EffectColor::srgb`.
- Local uv (0,0) is the TOP-LEFT of the quad. Textures were upside down for months
  because every test sprite was symmetric; test with an asymmetric sprite.
- Fade alpha to 0 before the quad's edges. Blending is in linear light, so even a
  4% tail cut off at the edge shows as a box.
- Alpha-test (`discard` when a < 0.004), because transparent texels still write
  depth and hide the sprite behind them.
- Keep noise inputs small: pass the effect its OWN time (seconds since it
  appeared). `fbm` fed the global clock turns into a repeating grid after
  minutes of play. `milli()` clamps to 0..1, so never pack a time with it.
- A new Rust uniform field needs the matching WGSL field, or pipeline creation
  fails with "invalid field accessor". bytemuck checks sizes, not names.
- WGSL is validated when the PIPELINE is created, not at `cargo build`, and a
  rejected pipeline draws NOTHING and logs nothing (the frame even gets faster).
  Install `device.on_uncaptured_error` and keep a naga validation or readback
  test.
- `textureSampleCompare` needs uniform control flow; after an early return use
  `textureSampleCompareLevel`.
- Build test sprites through `Effect::sprite` / `pack`. Never hand-pack a
  bitmask in a test: when the encoding changes, it silently tests nothing.

### Frame pipeline and performance facts
- The pass sequence is `wgpu_canvas/src/frame.rs` (`FrameTargets::encode`). The
  window and `HeadlessRenderer` share it, so readback tests exercise the real
  chain. Use `new_with_samples(w,h,4)` for desktop MSAA.
- Post-processing:
  - Post passes are fullscreen triangles: build them at 1x, not with the scene's
    MSAA count.
  - Give each pass its own uniform slot: every pass sharing one buffer sees the
    LAST write.
  - `SamplerDescriptor::default()` filters NEAREST; set Linear and ClampToEdge.
- On the phone (Adreno), dark-mode lag was the lit shader looping every light
  per pixel: 43 lights ran at 36-42 fps, and tiled light culling gave 62 fps.
  Post-processing was hard-disabled on Android until 2026-09-25; check that a
  pass RUNS on the device before optimising it.
- The texture atlas is keyed by `Arc` pointer. A new `Arc<RgbaImage>` every frame
  is an upload every frame; this caused three separate stutters. Share images
  (`quartz::white_pixel()`, `OnceLock` caches) and watch `uploads N/s` in the
  `frame:` log, which must be 0 at steady state.
- `GameObject::size` is BOTH the drawn size and the collision box. Rasterise
  textures small and let `size` scale them.
- Large 2D effects are FILL-bound, not CPU-bound: count drawn area first. A
  straight polyline needs ONE quad.

### quartz API shape (things that compile but do the wrong thing, or do not compile)
- Import from `quartz::prelude::*`, never from `prism` or `crystalline` directly.
- `Color(r,g,b,a)` is a tuple struct (no `Color::rgb`); `Key::Character` takes a
  `String`.
- `GameObject::new_rect(ctx, id, img, size, pos, tags, momentum, scale, GRAVITY)`:
  the LAST argument is gravity, not rotation. Set `obj.rotation` separately.
- `Action::SetPosition` zeroes momentum; `Action::Teleport` keeps it.
- `SmoothZoom` animates the zoom; `SetZoom` is an instant cut.
- `pool_acquire` resets position and momentum ONLY. Rotation, colour, animation,
  scale, tags and flags carry over from the object's last use.
- A pooled object parked off-screen with non-zero gravity gains momentum while it
  waits and flies off when it spawns; give parked objects zero gravity.
- Camera flash is engine-managed: use `camera_mut().flash(...)`.
- Plugins:
  - Call them through `Action::PluginCall` / `RunPlugin`; the bool they return
    is ignored.
  - They must implement `as_any` and `as_any_mut`.
  - `on_post_solve` runs only on the Crystalline physics path.
- `GameObjectBuilder` quirks: `.pivot()`, not `.with_pivot()`. `finish()` resets
  `rotation_momentum`. `fill_screen` only pins the object; it does not resize it.
- `Canvas::input_to_unzoomed(pos)` before hit-testing an `ignore_zoom` object.
- `Shared<T>` is a `RefCell`: read into a local before calling `get_mut` on the
  same value, or it panics at runtime.
- GIF frames: decode them into full composited frames. A raw frame buffer is a
  palette delta that shows wrong colours. For frame-exact art, take each raw
  frame without layering it over the previous ones, or the picture drifts
  (the gravity cannon).
- Scaling in `ramp`: every coordinate field of a draw item must go through
  `to_physical`. One field was missed, and shadows sat in the wrong place on
  scaled displays.
- Shadows vs collision: an object gets its shadow outline from
  `ensure_outline_for_image`. Registering a dynamic outline instead also switches
  its COLLISION to the outline shape.

## 5. Traps, by area (symptom, cause, fix)

### UI, scenes, input
- **A HUD or pause button does nothing while zoomed.** `ignore_zoom` objects are
  laid out at the base scale, but input is divided by base x zoom. Fix: convert
  with `c.input_to_unzoomed(pos)`, which uses the LAYOUT zoom, not `camera.zoom`.
- **A debug or test canvas var has no effect at all.** The scene's `on_enter`
  runs inside `load_scene` and read the var before it was set. Fix: set it
  before `load_scene`, or pass it into the canvas factory.
- **Pause buttons fire during other soft-pauses.** Handlers were gated on
  `game_paused`, which is true in any stasis. Fix: gate them on a dedicated
  `pause_menu_open`.
- **UI hidden when the pause menu opens reappears.** Tick callbacks re-derived
  its visibility, then `pause()` froze whatever they last set. Fix: hide it in
  the function that opens the menu, restore it on resume, or gate on
  `pause_animating` too.
- **Menu clicks do nothing** although a handler was registered in a scene's
  `on_enter`. Fix: register handlers that must survive scene changes at app
  start, guarded by `is_scene("<name>")`.
- **Text drifts off its boxes after a resize.** `ui_text_spec` bakes
  `virtual_scale()` in when the text is set, and at boot that value is still 1.0.
  Fix: re-apply text in an `on_update` whenever the scale changes.

### Game logic, generation, pacing
- **An expired buff stays in force.** Consumers read `player_buff` while only the
  tick knew about `buff_timer`. Fix: the one reader and one writer,
  `buff_active()` and `retire_buff()`.
- **The game hangs (an ANR on Android) when a buff activates.** The code re-locked
  the run-state mutex while holding it. Fix: write through the guard you hold,
  or `drop` it first, unconditionally. Rare paths (buffs, pickups, boss phases)
  are where this survives testing.
- **World generation stops at a certain distance.** An auxiliary spawner advanced
  `rightmost_x`, the cursor that gates the chain generator. Fix: only the
  generator writes its own cursor. A self-healing invariant and a source-level
  test guard it now.
- **Hops can't be made even though the generator obeys the reach rule.** Later
  placement passes moved nodes. Fix: enforce the rule at the LAST stage, against
  the ACTUAL previous node.
- **The difficulty ramp finished in 38 seconds.** It was written in raw world
  units. Fix: author pacing in minutes and convert through one measured speed
  constant.
- **A hazard never appeared.** Two independent gates (a stale zone rule and the
  new introduction curve) multiplied to almost zero. Fix: when a new system owns
  a concern, delete the old gate for it, then count live objects.
- **Flares never fired early in a run.** A cadence counted in NODES could not
  meet a guarantee measured in PX. Fix: use the same unit as the rule that
  consumes it.
- **Buff and shield nodes never existed** in the built game. An if/else chain put
  the general case first. Fix: order from most specific to least.
- **A recycled node still acts as a shelter.** Tag cleanup listed only some of the
  kind tags. Fix: clear EVERY tag the spawner can assign, from one list.
- **Warping to an off-level arena broke distance and the curve.** Fix: metrics
  exclude teleported regions (`!in_space && !boss_active`). Derive off-level
  origins from the curve's length, not a literal.
- **Features silently missing after making a region relocatable.** Constants and
  one-time object positions still pointed at the old fixed spot. Fix: grep for
  every derived constant, and position markers when they are revealed.
- **A finisher that runs forever.** Deleting one of two completion paths left
  `finish_boss` with no callers, and the survivor was an incomplete copy (no
  reward, no advance). Fix: route everything through the canonical finisher.
- **A clamp that constrains nothing.** A bound like `sqrt(R*R)` is just R. Fix:
  evaluate every clamp bound; it must depend on the same variables as the value.

### Bosses
- **Contact drained a whole run in under a second.** `lose_heart` ran every frame
  of an overlap. Fix: gate on `boss_contact_cooldown` (45 ticks), give that gate
  ONE owner, and latch multi-frame attacks so they hit once.
- **An arena-wide attack never touched the player.** Its extent came from the
  BOSS body instead of the band the player can be in. Fix: use the node-band
  constants plus margin, and aim safe gaps AWAY from the player.
- **Strikes are skipped when the target is the launch point.** An arrival edge
  written as `now && !prev` never fires at distance 0. Fix:
  `now && (state_ticks == 1 || !prev)`.
- **A part is fully open the moment its shield drops.** The window was written
  as "idle and ticks < N". Fix: a per-part "has finished an attack" flag, cleared
  whenever the part is shielded.
- **Boss forces nobody noticed.** The Magnetar pull (0.12) and the Titan wind
  (0.03) were tuned in raw px/tick^2, near arena gravity (0.041). Fix: size them
  as multiples of arena gravity (the pull is now ~6x, the wind ~2.5x), and pin
  them with tests. If a rope must not blunt a force, tear the rope off and lock
  grabs briefly.
- **A diagnosis contradicted by an observation in the same report.** Trust the
  observation. Also ask whether the bot even REACHED the case: it died at
  6-10k px, and the bug started at 36k.

### Rendering, effects
- **A pooled object wears an old effect in its next life.** A system released
  its effect only on its normal path, not on early returns. Fix: release first,
  unconditionally. When two systems share a slot (the player's shelter dome and
  buff aura), track which one attached.
- **Glows vanish at the screen edge.** The cull ignored the glow stroke, which
  spills outside the object's box. Fix: expand the cull box by the glow width.
- **A stutter when an effect turns on.** A new 1x1 image per call means an atlas
  upload per call. Fix: `white_pixel()` and cached images.
- **An attached effect is off-centre on a rotated object.** Fix: centre it with
  the rotated extent and copy the rotation.
- **An effect lags its object by a frame.** It was converted to screen space
  during the tick. Fix: keep effects in world units; the engine resolves them.
- **The flare wash hid the shelter domes.** The wash was drawn above gameplay.
  Fix: a mood wash goes just above the backdrop (layer 1).

### Android
- **The whole game freezes on any sound.** Each sound opened an audio device and
  blocked the game thread. Fix: one output stream; do every wait on the sound's
  own thread.
- **No audio on a device, and the frame rate pinned.** Asset paths named the
  build machine, and a "play the next track when this ends" rule re-spawned a
  failed sound every frame. Fix: `include_bytes!`, and play only handles that are
  actually connected.
- **A black screen after resume.** The surface was rebuilt from the destroyed
  window. Fix: rebuild from the NEW window, re-querying its formats.
- **The app dies at launch.** The surface was rebuilt on every `Resumed`, but
  that event also fires at startup with the same window. Fix: rebuild only when
  the native window's identity changes.
- **An ordinary resize or rotation kills the app.** `get_current_texture().unwrap()`
  under `panic = "abort"`. Fix: on Outdated or Lost, reconfigure and retry once;
  otherwise skip the frame.
- **Panics are invisible on a device.** stderr goes nowhere there. Fix: a panic
  hook that logs `PANIC:` (it already exists; keep it).
- **An optimisation that changes nothing.** The code never ran on the device.
  Fix: log per frame whether the pass ran before measuring it.

### Tooling and verification
- **A stale APK or binary shipped.** A failed build leaves the previous artifact
  in place. Fix: ask the artifact (compare mtimes and versions); never trust an
  exit code.
- **A build "passes" but never ran.** The binary wasn't on PATH, and the
  pipeline's exit code came from `tail`. Fix: grep for `Finished` or `error`.
- **A restored file still fails its test.** `mv`/`cp` from a backup kept the older
  mtime, so cargo replayed the stale build. Fix: `touch` the file after a restore.
- **macOS BSD grep matches nothing when the pattern has `\b`.** Also quote
  `--include='*.rs'` in zsh.
- **The headless flare audit falsely reported missing shelters.** It used
  `objects_in_radius`, which skips hidden objects, and new nodes are hidden until
  they animate in. Fix: scan the pool by tag and compare x.

## 6. Proven patterns

### Designing a boss
- **Calibrate against the roster.** Match the house timing language rather than
  inventing numbers: telegraphs around 60 ticks, pattern cooldowns of 120-160
  ticks, and damage windows that open a beat AFTER an attack lands. An early fight
  TEACHES its mechanic (faster cycle, cheaper hits); a late one tests it.
- **Two beats per part.** Alternate two attacks with opposite roles, one to dodge
  and one to punish, each with its own wind-up colour. Alternate them; never
  randomise the order.
- **Arrive, hold, open.** Compute arrival (`ceil(dist / MOMENTUM_CAP)`), hold the
  part still, open the weakpoint ~0.5 s AFTER arrival, then recover slowly while
  vulnerable. Freeze the body while any part is mid-attack.
- **Telegraphs clear before the hit; the hit flashes.** Vulnerability is shown on
  the part's own silhouette (outline states), not as a ring floating nearby. A
  hit needs a hit-stop, a flash and a rebound so it FEELS like a hit
  (`docs/boss-vfx-research-2026-09-27.md`).
- **Drawn equals hurts.** The hit test uses the exact geometry the shader draws
  (`ArcPath` sits in wgpu_canvas beside the packing). A test pins any constant
  shared between Rust and WGSL.
- **A boss can drive a global system.** The Titan runs the solar-flare system
  from its own clock (`titan_owns_flares`). Match phase lengths to the system's,
  and clear the shared fields on every exit (spawn, fall, win, finish).
- **Hard throws**: set `s.vx`/`s.vy` AND `boss_knockback_ticks` to get past the
  momentum cap briefly. For a rope-breaking force, unhook and set
  `grab_lockout_ticks`.
- **Sustained forces**: size them against arena gravity (0.041) and pin them with
  a test.

### Levels, pacing, pools
- **Pacing lives in minutes.** Convert to px through one measured constant
  (`ASSUMED_PLAYER_PX_PER_SEC`). Test the curve's shape at named times.
- **Hazard stages.** Each hazard gets an introduce-at and a mature-at in one
  table; it arrives at its rarest.
- **Derive the boss schedule from the roster**, not from a separate interval
  constant (they drift apart).
- **Reach envelope.** An asymmetric ellipse, because climbing costs momentum and
  falling is nearly free; the same budget function is used at generation AND
  at final placement.
- **A failsafe hook must not advance the spawn frontier.**
- **Pooled obstacles**: `*_live`, `*_free` and `*_rightmost` in State. Spawn ahead
  of the camera, cull behind it, and reset EVERY field on acquire (gravity 0 for
  manually moved objects).
- **Capturing the player** (cannon, upgrade menu): a state flag, a physics guard,
  the position pinned in the tick, and the release sets velocity. Use soft-pause
  (`game_paused`) for choice menus and pin the player so hazards cannot kill.
- **Telegraph a teleport in beats**: a named warning far out, the world dims (the
  player's own light widens), then the destination shows, then the warp.
- **META comes from distance plus flat bonuses**, never from coins or score, so it
  cannot be farmed.

### Verification tools
- **Check modes** exercise what the weak bot never reaches: place the player
  deterministically, then let the real code run.
- **Census.** Count what is actually alive at a point on the curve (`--at-minute`),
  rather than reading the functions that should produce it.
- **Starved frames.** The fraction of airborne frames with nothing in reach
  measures the LEVEL, independent of how well the bot plays.
- **Same-seed A/B.** Scale a force with a temporary env var (0x, 1x, 40x), log
  distances, compare; delete the debug line after.
- **Deadlocks.** Reproduce an on-device hang headlessly, then keep it as a test on
  a thread with `recv_timeout`, so CI fails instead of hanging.
- **Stutters.** The `frame:` and `tick:` logs together attribute one;
  `uploads 0/s` rules out texture churn.
- **Golden render** before changing a shader's encoding: render every case old and
  new through `HeadlessRenderer` and compare bytes.

## 7. Art pipeline (PixelLab)

Most of the art (bosses, cats, decor, rope tiles) comes from PixelLab through its
MCP server. Every file's provenance (object id, prompt summary, notes) is in
`assets/pixellab/SOURCES.md`. The ids belong to the main developer's PixelLab
account, so another account cannot fetch them, but the prompts and notes let you
regenerate matching art.

- **Objects.** `create_1_direction_object` with `view="sidescroller"`:
  - A size of 170 or below returns 4, 16 or 64 CANDIDATES in "review" status;
    keep the ones you want with `select_object_frames`.
  - `item_descriptions` makes several different items in one call.
  - Cost is about 20-40 generations. At most 8 jobs run at once; the server
    sometimes errors, so retry later. `wait_for_jobs` is free.
- **Style matching.** `style_images` takes base64 of the REAL PNG. Re-encode it
  as an indexed PNG (`tools/png.py`) to fit the call. Never type out or
  approximate base64.
- **Variants.** `create_object_state` (for example "damaged", and the cats' "open"
  and "half") keeps the object's look.
- **Animation.** `animate_object` in mode v3 stores 9 frames (8 plus the
  reference) for about 17-20 generations. v3 frames often CHANGE SHAPE from frame
  to frame, which reads as flicker or breaks in game. Measure the changed pixels
  between frames and on the last-to-first wrap before shipping. Where frames
  morph, ship one still and move it by scrolling.
- **Recolourable art.** Generate it neutral WHITE. At load, pull a bright tile's
  mean luminance to ~0.72 (a gamma, so the brightest texels stay white), then
  gradient-map it to the colour. Measure exposure on frame 0 and apply it to
  every frame (`cosmetics.rs`).
- **Seamless strips.** Find the tile's natural period (rows y and y+P match) and
  crop it to whole periods to tile without mirroring. Irregular art is mirrored
  instead. Directional art (flames) stays unmirrored.
- **Loading.** Art comes in through `include_bytes!` and helpers in
  `scenes/game/helpers.rs` (`pl_image_cached`, `pl_sprite`). Sizes are quantised
  so the image caches hit.
- **Review.** Build contact sheets (`tools/png.py`, `sips` on macOS) and LOOK
  before wiring art in. Test anything that samples a texture with an asymmetric
  sprite.

## 8. How to verify a change

The main developer's standard: lead with the outcome, ground every claim in
evidence (test output, a capture, a measurement), and say plainly what was NOT
verified. A change is not "done" because it compiles.

1. **Logic**: a focused test in the crate (`sim_tests.rs` for scenarios), then
   the full `cargo test --release`.
2. **Anything visual**:
   - Take headless captures (`HEADLESS_SHOT_DIR`, `HEADLESS_SHOP`, the boss
     flags), crop to the area and look.
   - For a shader, use `EFFECT_LOOK_DUMP`.
   - Measure where eyes can fool you (brightness profiles, frame differences,
     censuses).
3. **Feel and timing**: check modes (`*_CHECK`) and same-seed A/B runs.
4. **Engine changes**: run the `wgpu_canvas` and `quartz` suites. If `Effect`
   changed, also run the forge tests and its codegen check:
   ```
   cargo run --release --manifest-path quartz_forge/Cargo.toml --bin qf_verify_generated
   cargo check --manifest-path forge_verify_game/Cargo.toml
   ```
   Then delete `forge_verify_game/`.
5. **Device**: build the APK, confirm it is newer than your edits, install with
   `-s <serial>`, and check `lastUpdateTime`. The human tests on the phone and
   reports back.
6. **Committing**: only when asked; only the files you changed; a message that
   says WHY. Push the engine and the game together when the game needs the
   engine change.

## 9. Open decisions and next steps (as of 2026-09-28)

- **Boss order.** It is fully shuffled except the Magnetar, which is last. The
  Conductor and the Weaver were tuned for fixed slots. Recommended: a TIERED deal
  (Devourer, Colossus, Serpent, then Conductor, Weaver, Titan). See
  `docs/overnight-review-2026-09-27.md`.
- **Run-flare fairness.** A flare can start while the nearest shelter is up to
  ~4,800 px ahead and not yet visible, which costs 1-2 hearts with perfect play.
  Recommended: shield nodes every ~3,000 px and a search-ahead of ~3,200. Not
  changed yet.
- **Difficulty.**
  - The first 15-20 minutes barely change, because the smoothstep starts flat.
  - Recommended:
    - an ease-out early slope;
    - a 40-minute curve for Casual;
    - `zone_lap()` mutators after the top of the curve (the function exists but is
      unused);
    - surge and breather waves;
    - announcing each hazard stage.
  - Unused levers: rope length, node drift, hazard pairings, crumbling nodes, wind
    corridors, heart scarcity.
- **Achievements.** Rebuild them as one data table driven by game events, and fix
  the three bugs in `docs/achievements-brainstorm-2026-09-27.md` (81 ideas there).
- **Flare wash.** It now tints only the sky, so the nodes and player stay
  readable. That is a look change; revisit it if disliked.
- **Solar front.** It is a procedural shader (`SolarFront`). PixelLab art could
  replace or layer onto it.
- **Magnetar and Titan force tuning** is by feel: `MAGNETAR_PULL_ACCEL`,
  `MAGNETAR_PUSH_ACCEL`, `MAGNETAR_PULSE_LOCKOUT_TICKS` and `TITAN_WIND` in
  `constants.rs`.
- **Rope overshoot.** The rope draws a little past the hook and the ball (the
  classic GIF was built for that). It is minor, but visible on the premium styles.

## 10. Other docs in this repo

| Doc | What it holds |
|---|---|
| `docs/engine-program-2026-09-25.md` | engine programme tracker: light culling, post on Android, FrameTargets, typed effects, outline shadows |
| `docs/engine-research-2026-09-26.md` | Quartz vs industry 2D engines; the roadmap |
| `docs/optimization-plan.md`, `docs/rendering-optimization.md` | measured performance and what to do about it |
| `docs/boss-vfx-research-2026-09-27.md` | telegraphs, vulnerability and hit feel: research and what changed |
| `docs/overnight-review-2026-09-27.md` | final-boss choice, difficulty review, recommendations |
| `docs/achievements-brainstorm-2026-09-27.md` | achievement ideas and how to build them |
| `docs/art-replacement-plan.md` | replacing placeholder art |
| `docs/HOOK_GENERATION_STOPS_ANALYSIS.md` | generation stopping (resolved: the frontier cursor) |
| `assets/pixellab/SOURCES.md` | art provenance |

## 11. Glossary

| Term | Meaning |
|---|---|
| node / hook | a grab point |
| tether / rope | the line from the ball to the node |
| hooked / loose | whether the player is on a rope |
| buff | the timed state from a cyan node; needed to damage bosses |
| shelter / shield node | a gold node; tethered to one, flares cannot hurt you |
| KINDLE, FLARE, VENT, CALM | the Flare Titan's phases |
| starquake | the Magnetar's damage window |
| stasis | a hold where the world pauses around the player (fight entry, victory, menus) |
| arena | a boss fight's off-level area, entered by warp |
| eclipse | the world darkening before a fight |
| META | the permanent currency |
| census / starved frames | headless measurements (§2) |
| check mode | a boss's `*_CHECK` env var for scripted verification |
| J-builds | numbered phone test builds (J1-J11); only a label |
