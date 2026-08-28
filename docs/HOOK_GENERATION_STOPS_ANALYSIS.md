# Procedural Generation "Stops" After a Certain Distance — RESOLVED

**Status:** Root cause found, fixed and verified 2026-08-27.
**Cause:** `spawn_upgrade_nodes` raised the hook-chain frontier (`rightmost_x`) to a
companion node placed up to 55,000 px ahead of the player, which switched the chain
spawner off until the player walked the difference.

*The earlier hypothesis in this document — transient hook-pool exhaustion — was wrong.
It is kept in §6 because ruling it out is part of the record.*

---

## 1. The mechanism

`spawn_hooks` only runs while the chain frontier is inside the generation window:

```rust
while hooks_spawned < HOOKS_SPAWN_BUDGET_PER_TICK
    && !s.pending.is_empty()
    && !s.pool_free.is_empty()
    && s.rightmost_x < s.px + GEN_AHEAD     // ← the gate that was being tripped
```

`spawn_upgrade_nodes` places an upgrade node at `upgrade_rightmost + gap`, where
`UPGRADE_GAP_MIN/MAX` is **30,000–55,000 px**. It then placed a companion grab node
beside it so the player can always tether out of the upgrade dialogue — and did this:

```rust
s2.live_hooks.push(hid);
if hx > s2.rightmost_x { s2.rightmost_x = hx; }   // ← the bug
```

`upgrade_rightmost` starts at `SPAWN_X + VW * 1.4` ≈ 6,221, and the spawn condition
(`upgrade_rightmost < px + GEN_AHEAD`) is already true on frame 1. So the first upgrade
node — and its companion hook — is placed **within the first second of every run**, at
x ≈ 36,000–61,000. The frontier jumps there, and with `GEN_AHEAD = 13,440` the chain
spawner is dead until the player reaches `rightmost_x − 13,440`.

It then recurs every 30,000–55,000 px, forever.

## 2. Measured, not inferred

Frontier trace, first seconds of a normal-mode run:

```
FRONTIER t=0   px=1005  rightmost=3309   overshoot=2304   live=6  free=62 pending=40 blocked=false
FRONTIER t=60  px=1952  rightmost=49603  overshoot=47651  live=31 free=37 pending=16 blocked=true
FRONTIER t=120 px=1566  rightmost=49603  overshoot=48037  live=31 free=37 pending=16 blocked=true
...blocked for eight seconds...
```

`49,603` is exactly `upgrade node x + UPGRADE_R + HOOK_R + 60`.

## 3. Why the pool-exhaustion hypothesis was wrong

Three independent reasons:

1. **The trace shows `free=37` at the moment of the block.** The pool was two-thirds
   empty, not exhausted, and `pending=16` hooks were queued and waiting.
2. **This document's own observation disproves it.** It records that the
   `ensure_player_hooks` failsafe nodes *were* appearing in the blank stretch. That
   failsafe pops from the same `pool_free`. If the pool were empty it could not have
   placed anything either.
3. **Pool exhaustion is transient by nature** — the culler catches up within a screen or
   two. The reported symptom was a stop that persisted, and recurred at a similar
   distance after every respawn. That is a latched cursor, not a spike.

The headless bot never saw it because it dies around 6,000–10,000 px and the first
upgrade node sits at ≥ 36,000 px — which is why the earlier run reported `starved=0/93`.

## 4. The fix

**a. Auxiliary hooks no longer advance the chain frontier.** Five call sites borrow from
the shared hook pool (chain spawner, upgrade companion, gate companion, respawn
checkpoint, two debug spawners). Only the chain spawner may raise `rightmost_x`; the
upgrade companion and the two debug spawners no longer do. The respawn checkpoint node
still does, deliberately — it sits *behind* the player and so cannot open a gap ahead.

**b. A self-healing guard.** `rightmost_x <= gen_head_x` holds by construction: the
spawner only places specs the generator already produced. `spawn_hooks` now repairs any
violation and counts it in `frontier_repairs`, so a future sixth call site degrades to a
one-frame correction instead of switching generation off.

**c. A source-level test.** `sim_tests::only_sanctioned_code_raises_the_hook_chain_frontier`
scans the tree for writes that *raise* the frontier and fails on any outside the
allowlist, naming the file and line. Lowering writes (respawn/boss backfill `.min(...)`)
are always safe and are not counted.

**d. Telemetry.** The headless summary now prints
`worst_frontier_overshoot` and `frontier_repairs`. Healthy steady state is an overshoot
a little above `GEN_AHEAD` (the excess is the player swinging backwards) and zero repairs.

## 5. Verification

A/B over 6 episodes each, same build, bug reintroduced and removed:

| | with the bug | fixed |
|---|---|---|
| starved frames (in-band) | **47.2%** | **16.3%** |
| worst frontier overshoot | 21,237 px | 15,458 px |
| frontier repairs | 6 | 0 |

Over 20 episodes after the fix: `frontier_repairs=0`, `worst_frontier_overshoot=16,473`
against `GEN_AHEAD=13,440`. 30 tests pass; boss and solar-flare paths unaffected.

## 6. Ruled out (kept for the record)

- **Hook-pool exhaustion** — see §3. The pools are released correctly by the cull
  functions and there is no leak; the original audit of the pool lifecycle was accurate,
  it just was not the cause.
- **Unreachable-node generation** — hops are clamped to the reach envelope
  (`hop_dy_budget` / `clamp_into_envelope`) in both the generator and the spawner.
- **The stride-bound inversion** — genuinely existed, genuinely fixed earlier; both
  stride bounds are now interpolated so `lo <= hi` always holds.

## 7. Still open

`UPGRADE_GAP_MIN/MAX` of 30,000–55,000 px means the upgrade node and its companion are
placed roughly 40,000 px before the player can reach them, and the companion holds a slot
in the 68-entry hook pool for that whole stretch. Harmless now that it no longer stalls
generation, but placing both lazily — when the player is within `GEN_AHEAD` — would free
the slot and is the tidier design.
