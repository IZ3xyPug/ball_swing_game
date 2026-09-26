#!/usr/bin/env python3
"""Generate the Conductor's loop, its drum map, and the wave-warning sounds.

The Conductor is scored on releasing your tether on a KICK DRUM hit — not on
every beat. That is the whole design: a metronome where every click is a target
is a button-mashing test, and missing one of two targets a second reads as the
fight being unfair rather than as a rhythm to learn. Rock Band's beginner kick
mode is the reference: the pulse is always there, but only some of it is yours
to hit, and which ones changes bar to bar.

So the song and the game have to agree about exactly which sixteenth notes are
kicks. They do, because this script is the single source of that table: it
renders the audio AND writes `src/scenes/game/boss/beatmap.rs`. Editing a
pattern here changes both, and there is no second copy to forget.

    beat   = CONDUCTOR_BEAT_TICKS    / 60fps = 36/60 = 0.600s -> 100 BPM
    phase2 = CONDUCTOR_BEAT_TICKS_P2 / 60fps = 30/60 = 0.500s -> 120 BPM

Output is 16-bit mono WAV, which rodio decodes with its default features, so
there is no encoder dependency and nothing to install. Standard library only,
deliberately: an asset that can only be rebuilt on a machine with the right
packages is one nobody rebuilds.

    python3 tools/make_conductor_music.py
"""
import math
import os
import struct
import wave

SR = 44100
BARS = 8
BEATS_PER_BAR = 4
SLOTS_PER_BEAT = 4                       # sixteenth notes
SLOTS_PER_BAR = BEATS_PER_BAR * SLOTS_PER_BEAT

# ── The kick map. One string per bar of the 8-bar loop, one character per
#    sixteenth. 'x' is a kick, and a kick is a scoring target.
#
#    Read down the first column: every bar starts on the downbeat, so the player
#    always has an anchor. Everything after that varies — pairs (bar 2's 1-e),
#    syncopation off the grid (bars 3 and 7), a four-on-the-floor bar to lean
#    into (5), and a late cluster (6). Two bars are deliberately SPARSE (0 and
#    4) so the loop breathes and the player gets a rest rather than a constant
#    demand.
#
#    Slot 14 is left free in every bar: that is where the wave warning sits, and
#    it must never collide with a target. See WAVE_CUE_SLOT.
KICKS = [
    "x.......x.......",   # 0  sparse   — 1, 3
    "x.......x...x...",   # 1  +4       — 1, 3, 4
    "x.x.....x.......",   # 2  pair     — 1, 1e, 3
    "x.....x...x.....",   # 3  syncope  — 1, 2a, 3a
    "x.......x.......",   # 4  sparse   — rest bar
    "x...x...x...x...",   # 5  driving  — four on the floor
    "x.......x.x.x...",   # 6  cluster  — 1, 3, 3a, 4
    "x.....x.x...x...",   # 7  syncope  — 1, 2a, 3, 4
]

# The sixteenth the wave warning lands on: the "and of 4", the last off-beat
# before a bar turns over. Off the quarter-note grid the kicks sit on, so the
# ear separates "this one is yours to hit" from "this one is coming at you",
# which is the entire reason the two sounds are placed differently.
WAVE_CUE_SLOT = 14

assert all(len(p) == SLOTS_PER_BAR for p in KICKS), "every pattern is one bar"
assert all(p[WAVE_CUE_SLOT] != "x" for p in KICKS), \
    "a kick on the wave-cue slot would make the warning sound like a target"
assert all(p[0] == "x" for p in KICKS), "every bar needs its downbeat anchor"


# ── Deterministic noise. `random` would give a different drum kit on every
#    machine, which is the sort of thing that makes an asset diff unreadable.
class Noise:
    def __init__(self, seed=0x5EED):
        self.s = seed

    def __call__(self):
        self.s = (1103515245 * self.s + 12345) & 0x7FFFFFFF
        return (self.s / 0x3FFFFFFF) - 1.0


def env(t, attack, decay):
    """AD envelope, 0..1."""
    if t < attack:
        return t / attack if attack > 0 else 1.0
    return math.exp(-((t - attack) / max(decay, 1e-6)) * 3.2)


def sine(freq, t):
    return math.sin(2.0 * math.pi * freq * t)


def saw(freq, t):
    return 2.0 * ((freq * t) % 1.0) - 1.0


class Track:
    def __init__(self, seconds):
        self.n = int(SR * seconds)
        self.buf = [0.0] * self.n

    def add(self, start, dur, fn, gain):
        i0 = int(start * SR)
        i1 = min(self.n, int((start + dur) * SR))
        for i in range(max(0, i0), i1):
            self.buf[i] += fn((i - i0) / SR) * gain

    def write(self, path, loop_fade=0.012, peak_target=0.80):
        peak = max(1e-6, max(abs(v) for v in self.buf))
        scale = peak_target / peak
        fade = int(SR * loop_fade)
        out = [v * scale for v in self.buf]
        if fade > 0 and self.n > fade * 2:
            for i in range(fade):
                a = i / fade
                out[i] = out[i] * a + self.buf[self.n - fade + i] * scale * (1.0 - a)
        with wave.open(path, "w") as w:
            w.setnchannels(1)
            w.setsampwidth(2)
            w.setframerate(SR)
            w.writeframes(b"".join(
                struct.pack("<h", int(max(-1.0, min(1.0, v)) * 32767)) for v in out))


# ── The kit ───────────────────────────────────────────────────────────────────
def kick(t):
    """The target sound. A real kick: fast pitch drop into a body, plus a click
    so it cuts through on a phone speaker where the 50Hz body is inaudible."""
    body = sine(48.0 * (1.0 + 4.2 * math.exp(-t * 34.0)), t) * env(t, 0.002, 0.15)
    click = sine(1400.0, t) * env(t, 0.0005, 0.012) * 0.35
    return body + click


def snare(nz):
    def f(t):
        tone = (sine(225.0, t) * 0.5 + sine(338.0, t) * 0.3) * env(t, 0.001, 0.055)
        return tone * 0.6 + nz() * env(t, 0.001, 0.085) * 0.8
    return f


def hat(nz):
    def f(t):
        return nz() * env(t, 0.0005, 0.022)
    return f


def render_song(bpm, path):
    beat = 60.0 / bpm
    slot = beat / SLOTS_PER_BEAT
    total = beat * BEATS_PER_BAR * BARS
    tr = Track(total)
    nz = Noise()

    # A minor progression with a lift in the second half, so an 8-bar loop has
    # somewhere to go instead of repeating one chord eight times.
    roots = [55.00, 55.00, 61.74, 65.41, 55.00, 55.00, 73.42, 65.41]
    fifth = 1.4983

    for bar in range(BARS):
        bar_t = bar * BEATS_PER_BAR * beat
        root = roots[bar % len(roots)]
        pattern = KICKS[bar % len(KICKS)]

        # ── Kicks: the targets. Loudest thing in the mix, because the player is
        #    reading them, and every one is EXACTLY on its sixteenth.
        for s, ch in enumerate(pattern):
            if ch == "x":
                tr.add(bar_t + s * slot, 0.30, kick, 0.62)

        # ── Backbeat on 2 and 4. Never a target — it is the thing that makes
        #    the bar feel like music, and its job is to be clearly NOT a kick.
        for b in (1, 3):
            tr.add(bar_t + b * beat, 0.16, snare(nz), 0.20)

        # ── Hats on the eighths, for forward motion between targets.
        for e in range(BEATS_PER_BAR * 2):
            tr.add(bar_t + e * beat * 0.5, 0.05, hat(nz),
                   0.055 if e % 2 else 0.035)

        # ── Bass, following the chord.
        for b in range(BEATS_PER_BAR):
            tr.add(bar_t + b * beat, beat * 0.92,
                   lambda t, f=root: (saw(f, t) * 0.6 + sine(f, t) * 0.4)
                                     * env(t, 0.006, beat * 0.55),
                   0.28)

        # ── Pad: the fifth held across the bar, quiet, for atmosphere.
        tr.add(bar_t, beat * BEATS_PER_BAR,
               lambda t, f=root * fifth * 2.0, d=beat * BEATS_PER_BAR:
                   (sine(f, t) * 0.5 + sine(f * 1.005, t) * 0.5)
                   * (0.5 - 0.5 * math.cos(2.0 * math.pi * t / d)),
               0.09)

    tr.write(path)
    kicks = sum(p.count("x") for p in KICKS)
    print(f"{path}: {bpm:.0f} BPM, {BARS} bars, {total:.2f}s, "
          f"{kicks} targets ({kicks / total:.2f}/s)")


# ── The wave warning ─────────────────────────────────────────────────────────
#
# Two sounds, both pitched and shaped to be unlike the kick. The kick is a
# short low thump; these are bright and MOVING, because the message is not
# "hit this" but "something is coming, stop what you are doing".
def render_wave_cue(path):
    """The volley announcement: a rising whistle over a shimmer. Played once,
    quantised to the off-beat, when a wave volley is committed."""
    dur = 0.55
    tr = Track(dur)
    nz = Noise(0xC0FFEE)
    tr.add(0.0, dur,
           lambda t: sine(520.0 + 900.0 * (t / dur) ** 1.7, t) * env(t, 0.012, 0.30),
           0.55)
    tr.add(0.0, dur,
           lambda t: sine(781.0 + 1350.0 * (t / dur) ** 1.7, t) * env(t, 0.012, 0.26),
           0.22)
    tr.add(0.0, dur, lambda t: nz() * env(t, 0.05, 0.22) * 0.5, 0.14)
    tr.write(path, loop_fade=0.0, peak_target=0.85)
    print(f"{path}: wave volley announcement, {dur:.2f}s")


def render_wave_hit(path):
    """One wave passing: a falling filtered sweep. Played per wave, on the
    off-beat before that wave reaches the nodes."""
    dur = 0.40
    tr = Track(dur)
    nz = Noise(0xBEEF)
    tr.add(0.0, dur,
           lambda t: sine(1250.0 * math.exp(-t * 4.2), t) * env(t, 0.006, 0.20),
           0.50)
    tr.add(0.0, dur, lambda t: nz() * env(t, 0.004, 0.13) * 0.7, 0.20)
    tr.write(path, loop_fade=0.0, peak_target=0.85)
    print(f"{path}: single wave pass, {dur:.2f}s")


# ── The table the game reads ─────────────────────────────────────────────────
def write_beatmap(path):
    masks = []
    for p in KICKS:
        m = 0
        for s, ch in enumerate(p):
            if ch == "x":
                m |= 1 << s
        masks.append(m)

    rows = "\n".join(
        f"    0b{m:016b}, // bar {i}: {KICKS[i]}" for i, m in enumerate(masks))

    with open(path, "w") as f:
        f.write(f'''//! Which sixteenth notes are kick drums — the Conductor's scoring targets.
//!
//! GENERATED by `tools/make_conductor_music.py`. Do not edit: the same script
//! renders the audio from this table, so a hand edit here would silently score
//! the player against a rhythm the song does not play. Change the patterns in
//! the script and re-run it.
//!
//! Only KICKS are targets. The snare on 2 and 4, the hats on the eighths and
//! the bass are there to make a bar feel like music, and deliberately do not
//! score — a grid where every click is a target is a mashing test, not a
//! rhythm.

/// Bars in the loop. The pattern repeats every this many bars.
pub const CONDUCTOR_BEATMAP_BARS: u32 = {BARS};
/// Sixteenth notes per bar.
pub const CONDUCTOR_SLOTS_PER_BAR: u32 = {SLOTS_PER_BAR};
/// Sixteenths per quarter-note beat.
pub const CONDUCTOR_SLOTS_PER_BEAT: u32 = {SLOTS_PER_BEAT};

/// The sixteenth the wave warning sounds on — the "and of 4".
///
/// Never a kick in any bar (the generator asserts it), so the ear can tell
/// "yours to hit" from "coming at you" by WHERE it lands as well as by timbre.
pub const CONDUCTOR_WAVE_CUE_SLOT: u32 = {WAVE_CUE_SLOT};

/// Bit `s` set means sixteenth `s` of that bar is a kick.
pub const CONDUCTOR_KICKS: [u16; {BARS}] = [
{rows}
];

/// Total targets in one loop — what the resonance requirement is tuned against.
pub const CONDUCTOR_KICKS_PER_LOOP: u32 = {sum(p.count("x") for p in KICKS)};

/// Is sixteenth `slot` of bar `bar` (both within the loop) a scoring target?
#[inline]
pub fn is_kick(bar: u32, slot: u32) -> bool {{
    if slot >= CONDUCTOR_SLOTS_PER_BAR {{
        return false;
    }}
    let mask = CONDUCTOR_KICKS[(bar % CONDUCTOR_BEATMAP_BARS) as usize];
    mask & (1u16 << slot) != 0
}}
''')
    print(f"{path}: {BARS} bars, {sum(p.count('x') for p in KICKS)} targets/loop")


if __name__ == "__main__":
    here = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    assets = os.path.join(here, "assets")
    render_song(100.0, os.path.join(assets, "conductor_p1.wav"))
    render_song(120.0, os.path.join(assets, "conductor_p2.wav"))
    render_wave_cue(os.path.join(assets, "conductor_wave_cue.wav"))
    render_wave_hit(os.path.join(assets, "conductor_wave_hit.wav"))
    write_beatmap(os.path.join(here, "src", "scenes", "game", "boss", "beatmap.rs"))
