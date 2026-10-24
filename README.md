# ALIEN SPACE DROP

A little arcade game written in Rust + [miniquad](https://miniquad.com), styled
after an early-1980s **Apple II hi-res (280x192) BASIC game**.

Land your descending saucer on a pad by steering with **A / S** and speeding up
with **D**. Dodge the saucers flying horizontally across the field. The smaller
the pad, the more points.

## Play it

- **Windows desktop** — `dist/alien-space-drop.exe` (standalone, no install).
- **Web (GitHub Pages)** — deploy the included GitHub Action, then open:
  `https://<your-username>.github.io/<repo-name>/`

  One-time setup: repo **Settings → Pages → Build and deployment → Source:
  GitHub Actions**. Every push to `main` rebuilds the wasm and redeploys.

### Build locally

```sh
sh compile.sh         # Windows .exe  -> dist/alien-space-drop.exe
sh compile_web.sh     # WebAssembly   -> web/alien_space_drop.wasm
npx http-server web   # serve the web build locally
```

### Dev tools

```sh
./target/release/alien-space-drop.exe --save shot.png --playing --t 6
./target/release/alien-space-drop.exe --save frame.png --ascii   # text preview
cargo test            # simulation unit tests
```

## Controls

| Key            | Action                   |
| -------------- | ------------------------ |
| `A` / `←`      | move left                |
| `S` / `→`      | move right               |
| `D` / `↓`      | boost descent            |
| `SPACE`/`ENTER`| start / restart          |
| `ESC`          | quit                     |

## Layout

- `src/game.rs`   — pure, unit-tested simulation (no render/game-lib deps).
- `src/render.rs` — 280x192 Apple II palette framebuffer, sprites, ROM font.
- `src/audio.rs`  — beeper SFX (cpal on desktop, WebAudio bridge on web).
- `src/main.rs`   — miniquad window + CRT scanline shader + headless tooling.
- `web/`          — the static web site (`index.html`, vendored `gl.js`).
- `.github/workflows/deploy-web.yml` — GitHub Pages build & deploy.