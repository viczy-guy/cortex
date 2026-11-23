# App icon

The dot that expands (#101). The SVGs are the sources; every file here is
generated from them.

- `source.svg` — full bleed, for Windows and Linux.
- `source-macos.svg` — the same on Apple's grid (824 of 1024).
- `source-small.svg` — one thick ring, for 32 px and below.

To regenerate: render each SVG to a 1024 px PNG, run `npx tauri icon` on
the full-bleed one, take `icon.icns` from a run on the macOS one, and
replace `32x32.png` with the small one at 32 px.
