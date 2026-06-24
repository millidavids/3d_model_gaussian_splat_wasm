# Conversation Handoff

> Purpose: let a fresh Claude Code session (started **in this repo**) continue the
> conversation that scoped and scaffolded the project — without re-deriving everything.
> The full plan lives in [`DESIGN.md`](DESIGN.md); this file is the narrative + the
> exact point we paused at.

## How we got here (the short version)

1. The original ask (in the sibling [`3d_model_generator`](../../3d_model_generator) repo)
   was: port the whole classical photogrammetry pipeline (COLMAP + OpenMVS + rembg, all
   C++/Python) to pure Rust and compile it to WASM for a static site.
2. Assessment: a faithful port is a multi-year, research-grade effort, and even a perfect
   port hits WASM walls — the 4 GB address cap, no GPU for CPU-bound dense MVS, threads
   needing COOP/COEP. Classical MVS is fundamentally hostile to the browser.
3. Pivot the user chose: **Gaussian splatting** instead. Its heavy compute is GPU-shaped,
   and WebGPU is the one accelerator a static page can use — this *inverts* the feasibility
   story. The lo-fi mesh processing already lives in a separate sibling repo, so this repo
   is now purely "photos → 3D model (a splat)".
4. We drafted a plan, had it **independently gap-reviewed** (staff-level pass), corrected
   five load-bearing assumptions, and ran the make-or-break pose-feasibility research.

## Decisions locked (see DESIGN.md §1, §4, §9)

- **Fully static, free, no backend, no paid services.** Everything in the browser tab.
- **Permissive (Apache-2.0 / MIT) models & libraries only** — because the *outputs* feed a
  future **monetized game**, even though the tool itself is free/open. This disqualifies the
  CC-BY-NC feed-forward pose models (VGGT, DUSt3R/MASt3R, MapAnything-NC).
- **Build on [Brush](https://github.com/ArthurBrussee/brush)** (Apache-2.0, Rust/Burn/wgpu)
  for in-browser splat training; render via `wgpu-3dgs-viewer`. Brush is NOT a packaged
  library — expect to vendor/fork (Spike 1 will confirm).
- **Poses = classical CPU SfM via rust-cv, compiled to wasm** — NOT a big ML model. Spike 2
  verdict: 1B-param feed-forward posers don't fit 4 GB wasm + WebGPU buffer limits today.
  Kept behind a `PoseEstimator` trait so an ML poser can swap in later. The in-browser ML
  that *does* fit is the splat **training** (Brush).
- **Splat first; mesh later, locally in Blender** (can't live in the static site).

## Current repo state (Phase 0, verified)

- Git initialized; **nothing committed yet** (awaiting the user's go-ahead).
- Rust workspace (resolver 3, edition 2024, Rust 1.93.1), `wasm32-unknown-unknown` target.
- `crates/gsplat-core` — the `PoseEstimator` trait seam + `ImageSet`/`PosedScene` + a test.
- `README.md`, `docs/DESIGN.md` (canonical plan), this file.
- Green: `cargo check`, `cargo test` (1 pass), `cargo fmt --check`, **and
  `cargo check --target wasm32-unknown-unknown`**.

## Where we paused — two open decisions for the user

1. **Commit the Phase 0 scaffold?** (House rule: never commit without explicit approval.)
2. **Next step:** Spike 1 (validate Brush is consumable + `crossOriginIsolated` check) vs.
   jump straight to the **Phase 1 viewer** (`wgpu-3dgs-viewer` + `trunk` → orbit a sample
   splat on a deployed page). Assistant's lean: **viewer first** — visible win, independent
   of the Brush question, derisks the wasm+WebGPU+hosting toolchain.

## Pointers

- Plan / architecture / risks / spikes: [`DESIGN.md`](DESIGN.md)
- House coding conventions: [`../CLAUDE.md`](../CLAUDE.md)
- Auto-memory for this repo includes `project-gsplat-wasm` (the locked decisions, condensed).
