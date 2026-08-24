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
- **Solar flare hazard** (`constants.rs`, `state.rs`, `hearts.rs`, `spawning.rs`) —
  telegraphed flares on a cooldown; an unshielded player (not within
  `FLARE_SHIELD_RADIUS` of a gold `shield_node`) loses a heart on eruption.
  Reaching `hearts <= 0` now ends the run regardless of cause.
- **Space right-boundary wormhole** (`space_zone.rs`, `constants.rs`) — mirrors
  the left-boundary rescue teleport on the right side so the special space zone
  stays bounded (generous `SPACE_RIGHT_BOUNDARY_MARGIN`), preventing a player from
  drifting into unmapped/boss territory while exploring.
- **Lighting + shadows wired into the game** (`build_scene.rs` on_enter) — calls
  `enable_lighting`, adds an attached player light, and marks the player / boss /
  danger floor as shadow casters. Default ambient is full-bright so normal play is
  unchanged; it makes darkness attacks and shadows actually render.
- **Mega-shader plumbing** (`scenes/game/fx.rs`) — `register_mega_shader` and
  `push_mega_fx` helpers wrapping the engine's `register_shader_source` /
  `push_mega_sprite`. WGSL shaders are authored separately (see the engine's
  `renderer/mega_shader/common_effects.wgsl` as a reference).
- **Boss arena tether nodes + wormhole warp** (`boss.rs`) — on boss entry the
  arena spawns a grid of climbable grab nodes spanning the boss Y band (every
  third one is a buff node), and the player is warped into the arena
  (wormhole-style flash) so the fight is entered cleanly. This makes the
  upper-sky boss reachable without superhuman swinging. A `force_boss_warp` var
  lets the headless harness force entry for testing (`--boss-warp`).

> **Not yet validated:** the boss weakpoint *damage* path is still not confirmed
> in the sim. `--boss-warp` now reliably warps the bot into the arena
> (`bossIn=true`), but the bot dies within ~350 frames (3 grabs) — it can't
> chain the 2D grid to reach a buffed weakpoint. `bossHP` stays at max. Full
> validation needs a more capable (or teleporting) bot. The warp uses a camera
> flash; a wormhole-gif overlay can be layered onto `warp_player_into_arena`.

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

---

## 11. Space zone follow-ups (exploration, density, and boss separation)

Follow-up ideas for the regular space zone (reached via the special rocket pads):

- **Large, bounded explorable area.** The right-boundary wormhole (implemented)
  caps how far right a space player can drift. Tune `SPACE_RIGHT_BOUNDARY_MARGIN`
  so the zone *feels* enormous (it's currently `VW * 6.0`) while never letting a
  player drift into unmapped or boss territory. The left boundary already caps the
  other side. Since space and boss mode are mutually exclusive in code
  (`tick_boss_zone_entry` returns if `in_space_mode`), a space player can't
  actually spawn a boss — the boundary is about keeping the explorable region
  bounded and readable, not hard-enforcing mode separation.
- **Density vs. ease.** Current space coin gaps are `SPACE_COIN_GAP_MIN/MAX =
  1400/2600`, with coin spawns largely via formations and planet-guided trails.
  To avoid "too sparse" without making pickups trivially abundant, bias the drop
  table toward **formations / guarded trails** (planet-guide red coins, sun-bonus
  clusters) rather than plain single coins. That keeps density visually rich while
  making the payout a *positioning* reward (risk), not a freebie. These are
  constants/weights to tune, not new systems.
- **Extraction loop** (from §5) remains the core reason to stay: bank vs. greed on
  oxygen. Oxygen pickups set the "how long do we gamble" dial.

## 12. Last boss — barrier, generators, and the bait-and-bail

This is a strong capstone because it forces the player to *use the arena's worst
hazard (the sun) against the boss* instead of just avoiding it.

**The arena (an upper-sky space arena):**
- A **protective barrier** spans the sun-facing edge below the arena. While it's
  up, neither the player nor the boss can fall into the sun (it also prevents the
  player's normal fall-death from reaching the kill zone here).
- **Generators** (say 3) power the barrier, placed around the arena. Each is a
  small destructible node. The barrier only drops once all are disabled.

**How you break the generators (pick one or mix):**
- *Dodge & destroy* — swing between generators, breaking them with buffed weakpoint
  hits, while the boss attacks.
- *Lure* — draw the boss's telegraphed attack (e.g. its dive/shock) into a
  generator, so the **boss breaks its own shield**. This teaches reading its
  patterns and is the more interesting option. Encourage it by making generators
  take reduced/no damage from the player but full damage from boss attacks.

**The bait-and-bail (final kill):**
- Once the barrier is down, the boss can be lured toward the sun edge. Its final
  (or a "desperation") attack is a big telegraph. The player **baits** the boss to
  aim/commit its attack at the player near the edge, then **swings away at the
  last second**, letting the boss's own momentum carry it into the sun. This is the
  high-risk, high-skill finisher and it uses the swing (the game's core) as the
  "dodge" the boss can't follow.

**Implementation sketch (fits existing `boss.rs`):**
- Add to `State`: `boss_barrier_up: bool`, `boss_generators: Vec<String>`,
  `boss_generator_hp: Vec<i32>`, `boss_generator_style: u8`. Reuse the boss
  arena-clear/repopulate pattern for spawning generators on entry.
- The sun-kill check in `tick_boss` is bypassed while `boss_barrier_up` is true;
  the barrier is a platform that clamps the player's `py` above the kill line.
- A boss "dive" pattern (already plausible from the movement steering) aims at a
  predicted player position; when the dive lands near a generator, destroy it.
- The final bait-and-bail: when all generators are down, the boss gets a
  "desperation" lunge with a long telegraph; landing the player's escape while the
  boss crosses the now-open sun line triggers the kill.

**DECISION NEEDED**
- Generators: `2` vs `3`, and whether they take player damage at all.
- Lure mechanic: is the boss's attack the only way to break a generator, or can
  the player also break it? (Recommend: player hits do reduced damage, boss attacks
  do full — so luring is faster and the intended path.)
- How long the barrier takes to drop and whether the sun becomes "dangerous to the
  boss but always safe to the player" after a shield-down grace.

## 13. Do space boss battles still need oxygen pickups?

**Recommendation: suspend the oxygen timer during space boss battles.** A boss is a
focused engagement that already demands dodging, buffing, and aim; layering a
separate "you must leave or die" timer on top double-punishes and forces the player
to abandon the fight. Options:

1. **Pause the drain** while `boss_active` in space — simplest and cleanest. Keep
   the oxygen bar visible but frozen with a "DANGER — OXYGEN SUSPENDED" indicator.
2. **Provide oxygen pickups inside the arena** if you want resource pressure to
   persist (raid-style). Place a few around the arena so "extend the fight" is a
   positioning choice, not a hard deadline.
3. **Refill on boss entry** (room starts full) — the weakest option; it just
   converts the timer into a gate and removes tension.

Recommend **option 1** for the first implementation, revisit **option 2** if the
final boss is meant to be a long, resource-tight gauntlet.

**DECISION NEEDED**
- Which option (1 vs 2) for space bosses, and whether the normal-space extraction
  loop (bank vs. greed) should be suspended or paused during a boss.
