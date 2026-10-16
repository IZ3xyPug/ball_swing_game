#!/usr/bin/env python3
"""Generate the Colossus fight's sound effects.

Synthesised from scratch (sines, saws, filtered noise), 16-bit mono WAV, so
rodio plays them with its default features. Standard library only, like
`make_conductor_music.py`: an asset that only rebuilds on a machine with the
right packages is one nobody rebuilds.

    python3 tools/make_colossus_sfx.py

Writes assets/colossus_<name>.wav:
    windup   a fist or the torso winding up: grinding servos building
    slam     a fist landing: a deep boom and a rumbling tail
    clap     the hands' clap: a crack, a whoosh, a boom
    charge   the head charging its gaze: a rising, wavering whine
    beam     a gaze shot: a falling, buzzing laser blast
    meteor   the meteor storm: a whoosh coming in
    vent     the torso venting: a hiss of steam
    clang    the player's hit on a part: struck metal
    break    a part destroyed: a crunching metal explosion
"""
import math
import os
import random
import struct
import wave

SR = 44100
OUT = os.path.join(os.path.dirname(__file__), "..", "assets")
rnd = random.Random(7)


def secs(n):
    return int(n * SR)


def env_exp(i, tau):
    return math.exp(-i / (tau * SR))


def lowpass(x, cutoff):
    """One-pole low-pass; `cutoff` is a fixed Hz or a function of the index."""
    y, out = 0.0, []
    for i, v in enumerate(x):
        fc = cutoff(i) if callable(cutoff) else cutoff
        a = 1.0 - math.exp(-2.0 * math.pi * fc / SR)
        y += a * (v - y)
        out.append(y)
    return out


def highpass(x, cutoff):
    lp = lowpass(x, cutoff)
    return [v - l for v, l in zip(x, lp)]


def noise(n):
    return [rnd.uniform(-1.0, 1.0) for _ in range(n)]


def mix(*tracks):
    n = max(len(t) for t in tracks)
    return [sum(t[i] for t in tracks if i < len(t)) for i in range(n)]


def gain(x, g):
    return [v * g for v in x]


def osc(n, freq, shape="sine"):
    """`freq` is a fixed Hz or a function of the index (a sweep)."""
    ph, out = 0.0, []
    for i in range(n):
        f = freq(i) if callable(freq) else freq
        ph += f / SR
        p = ph % 1.0
        if shape == "sine":
            out.append(math.sin(2 * math.pi * p))
        elif shape == "saw":
            out.append(2.0 * p - 1.0)
        else:  # square
            out.append(1.0 if p < 0.5 else -1.0)
    return out


def shape(x, amp):
    return [v * amp(i) for i, v in enumerate(x)]


def soft_clip(x, drive=1.0):
    return [math.tanh(v * drive) for v in x]


def fade_edges(x, ms_in=2.0, ms_out=20.0):
    a, b = int(ms_in / 1000 * SR), int(ms_out / 1000 * SR)
    n = len(x)
    return [v * min(1.0, i / max(a, 1)) * min(1.0, (n - i) / max(b, 1)) for i, v in enumerate(x)]


def write(name, x, peak=0.89):
    x = fade_edges(x)
    m = max(abs(v) for v in x) or 1.0
    data = b"".join(struct.pack("<h", int(max(-1.0, min(1.0, v / m * peak)) * 32767)) for v in x)
    path = os.path.join(OUT, f"colossus_{name}.wav")
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(data)
    print(f"{path}  {len(x) / SR:.2f}s")


def boom(n, f0=70.0, f1=32.0, tau=0.35):
    sweep = lambda i: f1 + (f0 - f1) * env_exp(i, 0.08)
    return shape(osc(n, sweep), lambda i: env_exp(i, tau))


def rumble(n, tau=0.9, cutoff=120.0):
    return shape(lowpass(lowpass(noise(n), cutoff), cutoff), lambda i: env_exp(i, tau) * 3.0)


def metal(n, partials, taus, amps):
    out = [0.0] * n
    for f, tau, a in zip(partials, taus, amps):
        s = osc(n, f)
        for i in range(n):
            out[i] += s[i] * a * env_exp(i, tau)
    return out


# windup: low saw servos climbing in pitch, grinding noise opening up.
n = secs(0.95)
ramp = lambda i: (i / n) ** 1.4
servo = shape(osc(n, lambda i: 45 + 70 * (i / n) ** 1.2, "saw"), lambda i: 0.5 * ramp(i))
whine = shape(osc(n, lambda i: 280 + 420 * (i / n) + 18 * math.sin(i / SR * 2 * math.pi * 9)), lambda i: 0.12 * ramp(i))
grind = shape(lowpass(noise(n), lambda i: 200 + 1800 * (i / n)), lambda i: 0.7 * ramp(i))
write("windup", soft_clip(mix(lowpass(servo, 600), whine, grind), 1.4))

# slam: the boom, a dirty crack on the attack, the rumble after.
n = secs(1.6)
crack = shape(highpass(noise(secs(0.08)), 900), lambda i: env_exp(i, 0.025))
write("slam", soft_clip(mix(gain(boom(n), 1.0), gain(crack, 0.5), gain(rumble(n, 0.7), 0.6)), 1.6))

# clap: a sharp double crack, a rushing whoosh, a boom under it.
n = secs(1.3)
c1 = shape(highpass(noise(secs(0.06)), 1500), lambda i: env_exp(i, 0.012))
c2 = [0.0] * secs(0.03) + gain(c1, 0.7)
whoosh = shape(lowpass(highpass(noise(n), 300), lambda i: 3000 * env_exp(i, 0.4) + 200), lambda i: 0.8 * env_exp(i, 0.35))
write("clap", soft_clip(mix(gain(c1, 0.9), c2, whoosh, gain(boom(n, 85, 40, 0.3), 0.8)), 1.5))

# charge: a whine rising a few octaves, wavering faster as it climbs.
n = secs(1.15)
f = lambda i: 160 * (2 ** (2.8 * (i / n) ** 1.3))
vib = lambda i: f(i) * (1 + 0.02 * math.sin(2 * math.pi * (4 + 14 * i / n) * i / SR))
core = shape(osc(n, vib), lambda i: (i / n) ** 1.6)
harm = shape(osc(n, lambda i: vib(i) * 2.01, "saw"), lambda i: 0.15 * (i / n) ** 2)
hum = shape(osc(n, 55, "saw"), lambda i: 0.25 * (i / n))
write("charge", soft_clip(mix(core, lowpass(harm, 4000), lowpass(hum, 400)), 1.2))

# beam: a buzzing blast that falls in pitch, chopped by a fast tremolo.
n = secs(0.9)
zap = shape(osc(n, lambda i: 900 * env_exp(i, 0.35) + 160, "saw"),
            lambda i: env_exp(i, 0.45) * (0.75 + 0.25 * math.sin(2 * math.pi * 32 * i / SR)))
fizz = shape(highpass(noise(n), 2500), lambda i: 0.35 * env_exp(i, 0.2))
sub = shape(osc(n, 70), lambda i: 0.6 * env_exp(i, 0.3))
write("beam", soft_clip(mix(lowpass(zap, 5000), fizz, sub), 1.8))

# meteor: noise swept down, getting louder, a small thump at the end.
n = secs(1.4)
swoosh = shape(lowpass(highpass(noise(n), 150), lambda i: 4000 - 3400 * (i / n)),
               lambda i: (i / n) ** 1.5 * 1.2)
thump = [0.0] * secs(1.15) + boom(secs(0.25), 90, 45, 0.08)
write("meteor", soft_clip(mix(swoosh, thump), 1.3))

# vent: a burst of steam, bright hiss settling.
n = secs(1.3)
hiss = shape(highpass(noise(n), 1800), lambda i: min(1.0, i / secs(0.05)) * env_exp(i, 0.6))
body = shape(lowpass(highpass(noise(n), 400), 1500), lambda i: 0.4 * env_exp(i, 0.3))
write("vent", mix(hiss, body))

# clang: struck heavy metal, inharmonic partials ringing out.
n = secs(0.9)
ring = metal(n, [312, 841, 1395, 2210, 3170], [0.5, 0.35, 0.25, 0.15, 0.08], [1.0, 0.7, 0.5, 0.35, 0.2])
tick = shape(highpass(noise(secs(0.02)), 3000), lambda i: env_exp(i, 0.004))
write("clang", mix(ring, gain(tick, 0.6)))

# break: an explosion, tearing metal, debris clattering after.
n = secs(2.1)
blast = shape(lowpass(noise(n), lambda i: 2500 * env_exp(i, 0.25) + 120), lambda i: env_exp(i, 0.5) * 1.3)
tear = metal(n, [173, 466, 977, 1543], [0.6, 0.45, 0.3, 0.2], [0.5, 0.45, 0.35, 0.25])
debris = [0.0] * n
for _ in range(26):
    at = secs(0.15) + rnd.randrange(secs(1.6))
    clk = metal(secs(0.08), [rnd.uniform(1200, 4200)], [0.02], [rnd.uniform(0.2, 0.5)])
    for k, v in enumerate(clk):
        if at + k < n:
            debris[at + k] += v
write("break", soft_clip(mix(blast, gain(boom(n, 60, 28, 0.6), 1.1), tear, debris), 1.5))
