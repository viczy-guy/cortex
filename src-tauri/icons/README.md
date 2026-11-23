# App icon

The eyes (#116; 0.3.0 had the dot that expands, #101). The SVGs are the
sources; every file here is generated from them.

- `source.svg` — full bleed, for Windows and Linux.
- `source-macos.svg` — the same on Apple's grid (824 of 1024).
- `source-small.svg` — bigger eyes without the glow, for `32x32.png`.
- `source.png` — `source.svg` rendered at 1024 px, the input of `npx tauri icon`.

To regenerate: render each SVG to a 1024 px PNG, run `npx tauri icon` on
the full-bleed one, take `icon.icns` from a run on the macOS one, and
replace `32x32.png` with the small one rendered at 32 px. Only that file
uses the small drawing; the `.ico` frames and `Square30x30Logo.png` come
from the full-bleed run.
