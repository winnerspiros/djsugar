#!/usr/bin/env python3
"""Generates the built-in djsugar sample pack (synthesised, no external asset).

Output: res/samples/djsugar/*.wav, 44.1 kHz, 16-bit mono, peak -1 dBFS
Deterministic: same input gives same bytes (fixed RNG seed).
"""

import math
import os
import random
import struct
import wave

SR = 44100
OUT = os.path.join(
    os.path.dirname(os.path.abspath(__file__)),
    "..",
    "res",
    "samples",
    "djsugar",
)
rng = random.Random(1234)


def env_exp(t, decay):
    return math.exp(-t * decay)


def norm(buf, peak=0.891):  # -1 dBFS
    m = max(1e-9, max(abs(x) for x in buf))
    g = peak / m
    return [x * g for x in buf]


def fade(buf, ms_in=2, ms_out=8):
    n_in = int(SR * ms_in / 1000)
    n_out = int(SR * ms_out / 1000)
    for i in range(min(n_in, len(buf))):
        buf[i] *= i / n_in
    for i in range(min(n_out, len(buf))):
        buf[-1 - i] *= i / n_out
    return buf


def dur(sec):
    return int(SR * sec)


def kick():
    n = dur(0.45)
    b = []
    for i in range(n):
        t = i / SR
        f = 45 + 110 * env_exp(t, 28)
        ph = 2 * math.pi * f * t
        b.append(math.sin(ph) * env_exp(t, 6))
    return fade(norm(b))


def snare():
    n = dur(0.30)
    b = []
    for i in range(n):
        t = i / SR
        tone = math.sin(2 * math.pi * 185 * t) * env_exp(t, 22) * 0.5
        noise = (rng.random() * 2 - 1) * env_exp(t, 16) * 0.8
        b.append(tone + noise)
    return fade(norm(b))


def clap():
    n = dur(0.35)
    b = []
    bursts = [0.0, 0.011, 0.023, 0.037]
    for i in range(n):
        t = i / SR
        e = 0.0
        for s in bursts:
            if t >= s:
                e += env_exp(t - s, 45) * 0.5
        e += env_exp(t - 0.037, 12) * 0.6 if t >= 0.037 else 0
        b.append((rng.random() * 2 - 1) * e)
    return fade(norm(b))


def hat(open_=False):
    n = dur(0.6 if open_ else 0.08)
    b = []
    decay = 9 if open_ else 70
    prev = 0.0
    for i in range(n):
        t = i / SR
        x = rng.random() * 2 - 1
        hp = x - prev * 0.9  # crude high-pass
        prev = x
        b.append(hp * env_exp(t, decay))
    return fade(norm(b))


def rim():
    n = dur(0.06)
    b = []
    for i in range(n):
        t = i / SR
        b.append(
            math.sin(2 * math.pi * 1650 * t) * env_exp(t, 110) * 0.8
            + math.sin(2 * math.pi * 2500 * t) * env_exp(t, 140) * 0.3
        )
    return fade(norm(b))


def tom():
    n = dur(0.5)
    b = []
    for i in range(n):
        t = i / SR
        f = 110 + 90 * env_exp(t, 12)
        b.append(math.sin(2 * math.pi * f * t) * env_exp(t, 9))
    return fade(norm(b))


def crash():
    n = dur(1.6)
    b = []
    for i in range(n):
        t = i / SR
        b.append((rng.random() * 2 - 1) * env_exp(t, 2.2))
    return fade(norm(b))


def riser(sec=2.0):
    n = dur(sec)
    b = []
    ph = 0.0
    for i in range(n):
        t = i / SR
        frac = t / sec
        f = 180 * (2 ** (frac * 4))  # exponential sweep up 4 octaves
        ph += 2 * math.pi * f / SR
        noise = (rng.random() * 2 - 1) * 0.35 * frac
        b.append((math.sin(ph) * 0.6 + noise) * (frac**1.5))
    return fade(norm(b), 4, 20)


def downlifter(sec=2.0):
    n = dur(sec)
    b = []
    ph = 0.0
    for i in range(n):
        t = i / SR
        frac = t / sec
        f = 900 * (2 ** (-frac * 4))
        ph += 2 * math.pi * f / SR
        noise = (rng.random() * 2 - 1) * 0.25 * (1 - frac)
        b.append((math.sin(ph) * 0.6 + noise) * ((1 - frac) ** 1.5))
    return fade(norm(b), 4, 20)


def sub_drop(sec=1.8):
    n = dur(sec)
    b = []
    ph = 0.0
    for i in range(n):
        t = i / SR
        f = 90 * (2 ** (-(t / sec) * 3))
        ph += 2 * math.pi * f / SR
        b.append(math.sin(ph) * env_exp(t, 1.4))
    return fade(norm(b))


def bass808(note_hz=55.0):
    n = dur(0.9)
    b = []
    ph = 0.0
    for i in range(n):
        t = i / SR
        ph += 2 * math.pi * note_hz * (1 + 0.04 * env_exp(t, 30)) / SR
        b.append(math.tanh(math.sin(ph) * 1.6) * env_exp(t, 3.2))
    return fade(norm(b))


def stab(chord=(220.0, 277.18, 329.63)):
    n = dur(0.7)
    b = []
    for i in range(n):
        t = i / SR
        s = 0.0
        for f in chord:
            s += math.sin(2 * math.pi * f * t) + 0.3 * math.sin(
                2 * math.pi * 2 * f * t
            )
        b.append(math.tanh(s * 0.5) * env_exp(t, 5))
    return fade(norm(b))


def airhorn():
    n = dur(0.9)
    b = []
    ph = [0.0, 0.0]
    for i in range(n):
        t = i / SR
        f = 466.16 + (3.0 if t > 0.03 else 0)
        ph[0] += 2 * math.pi * f / SR
        ph[1] += 2 * math.pi * f * 1.5 / SR
        s = math.sin(ph[0]) + 0.6 * math.sin(ph[1]) + 0.2 * math.sin(2 * ph[0])
        b.append(math.tanh(s) * (1 if t < 0.8 else 1 - (t - 0.8) * 10))
    return fade(norm(b), 2, 30)


def vinyl_stop(sec=1.2):
    n = dur(sec)
    b = []
    ph = 0.0
    for i in range(n):
        t = i / SR
        f = 330 * (1 - t / sec) ** 2
        ph += 2 * math.pi * f / SR
        b.append(
            math.sin(ph) * (1 - t / sec) * 0.8 + (rng.random() * 2 - 1) * 0.05
        )
    return fade(norm(b))


def sweep_noise(sec=1.5):
    n = dur(sec)
    b = []
    lp = 0.0
    for i in range(n):
        t = i / SR
        frac = t / sec
        a = 0.02 + 0.5 * frac
        lp += a * ((rng.random() * 2 - 1) - lp)
        b.append(lp * (frac**0.7))
    return fade(norm(b), 4, 30)


SAMPLES = [
    ("01_kick", kick),
    ("02_snare", snare),
    ("03_clap", clap),
    ("04_hat_closed", lambda: hat(False)),
    ("05_hat_open", lambda: hat(True)),
    ("06_rim", rim),
    ("07_tom", tom),
    ("08_crash", crash),
    ("09_riser", riser),
    ("10_downlifter", downlifter),
    ("11_sub_drop", sub_drop),
    ("12_808", bass808),
    ("13_stab_minor", lambda: stab((220.0, 261.63, 329.63))),
    ("14_airhorn", airhorn),
    ("15_vinyl_stop", vinyl_stop),
    ("16_noise_sweep", sweep_noise),
]


def write(name, buf):
    path = os.path.join(OUT, name + ".wav")
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        frames = b"".join(
            struct.pack("<h", max(-32767, min(32767, int(x * 32767))))
            for x in buf
        )
        w.writeframes(frames)
    return path, len(buf) / SR


if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    for name, fn in SAMPLES:
        path, sec = write(name, fn())
        print(f"{os.path.basename(path)}  {sec:.2f}s")
