# Key detection network

`keynet.bin` holds the weights of the neural key detector (`src/keynet.rs`):
the `openkeyscan3.pt` checkpoint of openkeyscan-analyzer
(https://github.com/rekordcloud/openkeyscan-analyzer, commit d5cac64), which
builds on MusicalKeyCNN (https://github.com/a1ex90/MusicalKeyCNN). It was
converted with `scripts/export-keynet.py`: batch norms folded into the
convolutions, weights stored as 16-bit floats.

The network is KeyNet from F. Korzeniowski and G. Widmer, "Genre-Agnostic Key
Classification With Convolutional Neural Networks", ISMIR 2018.

The weights come under the MIT licence below, which lets them be used in
DJ Sugar's GPL code as long as this notice stays with them.

```
MIT License

Copyright (c) 2025 Alexander Sommer

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
