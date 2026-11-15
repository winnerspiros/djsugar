#!/usr/bin/env python3
"""Converts an openkeyscan-analyzer KeyNet checkpoint into the weights file
DJ Sugar's neural key detector reads (crates/engine/models).

    python3 scripts/export-keynet.py openkeyscan3.pt crates/engine/models/keynet.bin

Each batch norm is folded into the convolution before it. The file is:
"OKSKEY01", the layer count (u32), then per layer its output channels,
input channels, kernel height and width (u32 each), its biases (f32) and its
weights (f16, PyTorch's order). Needs PyTorch.
"""
import struct
import sys

import numpy as np
import torch

LAYERS = [f"conv{i}" for i in range(1, 10)]


def main(src, dst):
    sd = torch.load(src, map_location="cpu", weights_only=False)
    out = bytearray(b"OKSKEY01")
    out += struct.pack("<I", len(LAYERS))
    for name in LAYERS:
        w = sd[f"{name}.conv.weight"].double().numpy()
        g = sd[f"{name}.bn.weight"].double().numpy()
        b = sd[f"{name}.bn.bias"].double().numpy()
        mean = sd[f"{name}.bn.running_mean"].double().numpy()
        var = sd[f"{name}.bn.running_var"].double().numpy()
        scale = g / np.sqrt(var + 1e-5)
        w = w * scale[:, None, None, None]
        bias = b - mean * scale
        o, i, kh, kw = w.shape
        out += struct.pack("<4I", o, i, kh, kw)
        out += bias.astype("<f4").tobytes()
        out += w.astype("<f2").tobytes()
    with open(dst, "wb") as f:
        f.write(out)


if __name__ == "__main__":
    main(*sys.argv[1:3])
