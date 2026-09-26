# Rendering optimisation: what the device actually says

This is a 2D game. It should not be struggling, and the numbers below say it
is not struggling at anything a 2D game does — it is struggling at two
specific things bolted on top.

Everything here is measured on the test device (Motorola Edge 2025,
2712x1220), from the `frame:` / `tick:` / `night:` lines in logcat.

## The measurement

`night: ON`/`OFF` markers were added so a dip could be ATTRIBUTED rather than
guessed at. Across one session, 497 one-second windows:

| | night ON | night OFF |
|---|---|---|
| fps | 48.8 | 61.4 |
| **present** | **11.8 ms** | 1.1 ms |
| prepare | 2.4 ms | 1.9 ms |
| tick total | ~4 ms | ~4 ms |
| **texture uploads** | **94.7/s** | 3.9/s |

Two things stand out, and they are independent problems.

### 1. `present` is the cost, which means the GPU is behind

`present` is where the frame waits for the swapchain. It is 1.1 ms normally
and 11.8 ms in night mode. The CPU-side `tick` does not move at all. This is
not a logic problem, a pass-count problem, or an object-count problem: the GPU
is being given more work than it can finish in a frame.

The work is the night-mode post shader. It is ALREADY a single pass — bloom,
vignette and chromatic aberration are one fragment shader, one pipeline, one
full-screen draw — so merging passes cannot help. The cost is inside it:

```
4 rings x 8 taps + 1 centre = 33 texture samples per pixel
2712 x 1220 x 33            = 109 million fetches per frame
x 60fps                     = 6.5 billion fetches/second
```

Now 2 rings on Android (17 samples), which roughly halves it. That is a dial,
not a fix.

**The actual fix is a downsampled bloom.** Bloom is a blur; a blur does not
need full resolution. Sampling a quarter-size copy is 16x less work per tap
and looks BETTER, because a wide blur from few full-res taps is what produces
the ringing this one has. It costs two small extra passes — which is more
passes and far less fill, and fill is what the measurement says we are short
of.

Estimated: 17 full-res taps -> ~9 quarter-res taps = roughly 30x less bloom
fill. That should return night mode to the 60fps the rest of the game runs at.

### 2. Texture upload churn, 24x higher in the dark

3.9/s normally; 94.7/s in night mode, peaking at 150. The atlas is keyed on
`Arc::as_ptr` and `trim()` drops anything the scene no longer references, so
an image rebuilt each frame uploads, is dropped, is evicted, and uploads
again. Roughly two per frame here.

This is also the likely cause of the reported "effects stop for a split
second": a sprite whose texture is being re-uploaded has nothing resident to
draw that frame.

Hunting the source by reading code failed once, so the atlas now RECORDS WHAT
CHURNS — the log line gains `churn: 256x256 x58  ...`, naming the sprite by
its dimensions. The next capture in night mode identifies it outright.

## Why this is not a "simple 2D game is too heavy" problem

The rest of the frame is healthy and was measured getting there:

* `objects` (the tick's biggest stage) went 8-12.8 ms -> 0.4-1.9 ms by not
  advancing animations for invisible objects — the cost was a heap allocation
  per animated sprite per frame, for ~675 objects parked off screen.
* Sprite frames became shared `Arc`s, so the atlas uploads each one once
  instead of every frame.
* Steady-state uploads are ~0/s and the tick is ~4 ms.

A 2D frame at 60fps with 1.1 ms of present is exactly right. The problem is a
post-processing effect written for a desktop GPU and a per-frame allocation
somewhere in the dark path — not the game.

## Order of work

1. **Downsampled bloom chain.** The largest measured win and the only one that
   improves quality at the same time.
2. **Name and kill the night-mode churn**, using the new `churn:` line.
3. **Lighting and shadows.** Untouched so far and unmeasured: the night path
   also adds a shadow-casting lamp plus a trail chain. Worth its own markers
   before assuming anything.
4. **A quality dial that is chosen, not hardcoded.** Ring count is
   `#[cfg(target_os = "android")]` today. A phone is not one performance
   class, and a dial the game sets from a measured frame time is better than a
   constant chosen by platform.

## Rules that came out of this

* **Instrument before hunting.** `night:` markers turned "it lags sometimes"
  into a two-column table in one session. The `churn:` line exists for the
  same reason.
* **Fewer passes is not faster.** The thing that looked like the optimisation
  (merge the post passes) was already done, and the thing that sounds like a
  regression (add two passes for a downsample chain) is the large win. Fill is
  the budget on this device, not pass count.
* **`present` is the tell.** High `present` with a flat `tick` means the GPU,
  every time. High `prepare` means the CPU building the frame. They are
  different problems and the log separates them.
