# gsplat-wasm

Turn a series of **photos** (later, video) into a **3D Gaussian splat**, entirely in your
**browser** — a free, open, static **WebAssembly + WebGPU** site. No install, no backend,
no CUDA.

Sister project to the classical-photogrammetry [`3d_model_generator`](../3d_model_generator)
(COLMAP/OpenMVS → mesh), which stays usable as-is. This repo bets on the browser-native
path: Gaussian splatting, whose heavy compute is GPU-shaped and so maps onto WebGPU — the
one accelerator a static web page can use.

## Status

**Phase 1 — viewer working.** An in-browser WebGPU viewer renders a Gaussian splat you can
orbit and zoom. It currently shows a procedurally-generated sample splat (a colour-by-normal
sphere with RGB axes); loading your own `.ply`/`.spz` and the training pipeline come next.
See [`docs/DESIGN.md`](docs/DESIGN.md) for the full architecture, decisions, and phased plan.

## Run it

```bash
# one-time: install the wasm build tool
cargo install trunk            # or grab a prebuilt binary from trunk-rs releases
rustup target add wasm32-unknown-unknown

trunk serve --open             # builds wasm + serves at http://127.0.0.1:8137/
```

Needs a WebGPU-capable browser (Chrome/Edge 113+, or Safari 18+). Drag to orbit, scroll to
zoom. No cross-origin-isolation headers required — it hosts as plain static files.

## How it will work

1. **Poses** — classical CPU structure-from-motion (the [rust-cv](https://github.com/rust-cv)
   ecosystem), compiled to wasm, behind a swappable `PoseEstimator` trait.
2. **Train** — Gaussian-splat training in-browser via [Brush](https://github.com/ArthurBrussee/brush)
   (Burn + wgpu). This is the ML, and it runs on your GPU via WebGPU.
3. **View / share** — interactive splat rendering via `wgpu-3dgs-viewer`.

A game-ready **mesh** is a later, local step (Blender) — the public tool outputs splats.

## Principles

- **Fully static & free.** No backend, no paid services.
- **Permissively licensed throughout.** The tool is free/open *and* its outputs must be
  usable in a commercial game, so every model/library is Apache-2.0 / MIT (no CC-BY-NC).
- **Rust + WASM + WebGPU.**

## Layout

- `crates/gsplat-core` — UI-agnostic core types and pipeline seams (e.g. `PoseEstimator`).
- `crates/gsplat-app` — the wasm web app: WebGPU splat viewer (winit + wgpu + `wgpu-3dgs-viewer`).
- `index.html` / `Trunk.toml` — the static-site entry and build config.
- `docs/DESIGN.md` — the living design doc and plan.
- `docs/CONCEPTS.md` — a from-scratch primer on every concept the project uses.

## License

MIT OR Apache-2.0.
