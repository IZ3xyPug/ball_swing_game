# Ball Swing — Design & Implementation Plan

> Status: **active build.** This document records what the headless sim told us
> about the current loop, then lays out — concretely, against the existing code —
> how to implement the hearts/boss/buff/space/hazard/roguelike/monetization ideas
> we discussed. Specifics flagged as **DECISION NEEDED** still need a human call.

## Implemented so far (validated via headless)

- **Hearts + checkpoint respawn** (`src/scenes/game/hearts.rs`, `state.rs`,
  `build_scene.rs`, `bootstrap.rs`, `images.rs`) — `MAX_HEARTS = 3`. Falls cost a
  heart; the first two respawn at the last auto-progress checkpoint (orbit-in),
  the third ends the run. Headless: 3 heart losses then `death=fall`, `heartsEnd=0`.
- **Buff tether nodes** (`constants.rs`, `spawning.rs`, `events.rs`, `physics.rs`,
  `hearts.rs`) — ~5% of spawned hooks are cyan `buff_node`s; grabbing one grants a
  10 s buff that raises the momentum cap to `BUFF_MOMENTUM_CAP = 84` and enables
  boss weakpoint damage.
- **Boss contact inversion + weakpoints** (`boss.rs`, `constants.rs`) — body
  contact no longer damages the boss. A buffed hit near a weakpoint
  (`BOSS_WEAKPOINT_OFFSETS`) does damage; unprotected contact knocks the player
  back and unhooks the tether.
- **Boss darkness telegraph** (`boss.rs`) — periodic dark phase via the quartz
  lighting system (`set_ambient`), telegraphed and cleared on boss kill. No-op
  until `enable_lighting` is on.

> **Not yet implemented:** solar flares + shielded nodes (Stage 3), and a
> boss-reach test in the sim (the naive bot doesn't reach the 20 000 px threshold).

---

## 0. The headless harness (what we built)

A window-less simulation driver that boots the *real* `Canvas` + game scene and
drives the engine tick loop without a GPU surface. The game implements its own
rope physics inside the `on_update` callbacks, and the engine runs those plus
crystalline physics from `OnEvent::on_event(TickEvent)`, so we drive `on_event`
directly and never call `draw`. A small auto-swing policy injects space
press/release keyboard events and reads player/hook state from the canvas.

**Files**
- `src/headless.rs` — driver: builds the canvas, drives frames, auto-swing policy,
  metrics. Public `run(episodes, frames, boss_mode) -> AggregateReport`.
- `src/bin/headless.rs` — thin CLI.
- `src/lib.rs` — added `pub mod headless;`.
- `Cargo.toml` — added `[[bin]] name = "headless"`.

**Usage**
```
cargo run --bin headless -- --episodes 12 --frames 6000        # free roam
cargo run --bin headless -- --episodes 4  --frames 9000 --boss # boss mode
```

**Two pre-existing compile breaks had to be fixed to build anything** (worth
noting — the crate did not compile before this work):
- `ramp::run!` embeds `$CARGO_MANIFEST_DIR/resources` with `include_dir!`; the
  directory didn't exist → created `resources/README.md` (empty, documented).
- `quartz::AnimatedSprite::current_frame_index()` was removed but is still called
  in `src/scenes/game/events.rs:177` and `visuals.rs:144` → re-added the public
  getter in `quartz/src/sprite.rs`.

---

## 1. What the headless run told us about the *current* gameplay

Free-roam batch, 12 episodes, ≤6000 frames each:

| Metric | Value |
|---|---|
| Panics | 0 |
| Deaths (fall) | 0 |
| Space entries | 1 |
| Max zone reached | 1 (purple, ≥20 000 px) |
| Avg max distance | ~10 125 px |
| Best distance | 28 350 px |
| Avg max speed | ~68 (clustered at 56.9 normal-cap or 74 special-hook-cap) |
| Hooks grabbed | ~84 per episode (range 47–153) |
| Coins | 47 total |

**Read-offs for design**
- **Hooks are reachable** — even a naive bot that re-grabs the nearest hook never
  fell off screen in ~10 000 frames. `HOOK_FIXED_X_GAP = 1250`, `ROPE_LEN_MAX = 720`,
  stride 1160–1340 is a forgiving, chainable spacing. Good, but it means "fall = run
  over" is currently a *rare* fail; hearts/checkpoints will mostly matter in the
  boss arena and space, not the base swing.
- **Special-hook boost is the single biggest progression lever.** Episodes that
  hit a special hook (cap 74) travelled far more than ones stuck at the 56.9
  normal cap. This directly supports the "buff tether nodes" idea — the wiring for
  a buff-charged boost already exists (`SPECIAL_HOOK_*`).
- **No deaths** also means the *base* loop currently has little tension. The
  interest lives in hazards (spinners, gates, turrets, cannons, comets) and the
  space/boss layers — which is exactly where the new ideas plug in.
- **Space entry happens** via rocket pads (2.8% of pad slots). The space zone is
  reached but there's no *reason* to stay — confirming the "give it a purpose" need.
- A boss-mode run didn't reach the boss threshold (≥20 000 px) — the bot's
  oscillation keeps it around ~10–14k. To exercise the boss arena in the sim we'd
  want a smarter policy or a debug teleport; this is itself a finding.

---

## 2. Hearts + Checkpoint respawn

This is the keystone: it turns "fall = run over" into a run with lives and a
persistent last-save, which is what makes boss fights and roguelike structure
possible.

**Where it lives**
- `src/state.rs` — add to `State`:
  - `hearts: i32`, `max_hearts: i32`
  - `checkpoint: (f32, f32)` and `checkpoint_hook_id: String`
  - `respawn_phase: u8` (0 = none, 1 = orbit-in, 2 = active) and `respawn_ticks`
- `src/scenes/game/build_scene.rs` — replace the fall branch of the death check
  (around lines 1858–1903) so a fall **decrements hearts and respawns** instead of
  hard-loading `gameover`.

**How to respawn** (reuse existing patterns)
- The intro-orbit (`start_prompt_active`, `ORBIT_R = 240`, `ORBIT_OMEGA = 0.038`,
  momentum zeroed, camera anchored on hook) is exactly the "come back in rotating
  around the node until you first grapple" feel. Run the same block on respawn:
  set position to `checkpoint`, velocity (0,0), gravity 0, `start_prompt_active = true`,
  `game_paused = true`, `start_orbit_ticks = 0`, and a short "RESPAWN — GRAB TO SWING"
  prompt.
- **Auto-progress save**: save on every completed `PASSIVE_SCORE_BLOCK_SIZE` (5000 px)
  block, not on a wall-clock timer. On block boundary, set `checkpoint` to the
  nearest generated hook (or the last hook that was live and reachable). This is
  legible ("every 5000 px") and always lands on a safe node.
- **Boss arena special case**: if `boss_active`, respawn *inside* the arena on a
  dedicated arena anchor hook, not at the world checkpoint (which can be behind the
  arena wall). The arena already clamps `px` in `tick_boss_zone_entry`; add a
  similar clamp for respawn Y / anchor.
- **State to reset on heart loss**: zero_g, score_x2, active buff, current hook, rope.
  Decision: whether a heart loss also resets the passive-score "golden" bonus.
- **Oxygen / sun**: recommend they remain **run-ending** (not a heart cost) so the
  space zone keeps stakes. DECISION NEEDED.

**DECISION NEEDED**
- Hearts on the HUD (image vs count). Keep it light — a row of heart icons.
- Does losing a heart reset `zero_g`/`score_x2`? (Recommend yes.)
- Checkpoint cadence: 5000 px blocks, or a more forgiving 10000? (Recommend 5000;
  too generous removes tension.)
- Heart regen between blocks, or only pickups? (Recommend: hearts are a resource;
  a rare heart pickup in space/hazard zones, not passive regen.)

---

## 3. Boss design

The `boss.rs` module is a solid base but has one core problem we should invert.

**Invert the contact rule (the key change)**
- Today `tick_boss_player_hits_boss` reduces `boss_hp` on *any* body contact and
  bounces the player off. With hearts, touching the boss body should cost *you*
  (heart + knockback), and only **buffed weakpoint hits** should damage the boss.
- Add to `State`: `boss_weakpoints: Vec<(f32, f32, bool_open, u8 kind)>` and
  `player_buff: Option<u8/>`. Only an open weakpoint matching the current buff deals
  damage; it should be *telegraphed* (a glowing ring / projected pulse) so it's a
  timing/positioning read.

**Attacks, grounded in what exists**
- The boss already shoots bolts (`BOSS_BOLT_*`) and disconnects the tether on hit
  (`tick_boss_bolt_player_collision`). Extend into a 4-archetype set, each with a
  counter:
  1. **Area damage** — telegraph a zone (see §5), then damage on entry.
  2. **Knockback shock** — an expanding ring that throws the player off the rope and
     *forcibly unhooks* (`s.hooked = false` + hide rope). Best combo-breaker.
  3. **Tether-disconnect pulse** — momentary forced release across the arena.
  4. **Darkness** — use the quartz lighting module (see below).
- **Darkness attack via lighting**: quartz already has `LightSource` (Point/Spot/
  Directional), `AmbientLight::dark()` (strength 0.06) / `dim()` (0.2),
  `LightEffect::Flicker/Pulse`, `casts_shadows`. A "darkness" phase is a config lerp:
  push ambient to `dark()`, enable a few `casts_shadows` boss platforms, then restore.
  No new shader needed. Cache/lerp via `lighting/system.rs` tick.
- **Telegraphs**: generalize the existing `CometWarn` (warning object + reserved
  object + `timer`) into a reusable `Telegraph` helper; boss AoE and solar flares
  both use it.
- **Movement patterns, not just steering**: add discrete patterns (slow circle +
  sweeping laser, "dive to predicted player position" leaving a shockwave, "orbit &
  rain") and clean phase transitions that also **clear the player's buffs** on phase
  change (reuse the arena-clear block in `tick_boss_zone_entry`).

**DECISION NEEDED**
- Boss size/HP budget. `BOSS_SIZE = 360`, `BOSS_MAX_HP = 20`, contact = 1 hp.
  With hearts, rework HP so one weakpoint hit = e.g. 2 hp and the fight reads
  better (fewer, more meaningful hits).
- Weakpoint count and minimum buff duration so a buff isn't wasted.
- How many bosses in the roster and roughly how many distinct attack patterns each.

---

## 4. Buff tether nodes (combat loop)

Build on the existing special-hook wiring (`SPECIAL_HOOK_*`, the green artifact
gif pause/resume in `events.rs`).

- Add a `buff_node` tag distinct from `SPECIAL_HOOK`. On grab, grant a typed,
  timed buff: a distinct hook-glow color + a distinct player-trail emitter swap
  (`rebuild_player_trail`) + a distinct contact/projectile flash. Buffs are
  visual+mechanical, not just cosmetic.
- Buff types and their **standard-loop use** (so they matter outside bosses):
  - **Tether** — grab asteroid-hooks / gwell nodes you normally can't.
  - **Phase** — pass through gates/spinners once (lane switcher).
  - **Magnet** — stronger coin/cat-coin pull in space.
  - **Tempest** — redirect the *next* comet instead of just dodging it.
- Weakpoints match a buff type; the boss telegraphed its open weakpoint.

**DECISION NEEDED**
- Buff duration (ticks). Pick so a buff survives from node to boss without trivializing.
- How buffs stack (one at a time vs. multiple). Recommend **one active buff** —
  simpler to read and to telegraph against.

---

## 5. Space zone: purpose + oxygen

The zone already has the strongest pull (red cat coins at 25×, blue 5×) but no
*reason to stay and master it*.

- **Oxygen pickups**: a new pickup type. Amount refilled scales with placement risk
  (high above, behind a planet, near a blackhole → bigger refill). So grabbing one
  is a positioning micro-decision, not a survival afterthought.
- **Space is the only place to earn the meta/premium currency** (see §7), so it's the
  intended farming loop.
- **Space bosses / boss variants** — the most on-theme arena (planet gravity +
  blackhole teleport + darkness + tether-disconnect all combine).
- **Gravity slingshots as shortcuts** — planet attractions already exist
  (`gravity_influence_mult = 3.0`, `gravity_target`); a skilled slingshot could skip
  distance or reach a high-value node — a mastery reward that doesn't scale numbers.
- **Blackhole wormholes** as optional fast-travel to a far-ahead bonus lane with
  big reward but low oxygen.
- **Extraction loop instead of hard death**: when oxygen runs out, eject the player
  back to the surface with **banked** coins, losing unbanked ones. This turns space
  into a risk/reward bank-vs-greed loop and avoids double-punishing with hearts.

**DECISION NEEDED**
- Hard-death vs extraction on oxygen-out. Recommend **extraction** (bank vs greed).
- Whether oxygen refills fully on space re-entry (recommend yes, since risk is the
  timer).
- What the space-only currency is called and how much a "farm trip" yields.

---

## 6. Creative hazards & mechanics

Current hazards are largely *denser/faster* in Purp/Beginning-to-Black
(`SPINNER_*_MULT`, zone colors). We want **mechanical** escalation: each hazard is
a telegraph + a counter. These all reuse existing systems.

- **Solar flares** (your idea) — telegraph via generalized `CometWarn`; player must
  reach a shielded node before it erupts or lose a heart. Optionally strips ambient
  light during the flare (composes with the lighting module and the darkness attack).
- **Harmonic gates** — a gate that oscillates passable/solid at a frequency matched
  to your swing rhythm, so you must *time* the release.
- **Tether-snap zones** — areas that cut the rope, forcing momentum-only traversal
  over a gap. Tests the core.
- **Dark / eclipse zones** — `ambient` to `dim()/dark()`; hooks glow within a radius.
  Turns the visual system into a gameplay system.
- **Gravity-current "wind" bands** — a horizontal region that pushes the player.
- **One-way hooks** — grabbable only from one direction (swing with the current).
- **Turbulence zones** — small randomized impulses each tick (cheap constant chaos).
- **Comet storms** — multiple forecast comet tracks in the Black zone.
- **Combined hazards** (the real Black-zone difficulty): e.g., a tether-snap zone
  during a solar-flare warning, with the shielded node inside a gravity current.
  That's a genuinely new problem, not a bigger number.

**DECISION NEEDED**
- Order/placement: which hazard appears first in the Black zone. Recommend the
  **solar flare + shielded node** first — it reuses the most existing code and reads
  clearly.
- Whether eclipse/dark zones are a hazard type or only a boss attack. Recommend both.

---

## 7. Roguelike + boss rush + time trial

The game is intrinsically session-based (one life, one distance), which suits this.

- **Roguelike run**: a seeded distance run; add **pick-up-once upgrade nodes** that
  mutate the run (binary, legible, node-driven). E.g. "release surge stronger but
  reach shorter," "coins worth more but hearts fewer."
- **Boss rush**: a linear sequence of the boss roster with short cooldowns; compose
  cheaply with existing arena-clear logic.
- **Time trial + leaderboards**: a pure distance/time mode (how far in X seconds)
  is the least RNG-affected and the best leaderboard. Recommend this as the first
  competitive mode.

**DECISION NEEDED**
- Whether to gate time-trial / boss-rush behind unlocking bosses (recommend yes).
- Leaderboard scope (region, platform) and anti-cheat approach — a server-less
  leaderboard is fragile; decide if we self-host or use a provider.

---

## 8. Free-to-play monetization (no pay-to-win)

The shop is already cosmetic-oriented (characters, ropes, backgrounds, trails). Keep it that way.

- **Two currencies**: existing coins (soft, earned) + a premium "Stardust" that is
  **also earnable in-game** by playing.
- **All purchasables are cosmetic** — coolness is the product. Buy power? No.
- **Battle pass / seasonal track**: free track always reaches some Stardust +
  cosmetics; paid track reaches *more* cosmetics and *more* of the earnable
  currency — never a stat advantage. Reuses the existing daily-reward scene.
- **No loot boxes / randomized purchases / gacha** — deterministic cosmetics,
  direct purchase or earned.
- **Grind is the price of everything** — every purchasable cosmetic is reachable by
  play within a realistic (non-tedious) time. "I can earn the skin in ~20h" is fine;
  a whale-only skin is not.
- **Daily login + streaks** (already have `daily_reward` scene) is healthy engagement.

**DECISION NEEDED**
- Names of the premium currency and the "space-only" farming resource (should they
  be the same? Recommend the space-only resource *is* the premium currency's source).
- Season length and free-track size.
- Priorities: which of these lands in a first 0.x build vs. later.

---

## 9. Suggested implementation order (what to build first)

Each is small, reuses existing systems, and validates the thesis early.

1. **Hearts + checkpoint respawn** (normal loop; reuse orbit-in). Smallest change,
   biggest feel shift. Founds everything else.
2. **Invert boss contact rule + one "darkness" telegraph** via the lighting module.
   Proves the boss-fight *feel* cheaply.
3. **One buff node + one telegraphed weakpoint** to validate the
   "buffed weakpoint hit" loop end-to-end.
4. **Solar flare + shielded node** reusing `CometWarn`. Validates the
   "telegraph + counter" pattern all other hazards follow.
5. Then the roguelike structure (upgrade nodes), the space purpose/oxygen, and the
   monetization shell.

---

## 10. Open items to settle before we implement

- [ ] Heart count / HUD, and whether heart loss resets power-ups.
- [ ] Checkpoint cadence (recommend 5000 px blocks) and boss-arena respawn anchor.
- [ ] Hard-death vs extraction on oxygen-out in space.
- [ ] Boss HP/weakpoint budget (rework `BOSS_MAX_HP` / contact damage).
- [ ] One-active-buff rule + buff durations.
- [ ] First hazard for the Black zone.
- [ ] Space-only currency name & farm yield; premium currency name.
- [ ] First competitive mode (recommend time trial) and leaderboard hosting.
- [ ] Which pre-existing breaks to keep fixed (resources dir + `current_frame_index`).

Once we decide these, the next step is to implement item 1 (hearts + checkpoint)
and re-run the headless sim to compare distances/deaths before/after.
