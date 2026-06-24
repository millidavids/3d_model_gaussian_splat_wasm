//! The pose-estimation seam: photos → camera poses + sparse init points.
//!
//! The fully-static, in-browser constraint makes this the pipeline's critical
//! path (see `docs/DESIGN.md` §4). The backend is deliberately abstracted behind
//! [`PoseEstimator`]: v1 targets **classical CPU SfM compiled to wasm** (the
//! rust-cv ecosystem), while a feed-forward ML poser is a future swap once a
//! permissively-licensed model fits the browser's memory limits.

use std::path::PathBuf;

/// A set of input images to recover camera geometry from.
#[derive(Debug, Clone, Default)]
pub struct ImageSet {
    /// Paths (or, in-browser, virtual paths) to the input photographs.
    pub images: Vec<PathBuf>,
}

/// Camera poses + sparse initialization points, in a form a splat trainer
/// (Brush, via COLMAP / Nerfstudio conventions) can ingest.
///
/// The concrete shape (per-view intrinsics + extrinsics, sparse point cloud) is
/// pinned once Brush's ingestion path is confirmed (Spike 1) — kept opaque until
/// then to avoid churn.
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct PosedScene {}

/// Recover camera geometry from a set of images.
///
/// Implementors: classical SfM (v1, rust-cv → wasm) or a feed-forward ML model
/// (future). The trait is the swap point so the rest of the pipeline is
/// backend-agnostic.
pub trait PoseEstimator {
    /// Backend-specific failure (e.g. too few images registered).
    type Error: std::error::Error;

    /// Estimate poses + sparse init points for `images`.
    fn estimate(&self, images: &ImageSet) -> Result<PosedScene, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_set_collects_paths() {
        let set = ImageSet {
            images: vec![PathBuf::from("a.jpg"), PathBuf::from("b.jpg")],
        };
        assert_eq!(set.images.len(), 2);
    }
}
