#!/usr/bin/env python3
"""Generate every boss's sound effects, the menu click and the transition.

Synthesised from scratch, 16-bit mono WAV, standard library only (like
`make_conductor_music.py`). The voice aims at Terraria's bosses: short,
punchy, a little crunchy (a light bit-crush), metal that rings like struck
plate, lasers that are square waves swept down.

    python3 tools/make_boss_sfx.py

Writes assets/<boss>_<name>.wav for colossus, weaver, titan, magnetar,
serpent and devourer, and assets/ui_click.wav, assets/ui_transition.wav.
"""
import math
import os
import random
import struct
import wave

SR = 44100
OUT = os.path.join(os.path.dirname(__file__), "..", "assets")
rnd = random.Random(11)
TAU = math.tau


def n_(s):
    return int(s * SR)


# ── building blocks ─────────────────────────────────────────────────────────

def osc(n, freq, shape="sine", phase=0.0):
    ph, out = phase, []
    for i in range(n):
        f = freq(i) if callable(freq) else freq
        ph += f / SR
        p = ph % 1.0
        if shape == "sine":
            out.append(math.sin(TAU * p))
        elif shape == "saw":
            out.append(2.0 * p - 1.0)
        elif shape == "square":
            out.append(1.0 if p < 0.5 else -1.0)
        else:  # triangle
            out.append(4.0 * abs(p - 0.5) - 1.0)
    return out


def noise(n):
    return [rnd.uniform(-1.0, 1.0) for _ in range(n)]


def lowpass(x, fc):
    y, out = 0.0, []
    for i, v in enumerate(x):
        f = fc(i) if callable(fc) else fc
        a = 1.0 - math.exp(-TAU * max(f, 10.0) / SR)
        y += a * (v - y)
        out.append(y)
    return out


def highpass(x, fc):
    return [v - l for v, l in zip(x, lowpass(x, fc))]


def bandpass(x, lo, hi):
    return lowpass(highpass(x, lo), hi)


def env(n, attack=0.002, decay=0.3, sustain=0.0, curve=1.0):
    a = max(1, n_(attack))
    out = []
    for i in range(n):
        if i < a:
            out.append(i / a)
        else:
            t = (i - a) / SR
            out.append(sustain + (1 - sustain) * math.exp(-t / decay) ** curve)
    return out


def mul(x, e):
    return [a * (e(i) if callable(e) else e[i] if isinstance(e, list) else e) for i, a in enumerate(x)]


def mix(*tracks):
    n = max(len(t) for t in tracks)
    return [sum(t[i] for t in tracks if i < len(t)) for i in range(n)]


def at(x, secs):
    return [0.0] * n_(secs) + x


def gain(x, g):
    return [v * g for v in x]


def drive(x, k=2.0):
    return [math.tanh(v * k) for v in x]


def crush(x, bits=10, hold=2):
    """A light lo-fi grit: fewer bits, a sample held."""
    q = 2 ** (bits - 1)
    out, last = [], 0.0
    for i, v in enumerate(x):
        if i % hold == 0:
            last = round(v * q) / q
        out.append(last)
    return out


def metal(n, partials, decay=0.4, bright=1.0):
    """Struck plate: inharmonic partials, the high ones dying first."""
    out = [0.0] * n
    for k, f in enumerate(partials):
        tau = decay / (1.0 + k * 0.6 / bright)
        amp = 1.0 / (1.0 + k * 0.45)
        ph = rnd.random()
        for i in range(n):
            out[i] += math.sin(TAU * (f * i / SR + ph)) * amp * math.exp(-i / (tau * SR))
    return out


def clank(n, base, decay=0.35):
    ratios = [1.0, 2.32, 3.17, 4.53, 5.91, 7.4, 8.93]
    return mix(metal(n, [base * r for r in ratios], decay), mul(highpass(noise(n_(0.03)), 2500), env(n_(0.03), 0.0005, 0.006)))


def pluck(n, freq, decay=0.996):
    """Karplus-Strong: a taut string."""
    p = max(2, int(SR / freq))
    buf = [rnd.uniform(-1, 1) for _ in range(p)]
    out = []
    for i in range(n):
        v = buf[i % p]
        nxt = buf[(i + 1) % p]
        buf[i % p] = decay * 0.5 * (v + nxt)
        out.append(v)
    return out


def echo(x, delay=0.09, fb=0.35, mixv=0.35, n_out=None):
    d = n_(delay)
    n = n_out or len(x) + d * 4
    buf = x + [0.0] * (n - len(x))
    out = buf[:]
    for i in range(d, n):
        out[i] += out[i - d] * fb
    return [a * (1 - mixv) + b * mixv for a, b in zip(buf, out)]


def boom(n, f0=90.0, f1=35.0, decay=0.3):
    return mul(osc(n, lambda i: f1 + (f0 - f1) * math.exp(-i / (0.06 * SR))), env(n, 0.001, decay))


def sweep_noise(n, f0, f1, q=0.5):
    return lowpass(highpass(noise(n), lambda i: (f0 + (f1 - f0) * i / n) * q), lambda i: f0 + (f1 - f0) * i / n)


def write(name, x, peak=0.9, grit=True):
    if grit:
        x = crush(x)
    a, b = n_(0.002), n_(0.02)
    n = len(x)
    x = [v * min(1.0, i / max(a, 1)) * min(1.0, (n - i) / max(b, 1)) for i, v in enumerate(x)]
    m = max(abs(v) for v in x) or 1.0
    data = b"".join(struct.pack("<h", int(max(-1.0, min(1.0, v / m * peak)) * 32767)) for v in x)
    path = os.path.join(OUT, f"{name}.wav")
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(data)
    print(f"{name:24s} {n / SR:.2f}s")


# ── the Colossus: metal ─────────────────────────────────────────────────────

def colossus():
    # Wind-up: a ratcheting servo winding up, a metal creak under it.
    n = n_(0.9)
    ratchet = [0.0] * n
    t, step = 0.0, 0.07
    while t < 0.85:
        c = clank(n_(0.06), 900 + 700 * t, 0.02)
        for i, v in enumerate(c):
            j = n_(t) + i
            if j < n:
                ratchet[j] += v * 0.5
        t += step
        step = max(0.028, step * 0.9)
    servo = mul(osc(n, lambda i: 70 + 120 * (i / n) ** 1.3, "square"), lambda i: 0.25 * (i / n))
    write("colossus_windup", drive(mix(ratchet, lowpass(servo, 900)), 1.6))

    # Slam: a huge plate struck, a thud, a ringing tail.
    n = n_(1.5)
    write("colossus_slam", drive(mix(gain(clank(n, 110, 0.9), 0.9), gain(boom(n, 80, 30, 0.35), 1.2),
                                     mul(lowpass(noise(n), 900), env(n, 0.001, 0.12))), 1.8))

    # Clap: two plates meeting — bright double clang, a shockwave.
    n = n_(1.2)
    c1 = clank(n, 340, 0.45)
    write("colossus_clap", drive(mix(c1, at(gain(clank(n, 360, 0.4), 0.8), 0.035),
                                     gain(boom(n, 120, 50, 0.25), 0.9),
                                     mul(sweep_noise(n, 4000, 300), env(n, 0.005, 0.3))), 1.7))

    # Charge: a laser charging (square, rising, a fast tremolo).
    n = n_(1.1)
    f = lambda i: 140 * 2 ** (3.2 * (i / n) ** 1.4)
    core = mul(osc(n, f, "square"), lambda i: (i / n) ** 1.5 * (0.7 + 0.3 * math.sin(TAU * (8 + 30 * i / n) * i / SR)))
    write("colossus_charge", mix(lowpass(core, 3500), gain(mul(osc(n, 55, "saw"), lambda i: i / n), 0.4)))

    # Beam: a square laser blast swept down, crackling.
    n = n_(0.75)
    zap = mul(osc(n, lambda i: 1500 * math.exp(-i / (0.12 * SR)) + 180, "square"), env(n, 0.001, 0.35))
    write("colossus_beam", drive(mix(zap, gain(mul(highpass(noise(n), 3000), env(n, 0.001, 0.15)), 0.5),
                                     gain(boom(n, 140, 60, 0.2), 0.7)), 1.6))

    # Meteor storm: a falling whistle and a metal impact.
    n = n_(1.3)
    whistle = mul(osc(n, lambda i: 1800 - 1300 * i / n, "triangle"), lambda i: 0.4 * (i / n))
    write("colossus_meteor", mix(whistle, gain(sweep_noise(n, 3000, 400), 0.6), at(clank(n_(0.4), 160, 0.25), 1.0)))

    # Vent: steam with a metal rattle.
    n = n_(1.2)
    hiss = mul(highpass(noise(n), 2000), env(n, 0.02, 0.5))
    rattle = mul(clank(n, 620, 0.08), lambda i: 0.4 * (1 + math.sin(TAU * 18 * i / SR)))
    write("colossus_vent", mix(hiss, rattle))

    # Hit: a short bright metal ping (a blow landing on armour).
    n = n_(0.4)
    write("colossus_clang", mix(clank(n, 760, 0.18), gain(boom(n, 220, 120, 0.05), 0.4)))

    # Break: metal tearing apart, an explosion, scrap clattering.
    n = n_(2.0)
    blast = mul(lowpass(noise(n), lambda i: 3000 * math.exp(-i / (0.3 * SR)) + 150), env(n, 0.001, 0.45))
    scrap = [0.0] * n
    for _ in range(18):
        s = n_(0.2) + rnd.randrange(n_(1.5))
        c = clank(n_(0.15), rnd.uniform(500, 2200), 0.06)
        for i, v in enumerate(c):
            if s + i < n:
                scrap[s + i] += v * rnd.uniform(0.2, 0.5)
    write("colossus_break", drive(mix(blast, gain(clank(n, 95, 0.8), 0.8), gain(boom(n, 70, 25, 0.5), 1.2), scrap), 1.7))


# ── the Gravity Weaver: gravity and thread ──────────────────────────────────

def weaver():
    n = n_(1.0)
    hum = mul(osc(n, lambda i: 90 + 40 * math.sin(TAU * 3 * i / SR) + 90 * i / n), lambda i: (i / n) ** 1.2)
    warb = mul(osc(n, lambda i: 360 + 120 * math.sin(TAU * 7 * i / SR) * i / n, "triangle"), lambda i: 0.35 * i / n)
    write("weaver_windup", mix(hum, warb))
    n = n_(0.9)
    write("weaver_attack", mix(gain(pluck(n, 110, 0.997), 0.9), gain(pluck(n, 165, 0.996), 0.6),
                               mul(sweep_noise(n, 2500, 400), env(n, 0.002, 0.2))))
    n = n_(0.5)
    write("weaver_hit", mix(metal(n, [1320, 1980, 2970, 4190], 0.25), gain(pluck(n, 660, 0.99), 0.4)))
    n = n_(1.6)
    shards = [0.0] * n
    for _ in range(24):
        s = rnd.randrange(n_(1.0))
        c = metal(n_(0.2), [rnd.uniform(2000, 6000)], 0.05)
        for i, v in enumerate(c):
            if s + i < n:
                shards[s + i] += v * 0.4
    write("weaver_break", mix(gain(boom(n, 140, 40, 0.4), 1.0), shards, mul(highpass(noise(n), 3000), env(n, 0.001, 0.3))))
    n = n_(2.0)
    write("weaver_intro", echo(mul(osc(n, lambda i: 40 + 30 * i / n, "saw"), lambda i: (i / n) ** 2), 0.2, 0.4, 0.3))


# ── the Flare Titan: fire ───────────────────────────────────────────────────

def titan():
    n = n_(1.1)
    roar = mul(bandpass(noise(n), 200, lambda i: 300 + 2500 * (i / n)), lambda i: (i / n) ** 1.3 * 1.5)
    write("titan_windup", drive(mix(roar, gain(mul(osc(n, 50, "saw"), lambda i: i / n), 0.4)), 1.5))
    n = n_(1.2)
    crackle = [0.0] * n
    for _ in range(60):
        s = rnd.randrange(n)
        for i in range(n_(0.004)):
            if s + i < n:
                crackle[s + i] += rnd.uniform(-1, 1) * math.exp(-i / 40) * math.exp(-s / (0.5 * SR))
    write("titan_attack", drive(mix(mul(lowpass(noise(n), lambda i: 4000 * math.exp(-i / (0.25 * SR)) + 300), env(n, 0.002, 0.4)),
                                    gain(boom(n, 100, 35, 0.35), 1.2), crackle), 1.8))
    n = n_(0.45)
    write("titan_hit", mix(clank(n, 520, 0.15), mul(highpass(noise(n), 3500), env(n, 0.001, 0.12))))
    n = n_(2.0)
    write("titan_break", drive(mix(mul(lowpass(noise(n), lambda i: 3500 * math.exp(-i / (0.4 * SR)) + 120), env(n, 0.001, 0.6)),
                                   gain(boom(n, 75, 22, 0.6), 1.3)), 1.8))
    n = n_(2.0)
    write("titan_intro", drive(mix(mul(bandpass(noise(n), 80, 900), lambda i: math.sin(math.pi * i / n)),
                                   gain(mul(osc(n, 45, "saw"), lambda i: math.sin(math.pi * i / n)), 0.6)), 1.4))


# ── the Magnetar: electricity ───────────────────────────────────────────────

def zaps(n, density=80, f_lo=1500, f_hi=6000):
    out = [0.0] * n
    for _ in range(density):
        s = rnd.randrange(n)
        f = rnd.uniform(f_lo, f_hi)
        for i in range(n_(0.012)):
            if s + i < n:
                out[s + i] += (1 if (i * f / SR) % 1 < 0.5 else -1) * math.exp(-i / 150) * 0.6
    return out


def magnetar():
    n = n_(1.0)
    buzz = mul(mix(osc(n, 60, "saw"), gain(osc(n, 120, "square"), 0.4)), lambda i: (i / n) ** 1.2 * 0.6)
    whine = mul(osc(n, lambda i: 400 + 1400 * (i / n) ** 2), lambda i: 0.4 * (i / n) ** 2)
    write("magnetar_windup", mix(lowpass(buzz, 1200), whine))
    n = n_(0.9)
    write("magnetar_attack", drive(mix(zaps(n, 120), gain(boom(n, 160, 60, 0.2), 0.8),
                                       mul(osc(n, lambda i: 900 * math.exp(-i / (0.1 * SR)) + 90, "square"), env(n, 0.001, 0.3))), 1.6))
    n = n_(0.4)
    write("magnetar_hit", mix(zaps(n, 20, 3000, 7000), metal(n, [1100, 2400, 3900], 0.12)))
    n = n_(1.8)
    write("magnetar_break", drive(mix(zaps(n, 260), gain(boom(n, 90, 25, 0.5), 1.2),
                                      mul(lowpass(noise(n), 2000), env(n, 0.001, 0.4))), 1.7))
    n = n_(1.8)
    write("magnetar_intro", mix(mul(osc(n, lambda i: 200 + 1600 * i / n, "saw"), lambda i: 0.3 * math.sin(math.pi * i / n)),
                                lowpass(mul(osc(n, 60, "saw"), lambda i: 0.6 * math.sin(math.pi * i / n)), 800)))


# ── the Serpent ─────────────────────────────────────────────────────────────

def serpent():
    n = n_(1.0)
    write("serpent_coil", mul(bandpass(noise(n), 2500, 7000), lambda i: math.sin(math.pi * i / n) * 0.9))
    n = n_(0.8)
    write("serpent_sweep", mix(mul(sweep_noise(n, 300, 4000), lambda i: math.sin(math.pi * i / n)),
                               gain(boom(n, 70, 40, 0.3), 0.5)))
    n = n_(1.1)
    tear = mul(osc(n, lambda i: 600 + 500 * math.sin(TAU * 11 * i / SR) - 400 * i / n, "saw"), env(n, 0.01, 0.4))
    write("serpent_rift", drive(mix(lowpass(tear, 2500), gain(boom(n, 110, 40, 0.35), 0.9)), 1.5))
    n = n_(1.2)
    write("serpent_launch", drive(mix(mul(lowpass(noise(n), lambda i: 600 + 3000 * i / n), lambda i: (i / n) * 1.2),
                                      gain(mul(osc(n, lambda i: 80 + 300 * i / n, "saw"), lambda i: i / n), 0.5)), 1.4))
    n = n_(0.7)
    write("serpent_lash", drive(mix(zaps(n, 70, 2000, 8000), mul(osc(n, lambda i: 2000 * math.exp(-i / (0.05 * SR)) + 200, "square"), env(n, 0.001, 0.2))), 1.5))
    n = n_(1.4)
    write("serpent_wormhole", echo(mul(osc(n, lambda i: 120 + 60 * math.sin(TAU * 4 * i / SR), "triangle"), lambda i: math.sin(math.pi * i / n)), 0.12, 0.45, 0.4))
    n = n_(0.4)
    write("serpent_hit", mix(gain(boom(n, 180, 90, 0.08), 1.0), clank(n, 420, 0.1)))
    n = n_(1.5)
    write("serpent_break", drive(mix(mul(lowpass(noise(n), lambda i: 2500 * math.exp(-i / (0.3 * SR)) + 120), env(n, 0.001, 0.4)),
                                     gain(boom(n, 85, 30, 0.45), 1.2)), 1.7))
    n = n_(1.8)
    write("serpent_intro", mix(mul(bandpass(noise(n), 1500, 6000), lambda i: math.sin(math.pi * i / n) * 0.8),
                               gain(mul(osc(n, lambda i: 55 + 25 * math.sin(TAU * 2 * i / SR), "saw"), lambda i: math.sin(math.pi * i / n)), 0.5)))


# ── the Sun Devourer ────────────────────────────────────────────────────────

def devourer():
    n = n_(0.35)
    write("devourer_shot", mix(mul(osc(n, lambda i: 1100 * math.exp(-i / (0.08 * SR)) + 220, "square"), env(n, 0.001, 0.12)),
                               gain(mul(highpass(noise(n), 3000), env(n, 0.001, 0.05)), 0.4)))
    n = n_(0.45)
    write("devourer_hit", mix(gain(boom(n, 140, 60, 0.12), 1.2), mul(lowpass(noise(n), 1500), env(n, 0.001, 0.08))))
    n = n_(1.8)
    write("devourer_break", drive(mix(mul(lowpass(noise(n), lambda i: 3000 * math.exp(-i / (0.35 * SR)) + 100), env(n, 0.001, 0.5)),
                                      gain(boom(n, 70, 25, 0.55), 1.3), zaps(n, 40)), 1.8))
    n = n_(2.2)
    write("devourer_intro", drive(mix(mul(osc(n, lambda i: 38 + 14 * math.sin(TAU * 1.5 * i / SR), "saw"), lambda i: math.sin(math.pi * i / n)),
                                      gain(mul(bandpass(noise(n), 60, 500), lambda i: math.sin(math.pi * i / n)), 0.8)), 1.6))
    n = n_(1.0)
    write("devourer_windup", mul(lowpass(osc(n, lambda i: 60 + 200 * (i / n) ** 2, "saw"), 1500), lambda i: (i / n) ** 1.3))


# ── the menu ────────────────────────────────────────────────────────────────

def ui():
    n = n_(0.09)
    tick = mix(mul(osc(n, 1800, "triangle"), env(n, 0.0005, 0.02)), at(mul(osc(n_(0.06), 2700, "triangle"), env(n_(0.06), 0.0005, 0.015)), 0.025))
    write("ui_click", tick, peak=0.6, grit=False)
    n = n_(0.9)
    whoosh = mul(lowpass(highpass(noise(n), 300), lambda i: 600 + 5000 * math.sin(math.pi * i / n)), lambda i: math.sin(math.pi * i / n) ** 1.5)
    shimmer = mul(mix(osc(n, lambda i: 880 + 880 * i / n), gain(osc(n, lambda i: 1320 + 1320 * i / n), 0.5)), lambda i: 0.15 * math.sin(math.pi * i / n))
    write("ui_transition", mix(whoosh, shimmer), peak=0.7, grit=False)


if __name__ == "__main__":
    colossus()
    weaver()
    titan()
    magnetar()
    serpent()
    devourer()
    ui()
