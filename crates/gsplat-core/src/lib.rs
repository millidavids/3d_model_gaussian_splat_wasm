//! Core types and pipeline seams for the in-browser Gaussian-splatting model
//! generator (photos → camera poses → trained splat).
//!
//! UI-agnostic: the wasm app (and any native tooling) share these types. The
//! heavy compute lives elsewhere — splat training in [Brush] (Burn + wgpu), pose
//! estimation behind the [`pose::PoseEstimator`] seam — so this crate stays a
//! light, `wasm32`-friendly contract layer.
//!
//! [Brush]: https://github.com/ArthurBrussee/brush

pub mod pose;

pub use pose::{ImageSet, PoseEstimator, PosedScene};
