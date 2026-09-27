# Achievements — brainstorm

Asked for on 2026-09-26:

> "brainstorm lots of new achievements, again being creative, clever with
> naming of them, injecting humor, and rewarding creative use of game mechanics
> and peculiar ways to die as well as more realistically impressive milestone
> achievements, rather than some of the really easy to achieve distance and
> coin collecting achievements we currently have"

Eighty-one new ideas below, grouped, plus the four worth keeping. Each lists how it is earned, a tier (🥉 easy,
🥈 takes intent, 🥇 a real feat, 🤫 secret — hidden until earned), and what the
game already tracks versus what it would need. The last section covers how to
build it, and three problems in the current system found while reading it.

For scale: the game assumes about 300 px of net progress a second, so
**18,000 px is one minute** and the top of the difficulty curve is 1,440,000 px
(80 minutes).

---

## The current thirteen

| Now | Earned by | Verdict |
|---|---|---|
| Gold Master! | 10 coins, lifetime | **Cut.** Earned in the first minute of the first run. |
| Coin Hunter | 100 coins, lifetime | **Cut.** A few runs. |
| Marathoner | 10,000 px in a run | **Cut.** 33 seconds. |
| Quarter-Century | 25,000 px | **Cut.** 83 seconds. |
| Marathon | 50,000 px | **Cut.** Under 3 minutes. |
| Regular / Casual Regular / Rush Regular | 5 runs / 10 Casual / 5 Boss Rush | **Cut.** Rewards showing up, not playing. |
| Boss Slayer | "Defeat The Sun Devourer" — actually fires for ANY boss | **Keep, fix the text.** With a random roster the first boss is anyone. |
| Space Cadet | reach space | **Keep.** A real first. |
| Sunburnt / Running on Fumes / Gravity's Lap | die to the sun / oxygen / the floor | **Keep.** Now three of a whole death collection (section D). |

Players who already hold a cut achievement keep it. It simply stops being
offered to new profiles, or is moved to a hidden "legacy" row.

---

## A. Milestones that mean something

| Name | How | Tier | Needs |
|---|---|---|---|
| **Out of the Litter Box** | 100,000 px in one run (~5.5 min) | 🥉 | have (`best_distance`) |
| **Commuter Cat** | 360,000 px in one run (~20 min) | 🥈 | have |
| **Long Tail** | 720,000 px in one run (~40 min) | 🥇 | have |
| **Top of the Curve** | reach peak difficulty (1,440,000 px) in Normal | 🥇 | have (per-mode best) |
| **Pawsitively Endless** | 2,000,000 px in one Casual run — past the top of the curve | 🥇 | have |
| **Boss Slayer** | defeat any boss | 🥉 | have |
| **Nine Lives Well Spent** | finish Normal: defeat the final boss | 🥇 | boss-kill event with `is_final_boss` |
| **The Whole Menagerie** | defeat all seven bosses, lifetime | 🥇 | per-boss kill record |
| **Rush Hour** | clear Boss Rush | 🥈 | Boss Rush victory event |
| **Blitzkat** | clear Boss Rush in under N minutes (set N after a timed playtest) | 🥇 | Boss Rush total time (already scored) |
| **Hairball of Gold** | 300 coins in a single run | 🥈 | run coins at death |
| **Houston, We Have a Furball** | spend 3 minutes in space in one run | 🥈 | time-in-space counter |
| **Frequent Flyer** | play 100 runs | 🥈 | have (`runs_played`) |

## B. Swing craft — creative use of the mechanics

| Name | How | Tier | Needs |
|---|---|---|---|
| **Look Ma, No Paws** | cover 30,000 px without grabbing a node (cannons, pads and momentum only) | 🥇 | distance since last grab |
| **Terminal Velocicat** | hold the momentum cap for 5 seconds straight | 🥈 | speed ≥ cap timer |
| **Hang Time** | stay unhooked for 4 seconds, then grab (and live) | 🥈 | airborne timer |
| **Chain Reaction** | 25 grabs in a row, each within 1 s of the last release | 🥈 | grab/release timestamps |
| **Cutting It Fine** | re-hook within 150 px of the fall line | 🥈 | player y vs fall line at grab |
| **Ceiling Cat** | touch the top of the playable band | 🤫 | player y vs `HOOK_Y_MIN` |
| **Cat-apult** | grab a node within 1 s of a gravity cannon launch | 🥈 | cannon `FiringDown` → grab |
| **Cannonball Run** | three gravity-cannon launches within 20 s | 🥇 | cannon launch timestamps |
| **Gravity Assist** | leave a gravity well 40% faster than you entered it — a slingshot | 🥇 | speed at well enter/exit |
| **Well, Actually** | stay tethered to a node inside a well's pull for 5 s | 🥈 | tethered + in-well timer |
| **Needle Threader** | pass between two spinners less than 400 px apart, unhit | 🥈 | spinner pair proximity |
| **Bullet Time** | a turret shot passes within 60 px and misses | 🥈 | shot–player closest approach |
| **All You Can Buff** | grab three buff nodes inside one buff (re-buff before it runs out) | 🥈 | buff grab while `buff_timer > 0` |
| **Shelter Skelter** | reach a shield node in the last half-second of a flare's telegraph | 🥇 | `flare_warn` at shelter grab |
| **SPF Infinity** | take an entire unsheltered flare for zero damage (SUNPROOFING wards) | 🥉 | flare ended, wards used, no heart lost |
| **Starfall** | survive a stretch with three comets on screen at once | 🥈 | live comet count |
| **Retail Therapy** | buy five in-run upgrades (purple rings) in one run | 🥈 | upgrade purchases per run |
| **Upside Downer** | pass through a space rift and back without losing a heart | 🥈 | rift flip events |

## C. Boss feats — one proud moment per fight

| Name | Boss | How | Tier |
|---|---|---|---|
| **Lights Out** | Sun Devourer | defeat it without being hit by a bolt | 🥈 |
| **Self-Destruct Button** | Sun Devourer | bait it into its own generators until it breaks every one itself | 🥇 |
| **The Bigger They Are** | Colossus | defeat it without being hit by a lunge or a clap | 🥈 |
| **Talk to the Hand** | Colossus | destroy both hands before the head ever opens | 🥈 |
| **Tail Spin** | Serpent | kill the tail before any body segment | 🥈 |
| **Snake Eyes** | Serpent | finish it with the head as the very last piece standing, flawless | 🥇 |
| **Perfect Pitch** | Conductor | seven kicks in a row, no miss — arm it on one clean phrase | 🥈 |
| **Drop the Beat** | Conductor | land the killing hit on a downbeat | 🤫 |
| **Up Is the New Down** | Gravity Weaver | land the killing hit while gravity is inverted | 🥈 |
| **Loose Threads** | Gravity Weaver | kill both spindles of a pair inside one inversion | 🥇 |
| **Cool Customer** | Flare Titan | never take a burn for the whole fight | 🥇 |
| **Sunblock Is for Cowards** | Flare Titan | win without sheltering during a single flare | 🤫 |
| **Pole Position** | Magnetar | kill both poles inside the same starquake | 🥇 |
| **Field Trip** | Magnetar | get thrown by a push pulse and grab a node mid-flight | 🥈 |
| **Untouchable** | any | defeat a boss without losing a heart | 🥈 |
| **Clean Sweep** | any | Untouchable on three different bosses | 🥇 |
| **Speed Date** | any | defeat a boss within 60 s of arena entry | 🥇 |

## D. Peculiar ways to die

Today the game knows three causes (`sun`, `oxygen`, `fall`). Recording **what
took the last heart** unlocks the whole collection.

| Name | How | Tier |
|---|---|---|
| **Sunburnt** *(kept)* | die to the sun | 🥉 |
| **Running on Fumes** *(kept)* | die to low oxygen | 🥉 |
| **Gravity's Lap** *(kept)* | die to the danger floor | 🥉 |
| **Icarus** | die to the sun within 5 s of arriving in space | 🤫 |
| **Held Your Breath Wrong** | run out of oxygen with three or more hearts left | 🤫 |
| **Floor Is Lava** | die within 20 s of starting a run | 🤫 |
| **Participation Trophy** | die before 2,000 px three runs in a row | 🤫 |
| **So Close** | die within 3,000 px of a boss arena, while the eclipse darkens | 🥈 |
| **Premature Celebration** | lose a heart within 3 s of defeating a boss | 🤫 |
| **Spin Cycle** | a spinner takes your last heart | 🥉 |
| **Event Horizon** | lose your last heart inside a gravity well's pull | 🥉 |
| **Target Practice** | take three turret shots within 10 s (dead or alive) | 🥈 |
| **Forgot the Sunscreen** | die to a solar flare with a shield node within 600 px | 🤫 |
| **Rope-a-Dope** | die while tethered — hanging on did not save you | 🥉 |
| **Falling Upward** | fall off the TOP of the Weaver's arena while gravity is flipped | 🤫 |
| **Opposites Attract** | the Magnetar's pull drags you into its core | 🤫 |
| **Cat-astrophe** | lose three hearts within 5 seconds | 🥈 |
| **Quack Attack** | die while the rubber duck floats by | 🤫 |
| **Ninety-Nine Lives** | die 99 times, lifetime | 🥈 |

## E. Collection and economy

| Name | How | Tier | Needs |
|---|---|---|---|
| **Fat Cat** | hold 3,000 META at once | 🥈 | have (`meta_currency`) |
| **Big Spender** | spend 10,000 META, lifetime | 🥈 | META spent counter |
| **Fully Loaded** | max every permanent upgrade (14,669 META) | 🥇 | have (`perm_levels`) |
| **Fashion Victim** | own every rope style | 🥇 | have (`owned_cosmetics`) |
| **The Whole Litter** | own every cat | 🥇 | have |
| **Rainbow Connection** | equip every rope colour at least once | 🥉 | colours-equipped set |
| **Window Shopper** | visit every card in every shop tab without buying anything | 🤫 | shop browse set |

## F. Sightseeing — the new background oddities

The decor system already publishes `decor_rare_seen` when a rare piece first
enters the screen.

| Name | How | Tier |
|---|---|---|
| **Quack in Space** | spot the rubber duck | 🤫 |
| **Whale Hello There** | spot the space whale | 🤫 |
| **We Come in Peace** | spot the UFO | 🤫 |
| **Spacewalk** | spot the drifting astronaut | 🤫 |
| **Jelly Belly** | spot the space jellyfish | 🤫 |
| **Tourist Trap** | spot all five, lifetime | 🥈 |

## G. Just for fun

| Name | How | Tier |
|---|---|---|
| **Cat Nap** | stay tethered to one node for 60 seconds | 🤫 |
| **Zoomies** | three hours played, lifetime (`time_played_seconds`) | 🥈 |
| **Snooze Button** | pause ten times in one run | 🤫 |
| **Indecisive** | change rope colour 20 times in one shop visit | 🤫 |
| **Night Owl** | finish a run between midnight and 4 a.m. local time | 🤫 |

---

## Building it

1. **One table, not one function each.** Today every achievement is a const, a
   match arm and a hand-written check. A single table makes the list
   data (`id`, name, description, tier, secret, trigger). The trigger is either
   a lifetime-stat predicate (checked when stats change) or the name of a game
   event. Eighty of these as code would be eighty chances to typo a threshold.
2. **Emit events at the moments that matter** and let the table listen:
   - boss killed: kind, hearts lost, seconds in the arena, the order parts
     died, the damage source;
   - heart lost: cause;
   - death: cause, distance, seconds into the run;
   - grab and release timestamps;
   - flare start and end: sheltered?, wards used;
   - cannon launch; gravity-well enter and exit speed;
   - near misses (spinner pair, turret shot);
   - rare decor seen, shop purchase, pause.

   Most are one line at a site that already exists.
3. **Record the death cause** as what took the last heart: `spinner`, `well`,
   `turret`, `comet`, `asteroid`, `flare`, `boss:<kind>:<attack>`. Section D is
   free once this exists.
4. **Profile additions**: per-boss kills, per-cause deaths, rare decor seen,
   META spent, best coins in a run, and a small "flags" set for one-shot feats.
   All of these are plain lines in the existing save format.

### Three problems in the current system

- **Only the last of several unlocks is shown.** `check_achievements` can award
  several at once, but every `trigger_achievement` overwrites the same toast
  text, so only the last one shows. All are saved. A small queue fixes it, and
  matters more with eighty.
- **Any unlock hides Gold Master for the rest of that run.**
  `trigger_achievement` sets `GOLD_MASTER_UNLOCKED_VAR` as a shared "toast
  showing" gate. So if anything else unlocks first, the 10th coin cannot fire
  Gold Master until the menu reloads the flag from the profile. It goes away
  with the cut above, or by giving the toast its own flag.
- **Boss Slayer's text names the Sun Devourer.** It fires for any boss, and
  with the dealt roster the first boss can be any of six.
