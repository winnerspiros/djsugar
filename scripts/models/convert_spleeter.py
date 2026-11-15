#!/usr/bin/env python3
"""Converts Deezer's Spleeter 4-stem checkpoint to the safetensors file DJ Sugar loads.

    curl -LO https://github.com/deezer/spleeter/releases/download/v1.4.0/4stems.tar.gz
    mkdir 4stems && tar xzf 4stems.tar.gz -C 4stems
    pip install tensorflow-cpu numpy safetensors
    python3 convert_spleeter.py 4stems/model spleeter-4stems.safetensors

The weights are Deezer's (MIT). Each stem has its own U-Net; tensors are named
`<stem>.<layer>.<param>` and laid out for burn (NCHW): convolutions as
[out, in, kh, kw], transposed convolutions as [in, out, kh, kw]. Batch norms
are folded to a per-channel `scale` and `shift` (Keras epsilon 1e-3). Stored
as fp16 to halve the download.
"""
import struct
import sys

import numpy as np
import tensorflow as tf
from safetensors.numpy import save_file

STEMS = ["vocals", "drums", "bass", "other"]
BN_EPS = 1e-3


def name(base, i):
    return base if i == 0 else f"{base}_{i}"


def to_fp16(x):
    """`x` as fp16, rounded the same way on every machine."""
    values = np.asarray(x, dtype=np.float64).ravel().tolist()
    packed = struct.pack(f"<{len(values)}e", *values)
    return np.frombuffer(packed, dtype="<f2").reshape(np.shape(x)).copy()


def main(checkpoint, out):
    r = tf.train.load_checkpoint(checkpoint)
    # Everything is worked out in float64 and narrowed to fp16 by Python's
    # own (correctly rounded) packing, not numpy's cast, which uses the CPU's
    # conversion instructions where it has them and gave different bytes on
    # some GitHub runners. So the file is the same on every machine and
    # matches SHA256SUMS.
    def get(key):
        return r.get_tensor(key).astype(np.float64)

    tensors = {}
    for s, stem in enumerate(STEMS):
        # Keras numbered the layers in the order they were built: per stem,
        # 6 encoder convs then the final conv, 6 transposed convs, 12 batch
        # norms (6 encoder, 6 decoder).
        for k in range(6):
            conv = name("conv2d", 7 * s + k)
            tensors[f"{stem}.enc{k}.weight"] = get(f"{conv}/kernel").transpose(3, 2, 0, 1)
            tensors[f"{stem}.enc{k}.bias"] = get(f"{conv}/bias")
            up = name("conv2d_transpose", 6 * s + k)
            # TF stores transposed kernels as [kh, kw, out, in].
            tensors[f"{stem}.dec{k}.weight"] = get(f"{up}/kernel").transpose(3, 2, 0, 1)
            tensors[f"{stem}.dec{k}.bias"] = get(f"{up}/bias")
        final = name("conv2d", 7 * s + 6)
        tensors[f"{stem}.out.weight"] = get(f"{final}/kernel").transpose(3, 2, 0, 1)
        tensors[f"{stem}.out.bias"] = get(f"{final}/bias")
        for k in range(12):
            bn = name("batch_normalization", 12 * s + k)
            gamma, beta = get(f"{bn}/gamma"), get(f"{bn}/beta")
            mean, var = get(f"{bn}/moving_mean"), get(f"{bn}/moving_variance")
            scale = gamma / np.sqrt(var + BN_EPS)
            part = f"enc{k}" if k < 6 else f"dec{k - 6}"
            tensors[f"{stem}.{part}.bn_scale"] = scale
            tensors[f"{stem}.{part}.bn_shift"] = beta - mean * scale
    tensors = {k: to_fp16(v) for k, v in tensors.items()}
    save_file(tensors, out, metadata={"model": "spleeter-4stems", "stems": ",".join(STEMS)})
    print(f"{out}: {len(tensors)} tensors")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
