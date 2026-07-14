//! Orbital perspective camera.
//!
//! Currently unused: the renderer projects orthographically. Kept as scaffolding
//! for the perspective-projection work planned in M3 (see the roadmap).
use glam::{Mat4, Vec3, camera};

/// An orbital camera positioned by azimuth, altitude, and distance from the origin.
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

impl Camera {
    /// The right-handed view matrix looking from the orbit position at the origin.
    pub fn view_matrix(&self) -> Mat4 {
        let eye = Vec3::new(
            self.distance * self.azimuth.cos() * self.altitude.cos(),
            self.distance * self.altitude.sin(),
            self.distance * self.azimuth.sin() * self.altitude.cos(),
        );

        camera::rh::view::look_at_mat4(eye, Vec3::ZERO, Vec3::Y)
    }

    /// A right-handed perspective projection matrix for the given aspect ratio.
    pub fn projection(&self, aspect: f32) -> Mat4 {
        camera::rh::proj::directx::perspective(std::f32::consts::FRAC_PI_4, aspect, 0.1, 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_camera_matrices_are_finite() {
        let camera = Camera::default();
        assert!(camera.view_matrix().is_finite());
        assert!(camera.projection(16.0 / 9.0).is_finite());
    }
}
