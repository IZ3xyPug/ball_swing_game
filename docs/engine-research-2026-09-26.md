# Quartz vs. industry 2D engines — lighting, effects, performance, API

Research for the 2026-09-25 request: "deep research on industry standard 2D
engines for ways we can better optimize ours ... as well as API that needs to
be made more intuitive / user friendly / more useful and versatile", and
"make sure our lighting and effects systems are competitive and have all the
bells and whistles". Compared against Godot 4.7 and Unity 6 URP 2D, with
tile-GPU guidance from Arm and the Vulkan project. Sources at the end.

Status legend: **HAVE** (shipped, tested), **NEW** (landed this session),
**PARTIAL**, **GAP**.

## 1. Feature parity

### Lights

| Feature | Godot 4 | Unity URP 2D | Quartz |
|---|---|---|---|
| Point light | PointLight2D | Spot (as point) | HAVE |
| Spot / cone | via texture | Spot (inner/outer angle) | HAVE (one cone angle, soft edge) |
| Directional (sun/moon) | DirectionalLight2D, height, max distance | Global | HAVE |
| Global / ambient | CanvasModulate | Global light | HAVE (ambient colour + strength) |
| Freeform (polygon) light | via texture | Freeform | GAP |
| Sprite / textured light ("cookie") | Texture + scale | Sprite light | GAP |
| Blend modes | Add / Subtract / Mix | blend styles (up to 4) | GAP (additive only) |
| Normal maps | CanvasTexture | yes, quality setting | HAVE (per-sprite; flat default) |
| Specular | CanvasTexture specular + shininess | no | GAP |
| Light height (for normals) | yes | normal "distance" | PARTIAL (fixed 0.5 in shader) |
| Layer / cull masks (which items a light hits) | Z range, layer range, item cull mask | target sorting layers | GAP |
| Light effects (pulse, flicker, colour cycle, fades) | via AnimationPlayer | via Timeline | HAVE, built in |
| Attach light to object | parent node | parent | HAVE |
| Many lights, cheap | per-item light lists | light textures at reduced scale | **NEW**: tiled culling (16x8 tiles, exact) |

### Shadows

| Feature | Godot 4 | Unity URP 2D | Quartz |
|---|---|---|---|
| Polygon occluders | LightOccluder2D | ShadowCaster2D | HAVE |
| Auto occluder from sprite | "Create LightOccluder2D" from Sprite2D (editor bake) | Shadow shape from sprite | **NEW**: `.outline_shadow()`, runtime, follows animation frames |
| Concave shapes | yes | yes | **NEW** (used the largest convex piece before) |
| Soft shadows | PCF5 / PCF13 + smoothing | softness + falloff | GAP (hard edge only) |
| Shadow colour / strength | colour with alpha | strength | GAP (fully black) |
| Occluder cull mode (one-sided) | CW / CCW / disabled | self-shadows toggle | PARTIAL (self-exclusion only) |
| Occluder light mask | yes | target sorting layers | GAP |
| Cost control | filter choice, max distance | light batching | **NEW**: reach culling + bounding-circle early-out; 1024-point pool |

### Glow, bloom, post

| Feature | Godot 4 | Unity URP 2D | Quartz |
|---|---|---|---|
| Bloom | WorldEnvironment glow, levels | Volume bloom | **NEW**: quarter-res bright-pass (was 33 full-res taps and never ran on Android) |
| Per-object glow | NOT supported without HDR 2D — an open proposal (#6725) asks for exactly this | via HDR emission + threshold | **NEW**: every `set_glow` is emissive and blooms; opt-out per object |
| Glow follows sprite shape | n/a (shader work) | n/a | **NEW**: halo traced from alpha, crisp rim + falloff |
| Vignette / chromatic aberration | shaders | Volume | HAVE (night composite) |
| Colour grading / LUT | Environment adjustments | Volume | GAP |
| Post quality switch | per effect | per Volume | **NEW**: `set_post_processing_enabled` |
| Custom post shaders | CanvasLayer + shader | Renderer Feature | HAVE (3 override slots) |

Worth noting: on per-object glow Quartz is now AHEAD of Godot, whose users
report that glow "applies globally rather than to individual sprites" and is
"blocked on HDR support" (godot-proposals #6725). Ours needs no HDR target —
emissive items are drawn a second time into the quarter-size bloom layer.

## 2. Performance architecture

**How the big engines keep many lights cheap.** Unity's 2D renderer draws
lights into *light render textures* — one per batch of sorting layers, at a
configurable *Render Scale* — and sprites sample that texture once. Cost is
per light-area at low resolution, not per light per sprite pixel; the
documented optimisations are all about fewer such textures (light batching,
one or two blend styles, lower render scale). Godot keeps light lists per
canvas item.

**What Quartz did, and does now.** Every lit pixel looped every light — 43 in
a boss darkness, 36-42 fps on the test phone. Now: tiled forward culling
(exact) plus removing the ~40 node lights whose only job was visibility.
The next step if light counts grow again is Unity's approach: render lights
into a light texture at 1/2 or 1/4 scale and have sprites sample it. That
makes lighting cost independent of how many lights overlap a sprite, and
pairs naturally with a light render scale quality setting.

**Tile-based GPU rules (Arm, Vulkan guide) and where we stand:**

| Rule | Status |
|---|---|
| Clear, don't load, at pass start | HAVE |
| `StoreOp::Discard` for depth and MSAA samples never read again | **NEW** (depth was stored: ~13 MB/frame, ~0.8 GB/s at 60 fps on the phone) |
| Resolve MSAA in the same pass | HAVE |
| Fewer full-screen passes; merge post effects | PARTIAL: night mode is one composite; the air barrier is a second pass |
| Don't MSAA full-screen post passes | **NEW** (post passes 1x) |
| Don't allocate targets you do not use | **NEW** (~40 MB saved on the phone) |
| Downsample before blurring; dual-filter blur for wide radius | PARTIAL: one quarter-res level. Next: a 3-level dual-filter chain for wider, softer bloom at similar cost |
| Avoid `discard` in opaque shaders (defeats hidden-surface removal) | GAP: image shaders `discard` for clipping; move bounds clipping to scissor rects |
| mediump / f16 in fragment shaders | GAP (needs wgpu `SHADER_F16`; worth it for the lit shader) |

**Measured on the phone this session:** lights were the entire dark-mode
cost — cap 0 or 8 ran at 62 fps against 36-42 with all 43. The post pass's
real cost on the phone is still unmeasured (it never ran there before); build
E's per-darkness A/B measures it.

## 3. API — making it intuitive, useful, versatile

Concrete proposals, highest leverage first.

1. **Typed effects instead of raw bitmasks.** — **DONE 2026-09-26** (`wgpu_canvas::Effect`, ids in `bitmask.w`; see the programme tracker §2f). Was:
   `attach_mega_fx(c, id, img, size, (r,g,b,a), [MEGA_BIT_ENERGY_TETHER, mode, p, 0], 1)`.
   Every effect packs its parameters into an untyped `[u32; 4]` plus a
   colour's alpha, and the meaning lives in a WGSL comment. Proposal:
   `obj.effect(Effect::EnergyTether { colour, intensity, snap: None })`, an
   enum whose variants own their parameters, with the packing done once in
   the engine. The typed API should also take colours as sRGB `Color` and
   convert: effect tints are LINEAR today, so an orange picked in any colour
   picker renders as pale peach unless the author decodes it first. And the
   flag word is now FULL — bit 31 went to the thrust plume — so new effects
   need an effect id rather than a flag. The game already hand-wrote a wrapper per effect
   (`attach_strike_lane`, `attach_impact`, ...) because the raw form is
   unusable; those wrappers belong in quartz.
2. **One post-processing stack.** Replace three positional override slots
   with a declared stack: `canvas.post().bloom(..).vignette(..).custom("id", params)`.
   The engine then chooses how many passes that takes (merging where it
   can), which is the tile-GPU win, and ordering stops being the caller's
   problem.
3. **Light builder + quality settings.**
   `LightSource::point("torch").at(x, y).radius(r).intensity(i).shadows(Soft::Pcf5)`
   in place of `new(id, pos, colour, radius, intensity)` then mutating
   fields. A `RenderQuality` struct (post on/off, bloom levels, light render
   scale, shadow filter) that a settings menu can bind to directly.
4. **Light and shadow masks.** `light.affects(layers)` and
   `obj.shadow_for(layers)` — Godot's cull masks. The boss arenas would
   have used them: "light the nodes but not the background".
5. **Render stats as an API, not just a log line.** `canvas.render_stats()`
   returning lights, occluders, post frames, uploads, pass count — the frame
   log has proved to be the single most useful tool this session, and a
   debug overlay should be able to read it.
6. **Fix the documentation examples.** quartz doctests fail in
   `plugin/mod.rs` (5), `background`, `terrain_collision`, `scroll.rs`:
   example code that does not compile teaches the wrong API.

## 4. Prioritised roadmap

| # | Item | Why | Size |
|---|---|---|---|
| 1 | Measure build E on the phone | post cost and shadow cost are unmeasured there | play one fight |
| 2 | Soft shadows (PCF-style, 5 taps along the penumbra) + shadow colour/strength | biggest visible lighting gap vs Godot/Unity | M |
| 3 | ~~Typed `Effect` enum in quartz~~ DONE 2026-09-26 | biggest API gap; removes a class of mis-packed-parameter bugs | M |
| 4 | Dual-filter bloom chain (3 levels) | wider, smoother glow at similar cost | S |
| 5 | Light / shadow cull masks | versatility; saves lights in arenas | M |
| 6 | Post stack API + pass merging | API clarity and one fewer full-screen pass | M |
| 7 | Light render texture at reduced scale (Unity model) | if light counts grow past what tiling absorbs | L |
| 8 | Blend modes (add/sub/mix), textured "cookie" lights, freeform lights | bells and whistles | M each |
| 9 | Scissor instead of `discard` for clipping; f16 in lit shader | tile-GPU efficiency | S / M |
| 10 | Fix quartz doctests | API trust | S |

## Sources

- [Godot: 2D lights and shadows](https://docs.godotengine.org/en/stable/tutorials/2d/2d_lights_and_shadows.html)
- [Godot proposal #6725: glow options on CanvasItemMaterial](https://github.com/godotengine/godot-proposals/issues/6725)
- [Godot: LightOccluder2D](https://docs.godotengine.org/en/stable/classes/class_lightoccluder2d.html)
- [Unity URP: 2D light properties](https://docs.unity3d.com/6000.0/Documentation/Manual/urp/2d-light-properties-explained.html)
- [Unity URP: optimize 2D lights / light batching](https://docs.unity3d.com/6000.3/Documentation/Manual/urp/2d-light-batching-debugger.html)
- [Arm: post-processing effects on mobile](https://developer.arm.com/community/arm-community-blogs/b/mobile-graphics-and-gaming-blog/posts/post-processing-effects-on-mobile-optimization-and-alternatives)
- [Vulkan guide: tile-based rendering best practices](https://docs.vulkan.org/guide/latest/tile_based_rendering_best_practices.html)
- [UWA: dual blur and its implementation](https://medium.com/@uwa4d/dual-blur-and-its-implementation-in-unity-c2cd77c90771)
