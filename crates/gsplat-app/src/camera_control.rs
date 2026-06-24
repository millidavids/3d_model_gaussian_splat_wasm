//! Orbit controls: the viewer frames the splat and the user spins it.
//!
//! [`gs::Camera`] is a free-fly camera (position + pitch/yaw). We drive it as an
//! orbit camera by keeping the orbit state here — a `target`, a `distance`, and
//! two angles — and each frame placing the camera on that sphere looking inward.
//! Because the camera's own `pos`/`pitch`/`yaw` are public this needs no fork.

use glam::Vec3;
use wgpu_3dgs_viewer as gs;

/// Radians of orbit per pixel of drag.
const DRAG_SENSITIVITY: f32 = 0.006;
/// Fraction of distance changed per unit of scroll.
const ZOOM_SENSITIVITY: f32 = 0.1;
/// Gentle idle spin (radians/sec) so the page reads as 3D before any input.
const AUTO_SPIN_SPEED: f32 = 0.25;
/// Default zoom limits (the sample splat); [`OrbitCamera::frame`] overrides these
/// per loaded splat.
const MIN_DISTANCE: f32 = 1.5;
const MAX_DISTANCE: f32 = 20.0;
/// Camera distance as a multiple of a framed splat's radius (so it fits in view).
const FRAME_DISTANCE_FACTOR: f32 = 2.5;
/// Keep elevation just shy of the poles to avoid a degenerate look-direction.
const PITCH_LIMIT: f32 = std::f32::consts::FRAC_PI_2 - 1e-3;

/// Orbit state around a fixed target point.
#[derive(Debug, Clone)]
pub struct OrbitCamera {
    target: Vec3,
    distance: f32,
    /// Azimuth, radians.
    yaw: f32,
    /// Elevation, radians.
    pitch: f32,
    /// Zoom limits, kept relative to whatever splat is framed (see [`Self::frame`]).
    min_distance: f32,
    max_distance: f32,
}

impl OrbitCamera {
    /// Frame a unit-ish object at the origin from a slightly raised three-quarter view.
    pub fn new() -> Self {
        Self {
            target: Vec3::ZERO,
            distance: 4.0,
            yaw: 0.0,
            pitch: 0.35,
            min_distance: MIN_DISTANCE,
            max_distance: MAX_DISTANCE,
        }
    }

    /// Re-frame to fit a splat of the given centre and radius, keeping the
    /// current view angles. Distance and zoom limits scale with the radius so
    /// any splat — unit sphere or room-sized scan — sits sensibly in view.
    pub fn frame(&mut self, center: Vec3, radius: f32) {
        let radius = radius.max(1e-3);
        self.target = center;
        self.distance = radius * FRAME_DISTANCE_FACTOR;
        self.min_distance = radius * 0.4;
        self.max_distance = radius * 12.0;
    }

    /// Rotate the orbit by a mouse drag of `(dx, dy)` pixels.
    pub fn orbit(&mut self, dx: f32, dy: f32) {
        self.yaw -= dx * DRAG_SENSITIVITY;
        self.pitch = (self.pitch + dy * DRAG_SENSITIVITY).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// Dolly in/out by a scroll amount (positive scrolls in).
    pub fn zoom(&mut self, scroll: f32) {
        self.distance = (self.distance * (1.0 - scroll * ZOOM_SENSITIVITY))
            .clamp(self.min_distance, self.max_distance);
    }

    /// Advance the idle auto-spin by `delta_time` seconds.
    pub fn advance(&mut self, delta_time: f32) {
        self.yaw -= AUTO_SPIN_SPEED * delta_time;
    }

    /// Position `camera` on the orbit sphere, looking at the target.
    pub fn apply_to(&self, camera: &mut gs::Camera) {
        let forward = forward_from_angles(self.pitch, self.yaw);
        camera.pitch = self.pitch;
        camera.yaw = self.yaw;
        camera.pos = self.target - forward * self.distance;
    }
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self::new()
    }
}

/// Forward unit vector for a pitch/yaw, matching [`gs::Camera::get_forward`].
fn forward_from_angles(pitch: f32, yaw: f32) -> Vec3 {
    Vec3::new(
        pitch.cos() * yaw.sin(),
        pitch.sin(),
        pitch.cos() * yaw.cos(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_sits_at_distance_from_target_looking_in() {
        let orbit = OrbitCamera::new();
        let mut camera = gs::Camera::new(0.1..100.0, 1.0);
        orbit.apply_to(&mut camera);

        // On the orbit sphere...
        let to_target = orbit.target - camera.pos;
        assert!((to_target.length() - orbit.distance).abs() < 1e-4);
        // ...and the camera's forward points at the target.
        let forward = forward_from_angles(camera.pitch, camera.yaw);
        assert!(forward.dot(to_target.normalize()) > 0.999);
    }

    #[test]
    fn zoom_is_clamped() {
        let mut orbit = OrbitCamera::new();
        for _ in 0..1000 {
            orbit.zoom(1.0);
        }
        assert!(orbit.distance >= MIN_DISTANCE);
        for _ in 0..1000 {
            orbit.zoom(-1.0);
        }
        assert!(orbit.distance <= MAX_DISTANCE);
    }

    #[test]
    fn frame_places_camera_at_radius_scaled_distance() {
        let mut orbit = OrbitCamera::new();
        let center = Vec3::new(5.0, -2.0, 1.0);
        orbit.frame(center, 3.0);

        let mut camera = gs::Camera::new(0.1..1e4, 1.0);
        orbit.apply_to(&mut camera);

        let to_target = center - camera.pos;
        assert!((to_target.length() - 3.0 * FRAME_DISTANCE_FACTOR).abs() < 1e-3);
        // Zoom limits now track the framed radius.
        for _ in 0..1000 {
            orbit.zoom(1.0);
        }
        assert!(orbit.distance >= 3.0 * 0.4 - 1e-3);
    }

    #[test]
    fn orbit_pitch_is_clamped_to_poles() {
        let mut orbit = OrbitCamera::new();
        orbit.orbit(0.0, 1e6);
        assert!(orbit.pitch <= PITCH_LIMIT);
        orbit.orbit(0.0, -1e6);
        assert!(orbit.pitch >= -PITCH_LIMIT);
    }
}
