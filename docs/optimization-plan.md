# Engine optimisation: what is measured, and what to do about it

Every number here came from an A/B on this machine, not from reading the code
and reasoning about it. Where something was measured and turned out NOT to
matter, it is recorded too — those entries are what stop the next person
spending a day on them.

Method: interleave the variants in ONE build (an env var picks the variant, so
build-to-build drift cannot contaminate the comparison), take medians, and
check the harness can tell the variants apart before believing a difference.

## The headline, and how it moved

The first measurement said "skipping invisible objects in `update_objects` cuts
the `objects` stage 83%", and the obvious conclusion — that walking 986 objects
is expensive — was **wrong**. Isolating the loop body one field at a time:

| Variant | `objects` | tick total | fps |
|---|---|---|---|
| baseline | 0.9 ms | 1.3 ms | ~780 |
| skip the whole body when invisible | 0.1 ms | 0.4 ms | ~2200 |
| skip, but still touch a far field | 0.1 ms | 0.5 ms | ~2160 |
| gate **only `update_animation`** | 0.1 ms | 0.4 ms | ~2190 |
| gate only `scaled_size` | 0.9 ms | 1.3 ms | ~785 |

Gating one call captures the entire win. The walk is not the cost, and neither
is cache traffic — touching a field at the far end of the 720-byte struct on
the skipped path changed nothing.

The cost is that `update_animation` **allocates**:

```rust
let mut img = sprite.get_current_image();   // clones the frame's pixels
self.drawable = Some(Box::new(img));        // heap alloc + free of the old box
```

That ran for every animated object every frame regardless of visibility, and
this game parks ~675 pooled objects off screen at any moment.

**Shipped** (`quartz/src/canvas/physics.rs`): `update_animation` is gated on
`obj.visible`. The rest of the loop stays unconditional, so an object revealed
later in the frame still has a correct `scaled_size`. Four tests in
`quartz/tests/invisible_animation.rs`, validated by removing the gate — the two
behaviour tests fail without it, the two guard tests (a visible object still
animates; a revealed object is refreshed the same tick) stay green, which is
what stops the gate being widened until it breaks something.

Gameplay is byte-identical across all variants: same distance, speed, coins and
hook grabs on a fixed boss run.

## Next: the same allocation, for VISIBLE objects

The gate removes the waste; it does not make the remaining work cheap. Every
visible animated sprite still clones its frame's pixels and heap-allocates a
`Box` every frame. On the phone during a boss fight, visible animated objects
are exactly the ones that are numerous.

The fix is to mutate in place when `drawable` already holds an `Image` of the
right shape, instead of building and boxing a new one. Measure first: this is
a bigger change than the gate and the gate may already have taken the device
frame under budget.

## Renderer `prepare`

Unexplained, and measured at up to **14.7 ms a frame** on device during a boss
fight. It is the other half of the budget and nothing here has been traced.

Do to it what was done to the tick: subdivide and log per second — buffer
writes, batching, atlas lookups — then attack whichever dominates. **Needs the
phone.** The desktop GPU is fast enough that the ratios there have already
proved misleading once.

## Allocation churn elsewhere

* `build_physics_bodies` rebuilds a `Vec` of ~986 `PhysicsBody` every frame,
  each with a `gravity_target` `String` clone. Reuse a persistent buffer on the
  `Canvas` (clear and refill). Safe, no coupling, no behaviour change.
  Same shape of bug as the one above, so worth measuring rather than assuming.
* The draw tree allocates a `RequestTree`/`SizedTree` per object per frame.
  Measured 0.0 ms on desktop; unknown on device.

## Structural, only if the above is not enough

Crystalline's `contacts.rs` indexes `bodies[contact.plat_idx]` by body **id**,
so `id` must equal the slice position. That coupling is what prevents
`build_physics_bodies` from skipping invisible bodies. Breaking it (an id→index
map, or compacting with a translation table) would let the whole physics path
ignore invisible objects the way the broadphase already does.

`physics` measures 0.2 ms on desktop and 1.0–1.6 ms on device, so this is last.

## Instruments

* **The tick profiler had no switch.** Every stage was wired up and
  `set_enabled` was called from nowhere, so it was permanently off and looked
  finished; measuring anything meant editing the default and rebuilding. It now
  reads `QUARTZ_TICK_PROFILE=1`, or `Canvas::set_tick_profile(true)` from code.
* **The headless harness is deterministic in `--boss` mode** — nine runs,
  identical distance, speed, coins and grabs — so it CAN validate that an
  optimisation preserved behaviour. Normal episode mode is not (two runs of
  identical code gave dist 15462/survived and 6666/died), so behavioural
  comparisons must use `--boss`.
* The profiler emits one line a second, so a run that finishes in under a
  second prints nothing. A faster variant can look like a crashed one.

## Measured and NOT worth doing

* **`update_image_shape` per visible object** — no change at all.
* **Offscreen draw culling** — removes nothing (986 objects produce ~300 draw
  items either way), because pooling already keeps visible ≈ on-screen. Kept as
  insurance for large visible off-view objects, not as a win.
* **Gating `scaled_size` on visibility** — nothing, and it costs correctness on
  the frame an object is revealed.
* **A large parked object costing solver time** — it does not, in release. That
  was a debug-build artefact; `opt-level = 3` covers dependencies, not the game
  crate.
* **Texture upload churn** — already 0/s in steady state.

## Loose end

`GameObject::grounded` is written twice per object per frame
(`physics.rs:105` clears it, `:413` sets it) and **read nowhere** in this
workspace. Crystalline's `grounded` is a different field on `PhysicsBody`. It is
`pub`, so it may be someone's API; it is not a measurable cost, just dead work
worth a decision.
