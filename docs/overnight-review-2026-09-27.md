# Overnight review — the final boss, and the difficulty curve

Two assessments asked for on the night of 2026-09-26:

> "based on realistic difficulty, but factoring in likely earned upgrades which
> boss should be the final boss in normal mode? And I think the rest of the
> bosses should be randomized in both boss rush and in normal mode (other than
> our testing order selector which will stay in until we have a full release)
> other than the hardest boss being the final boss."

> "do a review of how our procedural difficulty does increase to make sure that
> it is noticeable throughout the course of normal mode and casual mode, and
> that we are making use of all the difficulty levers we should be, and I'm
> fine if you get creative with your suggestions"

Numbers below come from the source as of this morning. The curve table and
the hazard census were measured with the headless driver (`--at-minute N`).
The boss ranking is a reading of each fight's parameters, not a playtest. The
autopilot cannot fight bosses reliably enough to rank them, so treat the
ranking as a strong hypothesis that one playthrough can confirm or overturn.

---

## Part 1 — Which boss closes the run

**The Magnetar.** It is now `FINAL_BOSS` in `src/constants.rs`, and it is last in
Normal and in Boss Rush. Every other boss is dealt in a random order per run
(`deal_roster`, stored in `State::boss_roster`). The main-menu boss-order
selector still overrides everything, as asked.

### The seven fights side by side

| Boss | Hits to kill | When you can hurt it | What hurts you | Upgrades that blunt it |
|---|---|---|---|---|
| Sun Devourer | generators, then 20 buffed hits | once every generator powering its dome is down (buffed hits, or bait it into them) | bolts, telegraphed lunges, contact | hearts |
| Colossus | parts: hands, torso vents, head | after each lunge/clap (30–60-tick delay, then open) | lunges and claps with 60-tick telegraphs | hearts, FLYWHEEL (escape lunges) |
| Serpent | 8 segments × 1 + tail 4 + head 5 = 17 | exposed pieces as it passes | contact (45-tick cooldown), the hunting head | hearts, LONG LINE |
| Conductor | Resonance: 7 kicks, then 5 s armed | only after 7 on-beat releases | one attack per bar, contact | **none** — rhythm is not purchasable |
| Gravity Weaver | 4 spindles × 2 + loom 14 = **22** | the unwind after a shuttle; every live part after a flip (every 11 s, 7 s at the loom) | shuttle threads across the arena; falling "up" when gravity flips | hearts, LONG LINE (staying tethered through a flip) |
| Flare Titan | 4 vents × 2 + core 12 = 20 | after prominences; the VENT phase opens everything | prominences; **the flare: a burn every 2.5 s unsheltered** | **SUNPROOFING** (see below), LONG LINE (reach a shelter) |
| Magnetar | 2 poles × 3 + core 12 = 18 | **only in a starquake** — 5.5 s out of a ~17 s cycle | two sweeping beams (then four from the bare core); pull/push pulses that punish being loose; contact | hearts only |

### Why the Magnetar and not the others

1. **No upgrade answers it.** SUNPROOFING shrugs off one flare tick per level.
   A Titan flare burns twice (`TITAN_BURN_GRACE` 120, then every
   `TITAN_BURN_INTERVAL` 150 within a 300-tick flare). So SUNPROOFING 2, 884 META
   and well within reach by the late game, makes the Titan's signature mechanic
   free, and the fight shrinks to its prominences. The Weaver was softened on
   purpose (two-hit spindles). The Magnetar's threats are beams you must read
   and pulses you must be roped through, and nothing in the upgrade pool
   touches either. VITALITY helps everywhere equally, so it does not change the
   ranking.
2. **Its windows are the scarcest.** A Magnetar cycle is two beam cycles (dark
   110 + charge 70 + live 60 ticks each), a pulse (70 + 150), then the 330-tick
   starquake: about 1,030 ticks between openings, and damage only inside that
   5.5 s. The Weaver opens after every flip, every 11 s, 7 s in its core phase.
   The Titan opens after every prominence and in every VENT.
3. **It is a test, not a teaching fight.** Its header comment calls it "everything
   the run has taught... at the speed of a last boss": lanes that clear before
   the strike (Colossus), the shelter of a rope (Titan), the window after the
   attack (every boss). A finale that combines earlier lessons is fair. One that
   introduces a new verb is not.
4. **Why not the Conductor?** It is the only fight where the skill is not
   swinging. Players who do not read rhythm well would hit a wall that no upgrade
   and no amount of swinging practice moves. That is a risky thing to put
   between a player and the end of an 80-minute run.
5. **Why not the Weaver?** It is the runner-up: 22 hits, and the world turns over.
   But it hands out a window after every flip, and a player with LONG LINE rides
   flips out tethered. It is the right late-middle boss and the wrong finale.

**Estimated order, hardest first, for a player with typical late-game upgrades:**
Magnetar → Gravity Weaver → Conductor (high variance: easy for rhythm players)
→ Flare Titan (drops two places with SUNPROOFING 2) → Serpent → Colossus → Sun
Devourer.

**What "likely upgrades" means here.** A full Normal run pays about 608 META:
288 for distance (1 per 5,000 px), 20 for visiting space, and 6 × 50 for bosses
before the finale. Maxing every upgrade costs 14,669. A player reaching fight 7
has probably played enough to own VITALITY 1–2, LONG LINE 2–3, FLYWHEEL 1–2,
SUNPROOFING 1–2 and SECOND WIND 1, while the new cosmetics compete for the same
META.

### A consequence of randomizing the other six (please weigh in)

Several fights were tuned for a position. The Conductor says "it is the second
fight of the run, so the cycle is faster and each hit cheaper... its job is to
teach timing". The Weaver says "Fifth fight of the run... so it escalates rather
than teaches". Dealt at random, a new player can meet the Weaver as fight 1 at
minute 11, and the Sun Devourer as fight 6, where its eclipse lesson arrives
after the player has already seen five eclipses.

Two options if that proves rough in play:

- **Tiered deal** (recommended): shuffle within tiers, not across them. Tier 1 is
  Devourer, Colossus and Serpent. Tier 2 is Conductor, Weaver and Titan. The
  Magnetar is always last. It is the same `deal_roster`, with two Fisher-Yates
  passes instead of one. Runs still vary, but the arc still rises.
- **Slot scaling**: keep the full shuffle, but let the fight read its slot
  (`boss_index`) and scale cycle speed and hit costs, so an early Weaver is a
  gentler Weaver.

The BOSS_ROSTER comment ("The Sun Devourer opens the run") no longer describes
the game. It has been updated to say the list is the set of bosses and the
fallback order.

---

## Part 2 — Is the difficulty increase noticeable?

**Short answer: not in the first 15–20 minutes, which is most of a Casual
session and the first chapter of Normal.** After that the ramp is real and
readable, with a new hazard in every chapter. The curve's shape, not its
content, is what hides it.

### The curve today

`difficulty_t` is a smoothstep from minute 2 to minute 80
(`ASSUMED_PLAYER_PX_PER_SEC` = 300). A smoothstep starts with zero slope, so the
opening barely moves:

| Minute | 5 | 10 | 15 | 20 | 30 | 40 | 50 | 60 | 70 | 80 |
|---|---|---|---|---|---|---|---|---|---|---|
| `t` now | 0.004 | 0.029 | 0.074 | 0.135 | 0.294 | 0.481 | 0.670 | 0.836 | 0.955 | 1.0 |

At minute 10 the base curve (hook stride, vertical budget, hazard gaps) has
moved 3% of its range. At minute 20, 13.5%. Everything that makes the early game
change is the hazard schedule (`hazards.rs` STAGES): spinners at 6.6 min,
gravity wells at 16, turrets at 25.4, drifting asteroids at 37.4, comets at 48,
solar flares at 58.6. Each one also arrives at its rarest and takes about 25
minutes to mature.

### What the world actually looks like (measured)

Peak objects alive at once (headless census, `--at-minute N`, 4 one-minute
episodes each):

| Minute | Hooks | Pads | High asteroids | Spinners | Gravity wells | Turrets | Drift asteroids |
|---|---|---|---|---|---|---|---|
| 0 | 39 | 5 | 13 | 0 | 0 | 0 | 0 |
| 5 | 38 | 5 | 14 | 2 | 0 | 0 | 0 |
| 20 | 35 | 4 | 14 | 2 | 0 | 0 | 0 |
| 30 | 38 | 4 | 12 | 6 | 1 | 1 | 0 |
| 40 | 32 | 3 | 10 | 6 | 3 | 2 | 1 |
| 60 | 34 | 2 | 3 | 5 | 3 | 3 | 4 |
| 70 | 31 | 2 | 5 | 5 | 3 | 2 | 6 |

Minutes 0 and 20 are nearly the same screen. Two spinners appear, the supports
have not started thinning, and hook spacing has not moved. From 30 on, the
change is plain: hazards triple, and the supports that catch mistakes (pads and
high asteroids, 18 → 5) thin out.

(The autopilot's hearts-lost count is useless for this. It falls to its death at
the same rate at minute 0 as at minute 70, so it measures its own swinging, not
the curve. The census is the honest instrument.)

### Casual

Casual uses Normal's curve with no bosses and pays 0.35× META. So:

- A 15-minute Casual session never sees a gravity well, and a 25-minute one
  never sees a turret.
- Past minute 80 nothing changes, ever. The curve is flat at 1.0 and nothing
  else escalates. `zone_lap()`, written "for effects that should escalate on
  each lap", has no caller.

### Levers in use, and levers left on the table

**In use.** Hook stride (58–72% → 80–97% of reach) and vertical budget
(45% → 100%). Six hazards with their own introduce/mature schedules. Hazard gaps
(down to 0.55×). Spinner reach and scale, gravity-well size, turret fire rate,
comet count and speed, drift-asteroid share. The flare interval (90 s → 35 s).
Support thinning (pads to 0.45×, high asteroids to 0.55×). The eclipse before
each fight.

**Never scaled.** Rope length (`ROPE_LEN_MIN/MAX` fixed), gravity, the momentum
cap, node drift speed, pickup and heart frequency, hook size, the telegraph
lengths (on purpose, and rightly), camera look-ahead, and anything after minute
80.

### A fairness finding: run flares can start with the shelter out of reach

The flare may begin its 3 s telegraph if a shielded node lies anywhere from
2,200 px behind to 6,000 px ahead (`FLARE_SHELTER_SEARCH_*`). Shelters sit up to
4,800 px apart (`SHIELD_NODE_X_GAP`). Damage starts 4 s in (telegraph + 1 s
grace) and ticks every 2 s. So even a perfect player who keeps swinging forward
can meet a flare whose nearest shelter is up to ~4,800 px ahead. It has not
spawned in yet: nodes arrive hidden and animate in as you approach. That costs 1–2
hearts no matter what they do. In the headless sweep every late-curve flare
began with the nearest shelter about 5,000 px ahead and invisible. That figure is
inflated because each test episode starts in a fresh world, but the rule
permits it in a real run too.

This is the same problem the Titan arena had last night ("it has to be
reasonable to find a node before too much damage is done"), and the same fix
applies: tighten `SHIELD_NODE_X_GAP` to about 3,000 and the search-ahead to
about 3,200. The window then still always contains a shelter, and the worst case
is reachable before the first tick. The Titan's nearest-shelter reticle
(`titan_guide_to_shelter`) could also point the way during run flares. **Not
changed** — it is a tuning call, and yours to make.

The headless audit that was meant to catch this was itself wrong. It ignored
hidden nodes, so it flagged nearly every late flare as a violation. It now counts
live nodes whether shown or not, and also prints how far the nearest shelter was
at each flare (`nearest_shelter_at_flare avg=… worst=…`). That distance is the
number to watch.

### Recommendations, in order of payoff

1. **Give the curve an early slope.** Swap the smoothstep for an ease-out, or
   a blend of the two. `1 − (1 − u)²` reaches 0.20 at minute 10 and 0.41 at
   minute 20, against 0.03 and 0.135 today, and still lands at 1.0 at minute 80.
   Hazard stages are separate fractions and do not move. Only stride, vertical
   budget and gaps start moving sooner. This is the single change that most
   fixes "is it noticeable".
2. **Give Casual its own, shorter curve.** Casual sessions are short and have no
   bosses to pace them. A 40-minute Casual curve with the same stage fractions
   brings spinners at 3.3 min, wells at 8, turrets at 12.7, asteroids at 18.7,
   comets at 24 and flares at 29.3. It is one `DIFFICULTY_FULL_MINUTES` per
   mode, read through `GameMode`.
3. **Escalate past the top with `zone_lap`.** Once Casual passes the top of its
   curve, each zone lap (about 5.3 minutes) adds a *mutator* — a named, announced
   twist that stacks: "HEAVY SKIES" (gravity +8%), "FRAYED LINES" (max rope
   −6%), "RESTLESS NODES" (node drift ×1.5), "SCARCE HEARTS", "SOLAR STORM"
   (flare interval −20%). It gives an endless mode something to announce, and the
   player something to reach for ("I got to mutator 4").
4. **Add tension and release.** A monotonic ramp is invisible from the inside,
   and contrast is what a player notices. Ride a gentle wave on top of `t`: a
   ~45 s *surge* (gaps −15%, a hazard pair) then a ~20 s *breather* (a pad, a
   coin arc, no new hazards), once per zone cycle. Players will call it
   "the game got harder" at the surges without the average changing.
5. **Announce the chapters.** Each STAGES entry already has a name ("GRAVITY
   WELLS"). A one-time banner and a single sting when a hazard first appears
   turns an invisible schedule into a remembered milestone. If a banner already
   shows, add the sting and a brief slow-mo on the first instance.
6. **Fix the run-flare shelter window** as above.
7. **New levers worth trying**, cheapest first:
   - *Rope length*: max reach eases down by 10% over the curve, and LONG LINE
     buys it back. Upgrades then answer difficulty instead of only adding to it.
   - *Node drift*: hooks drift faster and some slowly orbit late in the curve.
     Aiming becomes the difficulty, not only spacing.
   - *Hazard pairings*: past about 60% of the curve, place a turret guarding a
     buff node, or a gravity well beside a spinner lane. Two known hazards
     combined read as new without new art.
   - *Crumbling nodes*: a rare node that cracks after 3 s tethered, drawn with the
     damaged-sprite language the bosses already use. Forces commitment.
   - *Wind corridors*: reuse `TITAN_WIND` as a zone feature — a visible
     solar-wind band that pushes sideways.
   - *Heart scarcity*: heart pickups thin with `t`, and VITALITY answers it.
   - *Fog*: the eclipse's dimming, briefly, outside boss approaches — nodes
     visible only near the player for 20 s.

Also fixed while reading: the `DIFFICULTY_FULL_DISTANCE` doc comment said
"≈ 2.88 M px". It is 1.44 M at the current 300 px/s.

---

## Part 3 — Everything else from the night, briefly

- **Background decor** (`src/scenes/game/decor.rs`, `assets/pixellab/decor`):
  23 new PixelLab pieces. There are 6 planets, a moon, a spiral galaxy and
  15 objects: satellites, stations and a derelict, a comet, a rocket. There are
  also 5 rare oddities: UFO, space whale, astronaut, space jellyfish and a rubber
  duck. At most 3 are on screen, one rare at a time, with 7,500–13,500 px of
  travel between arrivals. They use parallax, drift and bob, and are dimmed so
  they never compete with hazards. The run records rare sightings in
  `decor_rare_seen` for future achievements.
- **Rope styles** (`src/cosmetics.rs`, `Effect::TiledStrip` in wgpu_canvas).
  There are 12 purchasable styles beyond the classic energy rope. The tiles repeat
  along the rope, so every style looks right at any length. 7 are animated:
  helix, plasma chain, lightning, void thread, solar flame, stardust and spectral
  chain. Colour is chosen separately (10 colours). ORIGINAL wears each style's
  own signature colour.
- **Cat breeds**: 12 new animated cat balls (ball → half-open → spread-eagle).
  8 are breeds: ginger, midnight, Siamese, tuxedo, silver tabby, Maine Coon,
  Persian, sphynx. 4 are premium oddities: galaxy, robo-cat, astro-cat, magma.
  Prices are 150–800 META. `Price::Premium` is ready for a real-money currency.
- **Shop**: buy/select flow for both, the colour row (tap or W/S), live previews,
  and prices that grey out when unaffordable.
- **Flare Titan shelters**: every node within 1,800 px of a shelter (14 of 42
  nodes, farthest 1,619 px), a reticle on the nearest one each flare, and
  SUNPROOFING now works in that arena.
- **Headless tools**: `HEADLESS_SHOT_DIR` frame captures, `HEADLESS_SHOP` shop
  captures, `HEADLESS_IMMORTAL`, and the corrected flare audit.
