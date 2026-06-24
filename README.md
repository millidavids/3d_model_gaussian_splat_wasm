# gsplat-wasm

Turn a series of **photos** (later, video) into a **3D Gaussian splat**, entirely in your
**browser** — a free, open, static **WebAssembly + WebGPU** site. No install, no backend,
no CUDA.

Sister project to the classical-photogrammetry [`3d_model_generator`](../3d_model_generator)
(COLMAP/OpenMVS → mesh), which stays usable as-is. This repo bets on the browser-native
path: Gaussian splatting, whose heavy compute is GPU-shaped and so maps onto WebGPU — the
one accelerator a static web page can use.

## Status

**Phase 0 — scaffolding.** Nothing user-facing yet. See [`docs/DESIGN.md`](docs/DESIGN.md)
for the full architecture, decisions, and phased plan.

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
- `docs/DESIGN.md` — the living design doc and plan.

## License

MIT OR Apache-2.0.
