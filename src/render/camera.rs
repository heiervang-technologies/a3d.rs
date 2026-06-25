//! Orbital perspective camera.
//!
//! Currently unused: the renderer projects orthographically. Kept as scaffolding
//! for the perspective-projection work planned in M3 (see the roadmap).
use glam::{Mat4, Vec3};

/// An orbital camera positioned by azimuth, altitude, and distance from the origin.
#[allow(dead_code)]
pub struct Camera {
    /// Horizontal orbit angle in radians.
    pub azimuth: f32,
    /// Vertical orbit angle in radians.
    pub altitude: f32,
    /// Distance from the origin (orbit radius).
    pub distance: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            azimuth: 0.0,
            altitude: 0.3,
            distance: 3.0,
        }
    }
}

#[allow(dead_code)]
impl Camera {
    /// The right-handed view matrix looking from the orbit position at the origin.
    pub fn view_matrix(&self) -> Mat4 {
        let eye = Vec3::new(
            self.distance * self.azimuth.cos() * self.altitude.cos(),
            self.distance * self.altitude.sin(),
            self.distance * self.azimuth.sin() * self.altitude.cos(),
        );

        Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y)
    }

    /// A right-handed perspective projection matrix for the given aspect ratio.
    pub fn projection(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, aspect, 0.1, 100.0)
    }
}
