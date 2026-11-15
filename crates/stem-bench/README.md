# stem-bench

Times DJ Sugar's stem separation on this machine: Spleeter (instant stems on
load) and HTDemucs (high quality, in the background), on the GPU and the CPU.
It is a tool for choosing settings, not part of the app.

## On a Mac, without building

1. Open the latest **Stems benchmark** run in the repository's Actions tab and
   download the `stem-models` and `stem-bench-macos` artifacts.
2. Unzip both into one folder, so it holds `stem-bench` and a `models` folder
   (rename the unzipped `stem-models` folder to `models`).
3. In Terminal, in that folder:

   ```
   xattr -d com.apple.quarantine stem-bench
   chmod +x stem-bench
   ./stem-bench "/path/to/a track.mp3" --out stems
   ```

It prints how long each model takes. With `--out stems` it also writes every
stem as a WAV file to listen to.

## From source

    cargo run --release -p stem-bench -- TRACK --weights models

`scripts/models/convert_spleeter.py` makes `spleeter-4stems.safetensors`, and
the HTDemucs file comes from the URL the tool prints when it is missing. See
`src/main.rs` for all options.
