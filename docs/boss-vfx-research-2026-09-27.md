# Boss telegraphs, vulnerability and hit feel — research and what changed

Asked for on 2026-09-26: "deep research on other more recent 2D games and how
they do telegraphing and vulnerability effects and their general aesthetics
so we can further improve ours", plus "when we're attacking these bosses it
should feel like we actually hit them". This is what the research said, and
what each finding changed in the game.

## Findings

**1. Motion speed carries meaning before colour does.** Julian Love's rule
from Diablo III (quoted in An-Tim Nguyen's GDC 2022 talk *VFX as a Game
Design Language*): motion faster than a heartbeat reads as danger, slower as
safety. The same talk: colour is an accessibility problem when it is the only
channel, so status should also ride on shape and motion.

**2. The "now" of a weak point is a flash.** Metroid Dread marks every
counterable moment with a white flash (and a click) on the enemy; enemies
"glow for a brief period just before they strike". Nine Sols builds its whole
parry language on colour-coded flashes (red = unblockable, green = can be
countered).

**3. Telegraphs leave before the hit.** "The anticipation has to telegraph
the hit, and the fade has to clear before the next action"; the flash lands
on the exact frame of impact (Sunstrike VFX guide). A warning drawn under
the attack hides the attack.

**4. Timing is readable when it is spatial.** Closing/growing ring indicators
("the attack hits once both indicators are the same size") are the most
widely understood countdown; indicators are drawn slightly larger than the
hitbox so a near miss feels like a near miss.

**5. The boss's palette must not be the telegraph's.** Hades II's Chronos
fight is the cautionary case: a yellow stage, yellow boss and yellow
telegraphs; players report "so much red on the screen it's getting hard to see
what's what" elsewhere. (Our Colossus is amber/orange and its wind-ups are
red-orange: the same problem.)

**6. Emissive effects need a dark backing on busy scenes.** Diablo III put
black behind emissive effects so they pop over dense scenes instead of
washing out additively (JangaFX's breakdown).

**7. Bandwidth.** Too many things calling for attention at once jams the
signal; the thing the player must read first gets the highest contrast
(Nguyen; our own constants.rs already documents this as value hierarchy).

**8. A hit is felt, not just shown.** White hit flash (Hollow Knight flashes
enemies white), hit-stop of roughly 45-140 ms (a kill longer than a hit),
camera shake, and a physical response. Hit-stop does not have to freeze the
world: "the enemy/player shaking while things are still moving" works.

## What changed

| Finding | Change |
|---|---|
| 1 | Vulnerable and shielded states breathe at 0.7-0.8 Hz; telegraph dashes crawl at 2.2 Hz (were 0.35); the Weaver's live thread crackles, its reeling-in thread is calm. |
| 2 | Vulnerable state (new `OutlineState`): the whole silhouette flashes white the frame a window opens, with a ring bursting outward from the outline. `fx::attach_state_marker` tracks each state's age so every boss gets it for free. |
| 2 | A window about to close speeds its pulse into a flicker and blinks its brackets (Weaver windows report `closing`). |
| 3 | Colossus zones vanish as the hand launches; the head's beam path shows during wind-up and in each gap before the next shot, never under the sweep (beams 2-3 used to have no telegraph at all); Weaver lanes clear 10 ticks early; the Devourer's lunge — which had no telegraph — now shows a filling lane that clears just before the body moves. The landing gets its own burst on the impact frame. |
| 4 | `Effect::DangerZone`: a ring closing onto the strike disc as the wind-up runs, drawn 1.6x the strike. |
| 5 | Kept the documented decision that the Colossus's wind-up stays hot (its strike IS its own fire), and separated telegraphs from the art by motion, luminance and shape instead: fast dashes, hazard stripes, and white-hot cores on the closing ring and the lane's leading edge. |
| 6 | Dark band behind the danger zone's rim and ring, and outside the lane's rails. |
| 7 | Closed parts wear a LIGHTER shell than phase-shielded ones; wind-up rings sit on an overlay slot over the state instead of replacing it. |
| 8 | Boss parts are solid (`boss::common::bounce_off_part`); a landed hit (`land_hit`) flashes the part, shakes the camera, and holds the player at the contact point for 5 frames (83 ms) while the part shakes, then rebounds at >= 30 px/tick. |

## Found while doing it

- Colossus and Serpent took damage every frame the player overlapped an open
  part (only a KILL set invulnerability), so one pass through a hand took
  several HP. Solid parts make one touch one hit; a 20-tick per-hit guard
  backs it up.
- Serpent: a buffed hit on an open plate also counted as body contact, which
  threw the player and spent a buff absorption on every hit.
- Weaver: parts opened by a flip started their next wind-up a few frames in
  (a vulnerable flash that shut at once); spindles now hold through the
  window. A lone spindle mid-gauntlet no longer "opens" for one frame.
- The Weaver's retract ran backwards (the snap effect erases from the
  source end); it now reels in, shrinking toward the spindle.
- The Devourer's weakpoints said "hit me" while its dome was still up.
- The old Colossus vulnerability ring flickered based on position, not time.
- The outline field is sampled from an sRGB atlas, so the shader re-encodes
  it; the first shield lattice picked hex centres inside staggered
  rectangles, which draws triangles.

## Not done / next

- Hit-stop freezes the combatants, not the world. A true time-scale pause
  would need an engine-level simulation clock.
- Audio cues (Metroid Dread's click) — no per-state sounds yet.
- ~~A per-boss telegraph audit for the two unfinished bosses as they are
  built.~~ Done as they were built — see below.

## The last two bosses, built to the same rules (2026-09-27)

| Finding | Flare Titan | Magnetar |
|---|---|---|
| 1 | Prominence path dashes crawl at 1.4 Hz; the kindle's count-in beats three times on every part. | The charging beam flickers at 11 Hz and its dashes stream outward: danger by motion before colour. |
| 3 | The curved path (`Effect::ProminenceArc`, stage `Path`) and the landing ring clear 10 ticks before the throw; the plasma lands on a clean frame. | The charging line (`Effect::PulsarBeam`, `Charging`) clears 10 ticks before the beam goes live. |
| 4 | The landing's `DangerZone` closes onto a disc a little larger than the splash. | Chevrons down the charging beam's leading side point the way it will sweep. |
| 5 | Orange boss: vulnerable is a paler, whiter gold than the house colour; the path is the house red, the plasma the boss's orange. | Blue boss: the shield shell is violet instead of the house blue. |
| 6 | The path has a dark band behind its rails. | The charging line has a dark band under it. |
| 8 | Solid parts, felt hits, as every boss. | The same. |

Found while doing it: every texture the effect shader drew was upside
down (the quad's top-left vertex sampled the texture's bottom row). Round
art hid it; the outline field showed it — the Weaver's diagonal spindle
wore its shield along the other diagonal, and the Colossus torso's shield
missed its shoulders. And a glow cut off at its sprite's edge shows as a box
even at ~4% alpha, because blending happens in linear light: every new
effect fades to zero before its quad's edge.

## Sources

- An-Tim Nguyen, [*VFX as a Game Design Language*, GDC 2022 (slides)](https://media.gdcvault.com/GDC+2022/Speaker+Slides/VFXasagamedesignlanguage_Nguyen_An-Tim.pdf) — heartbeat rule, colour accessibility, bandwidth/hierarchy
- [GDC 2013: Julian Love, *The VFX of Diablo*](https://gdcvault.com/play/1017660/Technical-Artist-Bootcamp-The-VFX)
- [JangaFX: Exploring and Modernizing the VFX Methods of Diablo 3](https://jangafx.com/insights/diablo-3-vfx-experiments) — dark backing behind emissives
- [Wikitroid: Melee Counter](https://metroid.fandom.com/wiki/Melee_Counter), [Screen Rant: Metroid Dread melee counter](https://screenrant.com/metroid-dread-melee-counter-guide/) — the white counter flash
- [TheGamer: Nine Sols review](https://thegamer.com/nine-sols-review/), [Nine Sols boss guide](https://frostilyte.ca/2025/03/10/nine-sols-boss-guide/) — colour-coded attack classes
- [House of Nettles: Hades II Has a Legibility Problem](https://nex-3.com/blog/hades-ii-has-a-legibility-problem/), [Steam: Chronos attack telegraph](https://steamcommunity.com/app/1145350/discussions/0/4327475551419791707/) — palette clash
- [Sunstrike Studios: VFX for Games](https://sunstrikestudios.com/en/blog/vfx-for-games/) — flash on the impact frame, fade before the next action
- [VFX Apprentice: Area of Effect](https://www.vfxapprentice.com/blog/area-of-effect-aoe-in-games), [Mochi Lab: Boss Telegraph Pack](https://mochilab-studio.itch.io/boss-telegraph-pack-vol1) — ring/shrinking indicators
- [Game Developer: Enemy Attacks and Telegraphing](https://www.gamedeveloper.com/design/enemy-attacks-and-telegraphing)
- [Game Developer: Improving the Combat Impact of Action Games](https://www.gamedeveloper.com/audio/improving-the-combat-impact-of-action-games), [Dawnosaur: 7 Game Feel Tricks](https://dawnosaur.substack.com/p/7-game-feel-tricks-to-improve-your) — hit flash, hit-stop durations
