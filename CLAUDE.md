# gsplat-wasm — Project Documentation

In-browser Gaussian-splatting 3D model generator: **photos → camera poses → trained splat**,
running fully client-side as a static **WebAssembly + WebGPU** site. Free/open tool;
permissively-licensed throughout so outputs are usable in a commercial game. See
`docs/DESIGN.md` for the architecture and plan, `docs/HANDOFF.md` for conversation state, and
`docs/CONCEPTS.md` for the from-scratch concept primer.

## Learning Primer — `docs/CONCEPTS.md` (MAINTAIN AS WE GO)

The repo owner is using this project to **learn** the concepts, not just to ship it. So we keep
a from-scratch teaching primer at **`docs/CONCEPTS.md`**, modelled on the sibling
[`../lo_fi_converter_blender_addon/docs/CONCEPTS.md`](../lo_fi_converter_blender_addon/docs/CONCEPTS.md).

**Standing rule:** whenever we introduce, use, or debug a new concept, method, term, library,
or algorithm — update `docs/CONCEPTS.md` in the same change. Do not let code outrun the primer.

Style to match (see the sibling):
- **Teach from zero.** Assume no prior 3D/GPU/Rust-wasm knowledge; define every term in plain
  language, give a **mental model / analogy**, and say *why it matters* and *how we used it here*.
- **Structure:** an anchoring idea up top → numbered concept sections → a **"hard lessons"**
  section (the bugs that taught a principle) → a **named-methods table** (real technique names +
  sources, for further reading) → a **glossary** of one-line definitions.
- Keep it honest and specific to *this* codebase (reference our actual files/decisions), and
  prune anything that becomes wrong.

## Technology Stack

- **Language**: Rust (edition 2024, pinned 1.93.1; `wasm32-unknown-unknown` target).
- **GPU / ML**: WebGPU via `wgpu`; splat training via [Brush](https://github.com/ArthurBrussee/brush)
  (Burn + wgpu). Rendering via `wgpu-3dgs-viewer`.
- **Pose estimation**: classical CPU SfM (the [rust-cv](https://github.com/rust-cv) ecosystem),
  compiled to wasm, behind the `gsplat-core::PoseEstimator` trait (swappable for an ML poser later).
- **Web build**: `trunk` + `wasm-bindgen`.
- **Licensing rule (hard):** every model/library must be **Apache-2.0 / MIT**. No CC-BY-NC
  (their outputs are commercially risky for the downstream game). The repo is `MIT OR Apache-2.0`.

## Rust Best Practices

### Module Structure — Feature-Sliced & Granular
**Prefer many small concern-focused files over a few large canonical ones.** Group by concern,
not by file type. Reserve canonical names (`types.rs`, `constants.rs`) for genuinely
cross-cutting content.

**Hard rules:**
- `mod.rs` / `lib.rs` do `mod` declarations + `pub use` re-exports ONLY. No logic, types, or constants.
- Files exceeding ~300 lines must be split unless every line is genuinely cohesive.
- `styles.rs` is forbidden. Constants live with their feature, or in a `constants.rs` for
  cross-cutting values only.

### Module Visibility
- `pub(super)` for items only needed within a module; `pub(crate)` for crate-internal APIs;
  `pub` only for true public API.

### Function Arguments
- Helper functions: keep argument counts reasonable; when helpers share a parameter group,
  extract a params struct.
- Constructors with many fields: prefer `#[allow(clippy::too_many_arguments)]` on `new()`.

### Constants Organization
- Crate-wide constants in a top-level `constants.rs` (split by concern past ~200 lines).
- Module-specific constants live in feature files, or a module `constants.rs` if shared across
  feature files. Constants used by exactly one feature file are inlined there.
- Use `pub(super)` for module-internal constants.

### Error Handling
- `Result<T, E>` for fallible operations; `thiserror` for library errors, `anyhow` for app context.
- Never `.unwrap()` in production code; `.expect()` only for invariants, with descriptive messages.

### Logging
- Use `tracing` (or `log`) macros. Avoid excessive logging; remove debug logging when done.
- Note: in wasm, route logs through `tracing-wasm` / `console_log` to the browser console.

### Code Sharing & Simplification
- Extract shared logic rather than duplicating; check existing implementations for shared patterns.
- Feature-specific behavior should be minimal overrides on shared helpers.
- `/simplify` before releasing to reduce duplication.

## Build & Testing

### Iterative compile checks (USE THIS during work)
```bash
cargo check                                   # fast frontend check (native)
cargo check --target wasm32-unknown-unknown   # the deploy target — keep it green
cargo fix --allow-dirty                       # auto-remove unused imports
```

### Testing & quality gates
```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
```

### Web (once the app crate exists)
```bash
trunk serve     # local dev server
trunk build --release
```
Per project norm: **render and visually verify** splat quality — mechanics passing ≠ good output.

## Git Workflow
- **Never commit unless explicitly instructed.**
- The sibling `../3d_model_generator` repo is **frozen** — do not edit it from here.
