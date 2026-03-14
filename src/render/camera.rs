use glam::{Mat4, Vec3};

pub struct Camera {
    pub azimuth: f32,
    pub altitude: f32,
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
    pub fn view_matrix(&self) -> Mat4 {
        let eye = Vec3::new(
            self.distance * self.azimuth.cos() * self.altitude.cos(),
            self.distance * self.altitude.sin(),
            self.distance * self.azimuth.sin() * self.altitude.cos(),
        );

        Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y)
    }

    pub fn projection(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, aspect, 0.1, 100.0)
    }
}
