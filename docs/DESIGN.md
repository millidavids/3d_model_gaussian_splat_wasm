# Design: In-Browser Gaussian-Splatting 3D Model Generator

> Status: **Phase 0 (scaffolding)**. This is the living design doc. It was drafted,
> independently reviewed by a staff-level review pass, and revised. Section 9 records the
> research spikes that shaped it.

## 1. Goal & hard constraints

Turn a series of **photos** (and, later, **video**) into a **3D Gaussian splat**, written
in **Rust**, running **fully in the browser** as a **static WebAssembly + WebGPU site**.

Locked decisions (2026-06-24):
- **Fully static from day one.** No backend we host, no paid services, no local pre-step.
  Everything happens in the browser tab.
- **Free & open tool, but commercially-usable outputs.** The site is free/open-source. The
  generated 3D assets feed a **future monetized game**, so every model/library in the
  pipeline must be **permissively licensed (Apache-2.0 / MIT)**. CC-BY-NC models are
  disqualified — their outputs are commercially risky.
- **Splat first, mesh later.** v1 deliverable is a viewable/shareable Gaussian splat. Mesh
  extraction is a later, *local/desktop* step (Blender) — see §6.
- **Sibling to the frozen `3d_model_generator`**, which stays usable as-is.

## 2. Why Gaussian splatting (vs. porting the classical pipeline)

Classical COLMAP+OpenMVS photogrammetry is CPU-bound and hostile to WASM (4 GB address
cap, no GPU, threads need COOP/COEP). Gaussian-splatting's heavy compute is **GPU-shaped**,
and the browser exposes a GPU via **WebGPU** — so the expensive part maps onto the one
accelerator a static page can actually use. That inverts the feasibility story.

## 3. Pipeline, decomposed

| # | Sub-problem | Approach | Where it runs |
|---|-------------|----------|---------------|
| 1 | photos → camera poses + sparse init | **classical CPU SfM (rust-cv)**, behind a `PoseEstimator` trait | browser (wasm, CPU) |
| 2 | poses + images → trained splat | **Brush** (Burn + wgpu) | browser (WebGPU) |
| 3 | splat → interactive viewer / shareable file | **`wgpu-3dgs-viewer`** | browser (WebGPU) |
| 4 | *(later)* splat → mesh | Blender, **local/desktop** | offline |
| 5 | *(later)* video → frames | WebCodecs | browser |

Everything but #1 is integration of existing Rust. **#1 is the critical-path research risk**
(see §9, Spike 2).

## 4. The pose-estimation decision (the crux)

Brush needs camera poses; it has no SfM. Because we're fully static, poses must be produced
**in the browser**. Research (§9, Spike 2) found that feed-forward ML posers (VGGT,
MapAnything, DUSt3R/MASt3R, Fast3R, Spann3R) are all ~1B-param transformers, multiple GB of
weights — they do **not** fit the 4 GB wasm space + WebGPU 128/256 MiB buffer limits today,
and the best ones are CC-BY-NC (license-incompatible with our monetized assets).

**Decision:** v1 poses come from **classical CPU SfM compiled to wasm** via the
[rust-cv](https://github.com/rust-cv) ecosystem (`akaze` features → matching → `p3p` pose →
`vslam` bundle adjustment). Pure Rust, MIT/Apache, CPU→wasm. A `PoseEstimator` trait keeps
the backend swappable so a feed-forward ML model drops in later, once models shrink or
browser limits grow. **Open risk:** rust-cv SfM is research-grade, not COLMAP-robust, and
its wasm-readiness is unproven — this is the biggest end-to-end risk and is de-risked in
parallel with Phases 1–2, which don't depend on it.

## 5. Build on Brush — with eyes open

[Brush](https://github.com/ArthurBrussee/brush) (Apache-2.0) trains AND renders splats
in-browser via WebGPU. **Caveat:** it is *not* a packaged library — it's an app/CLI over
~25 undocumented internal crates, not on crates.io. "We don't reinvent the rasterizer" is an
assumption until Spike 1 proves it; the likely reality is vendoring/forking a pinned commit.
The Phase 1 viewer therefore uses the *real, consumable* `wgpu-3dgs-viewer` crate, keeping
the first milestone off the unproven Brush-as-library bet.

## 6. Splat → mesh (later, local)

A splat is a radiance field, not game-ready geometry. Mesh extraction (SuGaR / 2DGS) has no
mature Rust implementation and can't live in the static site. Pragmatic split: **the public
tool outputs splats; meshes are baked locally in Blender** (experimental 3DGS-import addons +
Blender's retopo/decimate), reusing the host-native Blender workflow from the frozen repo.
A truly in-browser splat→mesh (2DGS/TSDF in Rust/WASM) is greenfield — deferred.

## 7. Phases (each ships something visible)

- **Spikes (week 1, do first):** see §9 — (1) Is Brush consumable? + headers check;
  (2) pose feasibility memo. **DONE for Spike 2; Spike 1 pending.**
- **Phase 0 — Scaffold:** repo, Rust workspace, wasm/wgpu + `trunk` build, CI, license,
  deployed "hello WebGPU". *(in progress)*
- **Phase 1 — Viewer first (lowest risk):** load a `.ply`/`.splat`, render in-browser via
  `wgpu-3dgs-viewer`; deploy a public orbit-the-splat URL. Proves WASM+WebGPU+hosting.
- **Phase 2 — Training from a prepared (posed) dataset:** adopt Brush's training path on a
  known COLMAP/Nerfstudio sample; live in-browser training. The "ML in your tab" milestone.
- **Phase 3 — Pose (rust-cv):** raw photos → posed dataset, in-browser CPU SfM behind the
  `PoseEstimator` trait. The long pole.
- **Phase 4 — End-to-end, fully static:** photos → pose → train → view/share.
- **Phase 5+ — Stretch:** video input (WebCodecs); local Blender splat→mesh bridge;
  `.spz`/SOG compression; sharing UX.

## 8. Cross-cutting risks

- **Pose (rust-cv) robustness + wasm-readiness** — the #1 end-to-end risk; de-risk early,
  in parallel with Phases 1–2.
- **Memory ceilings** (4 GB wasm + WebGPU 128/256 MiB buffers): cap image count, resolution,
  Gaussian count; surface limits in UI. wasm64 doesn't lift the WebGPU buffer limits.
- **COOP/COEP / SharedArrayBuffer:** GPU compute may need none; settle via Spike 1's
  `crossOriginIsolated` test. If needed, `coi-serviceworker` works even on GitHub Pages — do
  not pre-eliminate hosts.
- **Browser support:** WebGPU is broad in 2026; the binding constraint is what Brush/wgpu
  runs on (Chrome 134+/Edge). Re-test current Safari/Firefox.
- **Brush bus-factor:** single maintainer, churning internals, no library contract → budget
  for maintaining a fork, not just pinning a SHA.
- **Quality:** render and *eyeball* splat quality each phase — mechanics passing ≠ good output.

## 9. Research spikes

### Spike 1 — Is Brush consumable as a dependency? *(PENDING)*
From a throwaway crate, depend on `brush-render`/`brush-train` (pinned SHA) and drive
train+render without forking; read `crossOriginIsolated` on the web build to settle hosting.
Outcome decides depend-vs-vendor/fork and the host requirement.

### Spike 2 — Can any pose model fit a browser? *(DONE — verdict below)*
- Feed-forward 3D models (VGGT, MapAnything, DUSt3R/MASt3R, Fast3R, Spann3R) are all
  **~1B params, multi-GB**. They exceed the 4 GB wasm space + WebGPU 128/256 MiB buffer
  limits, especially as image count grows (attention scales with views).
- Licensing: VGGT, DUSt3R/MASt3R, and the primary MapAnything checkpoint are **CC-BY-NC** —
  disqualified for monetized assets. MapAnything has an Apache checkpoint but it's still 1B.
- **Verdict:** in-browser feed-forward ML pose is premature in mid-2026 and license-risky.
  v1 uses **classical CPU-wasm SfM (rust-cv)**; ML pose stays a future trait-swap. The
  in-browser ML that fits today is the **splat training** (Brush), which is the headline ML.
