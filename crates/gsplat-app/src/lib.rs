//! Phase 1 web app: render the sample Gaussian splat in the browser via WebGPU
//! and let the user orbit it. Proves the WASM + WebGPU + hosting toolchain
//! end-to-end, independent of the (still-unproven) Brush training path.

mod app;
mod camera_control;
mod graphics;
mod scene;
#[cfg(target_arch = "wasm32")]
mod web_entry;

pub use app::run;
