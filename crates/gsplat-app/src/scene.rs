//! A procedurally-generated sample splat, so Phase 1 needs no external asset.
//!
//! The shape — a direction-coloured sphere plus red/green/blue coordinate axes —
//! is deliberately unambiguous: the rainbow gradient gives depth cues and the
//! axes make orientation obvious while orbiting, so a glance confirms the
//! WASM → WebGPU → viewer path actually renders 3D. A real `.ply`/`.spz` loader
//! drops in behind the same [`Gaussians`] type later (it is just another source).

use glam::{Quat, U8Vec4, Vec3};
use wgpu_3dgs_viewer::core::{Gaussian, Gaussians, IterGaussian};

/// Gaussians placed over the sphere surface.
const SPHERE_COUNT: usize = 24_000;
/// Gaussians per coordinate axis.
const AXIS_COUNT: usize = 400;
/// Sphere radius, in the world units the orbit camera frames.
const SPHERE_RADIUS: f32 = 1.0;
/// How far the axis bars extend past the sphere.
const AXIS_LENGTH: f32 = 1.5;
/// Isotropic standard deviation of a sphere-surface splat (linear, not log).
const SPHERE_SCALE: f32 = 0.014;
/// Isotropic standard deviation of an axis splat.
const AXIS_SCALE: f32 = 0.02;

/// Build the sample splat: a direction-coloured sphere with RGB axes.
pub fn sample_splat() -> Gaussians {
    let mut gaussians = Vec::with_capacity(SPHERE_COUNT + 3 * AXIS_COUNT);
    push_sphere(&mut gaussians);
    push_axis(&mut gaussians, Vec3::X, U8Vec4::new(220, 40, 40, 255));
    push_axis(&mut gaussians, Vec3::Y, U8Vec4::new(40, 200, 60, 255));
    push_axis(&mut gaussians, Vec3::Z, U8Vec4::new(50, 90, 230, 255));
    Gaussians::from(gaussians)
}

/// Axis-aligned bounding centre and radius (half the box diagonal) of a splat,
/// used to frame an arbitrary loaded splat in the orbit camera. Empty → unit.
pub fn bounds(gaussians: &Gaussians) -> (Vec3, f32) {
    let mut min = Vec3::splat(f32::INFINITY);
    let mut max = Vec3::splat(f32::NEG_INFINITY);
    for g in gaussians.iter_gaussian() {
        min = min.min(g.pos);
        max = max.max(g.pos);
    }
    if !min.is_finite() || !max.is_finite() {
        return (Vec3::ZERO, 1.0);
    }
    let center = (min + max) * 0.5;
    (center, (max - center).length().max(1e-3))
}

/// Lay out points evenly on a sphere (Fibonacci lattice), coloured by surface
/// direction so each facing reads as a distinct hue.
fn push_sphere(out: &mut Vec<Gaussian>) {
    // Golden-angle increment spreads successive points without clustering.
    let golden_angle = std::f32::consts::PI * (3.0 - 5.0_f32.sqrt());
    for i in 0..SPHERE_COUNT {
        let t = i as f32 / (SPHERE_COUNT - 1) as f32;
        let y = 1.0 - 2.0 * t; // walk the poles, +1 → -1
        let ring_radius = (1.0 - y * y).max(0.0).sqrt();
        let theta = golden_angle * i as f32;
        let dir = Vec3::new(theta.cos() * ring_radius, y, theta.sin() * ring_radius);

        out.push(splat(
            dir * SPHERE_RADIUS,
            direction_color(dir),
            SPHERE_SCALE,
        ));
    }
}

/// Draw one coordinate axis as a bar of same-coloured splats from the origin out.
fn push_axis(out: &mut Vec<Gaussian>, axis: Vec3, color: U8Vec4) {
    for i in 0..AXIS_COUNT {
        let t = i as f32 / (AXIS_COUNT - 1) as f32;
        out.push(splat(axis * (t * AXIS_LENGTH), color, AXIS_SCALE));
    }
}

/// Map a unit direction to an RGB colour (the classic normal-as-colour palette).
fn direction_color(dir: Vec3) -> U8Vec4 {
    let rgb = (dir * 0.5 + Vec3::splat(0.5)) * 255.0;
    U8Vec4::new(rgb.x as u8, rgb.y as u8, rgb.z as u8, 255)
}

/// An isotropic (round) splat: no rotation, no view-dependent colour.
fn splat(pos: Vec3, color: U8Vec4, scale: f32) -> Gaussian {
    Gaussian {
        rot: Quat::IDENTITY,
        pos,
        color,
        sh: [Vec3::ZERO; 15],
        scale: Vec3::splat(scale),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use wgpu_3dgs_viewer::core::{GaussiansSource, PlyGaussians, WriteIterGaussian};

    use super::*;

    #[test]
    fn sample_splat_has_expected_count() {
        let gaussians = sample_splat();
        assert_eq!(gaussians.len(), SPHERE_COUNT + 3 * AXIS_COUNT);
    }

    #[test]
    fn bounds_of_sample_is_centred_and_sized() {
        let (center, radius) = bounds(&sample_splat());
        // Sphere is at the origin; axes reach AXIS_LENGTH along +X/+Y/+Z, so the
        // box centre sits a little off-origin and the radius is near AXIS_LENGTH.
        assert!(center.length() < 0.5, "roughly centred, got {center:?}");
        assert!(
            (0.5..2.5).contains(&radius),
            "plausible radius, got {radius}"
        );
    }

    /// The loader path leans on `Gaussians::read_from(.., Ply)`; prove it round-trips
    /// real PLY bytes produced by the same library. Set `DUMP_SAMPLE_PLY=<path>` to
    /// also emit a small `.ply` fixture for manual drag-and-drop testing.
    #[test]
    fn sample_splat_ply_roundtrips() {
        let ply: PlyGaussians = sample_splat().iter_gaussian().collect();
        let mut bytes = Vec::new();
        ply.write_to(&mut bytes).expect("write ply");

        let parsed =
            Gaussians::read_from(&mut Cursor::new(&bytes), GaussiansSource::Ply).expect("read ply");
        assert_eq!(parsed.len(), sample_splat().len());

        if let Ok(path) = std::env::var("DUMP_SAMPLE_PLY") {
            let small: PlyGaussians = sample_splat().iter_gaussian().take(400).collect();
            let mut file = std::fs::File::create(&path).expect("create dump file");
            small.write_to(&mut file).expect("write dump");
        }
    }

    #[test]
    fn sample_splat_values_are_finite_and_bounded() {
        let gaussians = sample_splat();
        for g in gaussians.iter_gaussian() {
            assert!(g.pos.is_finite(), "position must be finite");
            assert!(g.scale.cmpgt(Vec3::ZERO).all(), "scale must be positive");
            // Everything lives inside the volume the orbit camera frames.
            assert!(g.pos.length() <= AXIS_LENGTH + 1e-3, "splat within bounds");
        }
    }
}
