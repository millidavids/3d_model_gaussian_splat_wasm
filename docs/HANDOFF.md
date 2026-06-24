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

## Current repo state (Phase 1 complete, verified)

- **Phase 0 committed** (`711069b`). **Phase 1 work is staged in the tree but NOT yet
  committed** (awaiting the user's go-ahead, per house rule).
- Rust workspace (resolver 3, edition 2024, Rust 1.93.1), `wasm32-unknown-unknown` target.
- `crates/gsplat-core` — the `PoseEstimator` trait seam + `ImageSet`/`PosedScene` + a test.
- `crates/gsplat-app` — **the Phase 1 viewer**: winit 0.30 + wgpu 29 + `wgpu-3dgs-viewer`
  0.7. Modules: `scene` (procedural sample splat + `bounds` for framing),
  `camera_control` (orbit camera over `gs::Camera`, `frame()` to fit a loaded splat),
  `graphics` (wgpu setup + per-frame canvas-size sync + render + `load_gaussians`),
  `app` (winit event loop, web + native; a one-slot load inbox), `loader` (web-only
  drag-and-drop `.ply`/`.spz` → `Gaussians`), `web_entry` (`#[wasm_bindgen(start)]`).
- **Drag-and-drop loading works**: drop a `.ply`/`.spz` and it swaps in, camera auto-frames.
  Verified in-browser by dispatching a synthetic file drop (console: "Reading PLY format with
  60 Gaussians" → "loaded small.ply"). `scene::sample_splat_ply_roundtrips` (set
  `DUMP_SAMPLE_PLY=<path>`) emits a small `.ply` fixture for manual testing.
- Web build: root `index.html` + `Trunk.toml`; `.cargo/config.toml` sets `+simd128`.
  `trunk` installed at `~/.cargo/bin`, plus matching `wasm-bindgen-cli` 0.2.126 (the
  trunk-bundled download 404s for that version — installed from source instead).
- **Visually verified in Chrome** (`trunk serve` → `http://127.0.0.1:8137/`): renders the
  splat ball, auto-spins, drag-orbit / scroll-zoom, no console errors, 25 200 gaussians.
- Green: `cargo check` (native + wasm), `cargo test` (6 pass), `cargo clippy -D warnings`
  (both targets), `cargo fmt --check`, `trunk build`.

### Gotchas worth remembering (load-bearing)
- `wgpu-3dgs-core` returns `impl Future + Send` from an async buffer download → needs wgpu's
  **`fragile-send-sync-non-atomic-wasm`** feature on wasm (sound: no threads).
- glam **`Vec3A: Pod`** only exists with **`+simd128`** → set in `.cargo/config.toml`.
- winit's web `inner_size()` is unreliable at init → `graphics::drawable_size` reconciles the
  surface to the canvas CSS-size × DPR **every frame** (fixes a full-screen color smear).
- The `Gaussian` struct holds **linear** `scale` and **linear RGBA** `color` (the GPU pod
  builds covariance from linear scale; no exp/log on our side).

## Where we paused — open decisions for the user

Committed through Phase 1.5: `446bbc5` (loader). Deploy work (release build verified
18 MB → 2.3 MB; `.github/workflows/deploy.yml`; README/CONCEPTS updates) is **staged but
uncommitted**.

1. **Finish the deploy** (two manual steps only the owner can do): in the GitHub repo,
   Settings → Pages → Source = "GitHub Actions"; then **push** (the workflow deploys on push
   to `main`/`master`). Remote exists: `origin git@github.com:millidavids/3d_model_gaussian_splat_wasm.git`.
   Note: local branch is `master`; the workflow triggers on both `main` and `master`.
2. **Next step — Phase 2 (training from a posed dataset).** Opens with the *other half* of
   Spike 1: is Brush (`brush-render`/`brush-train`) consumable as a pinned dependency, or must
   we vendor/fork?

## Pointers

- Plan / architecture / risks / spikes: [`DESIGN.md`](DESIGN.md)
- Concept primer (kept current as we work — house rule): [`CONCEPTS.md`](CONCEPTS.md)
- House coding conventions: [`../CLAUDE.md`](../CLAUDE.md)
- Auto-memory for this repo includes `project-gsplat-wasm` (the locked decisions, condensed).
