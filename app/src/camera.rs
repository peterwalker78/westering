//! Where the view is looking, and the stereographic projection from the sky
//! onto the window. Horizon vectors are `[north, east, up]`.

use westering_core::coords::{Vec3, dot, from_alt_az, normalise};

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub az: f64,
    pub alt: f64,
    /// Horizontal field of view, degrees.
    pub fov: f64,
    pub width: f64,
    pub height: f64,
    forward: Vec3,
    right: Vec3,
    up: Vec3,
    k: f64,
}

pub const FOV_WIDEST: f64 = 120.0;
/// Close enough to fill the view with a planet's disc.
pub const FOV_NARROWEST: f64 = 0.01;

fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

impl Camera {
    pub fn new(az: f64, alt: f64, fov: f64) -> Camera {
        let mut c = Camera {
            az,
            alt,
            fov,
            width: 1.0,
            height: 1.0,
            forward: [1.0, 0.0, 0.0],
            right: [0.0, 1.0, 0.0],
            up: [0.0, 0.0, 1.0],
            k: 1.0,
        };
        c.update();
        c
    }

    pub fn set_size(&mut self, width: f64, height: f64) {
        self.width = width.max(1.0);
        self.height = height.max(1.0);
        self.update();
    }

    pub fn update(&mut self) {
        self.az = self.az.rem_euclid(360.0);
        self.alt = self.alt.clamp(-20.0, 89.5);
        self.fov = self.fov.clamp(FOV_NARROWEST, FOV_WIDEST);
        self.forward = from_alt_az(self.alt, self.az);
        self.right = normalise(cross([0.0, 0.0, 1.0], self.forward));
        self.up = cross(self.forward, self.right);
        self.k = (self.width / 2.0) / (2.0 * (self.fov.to_radians() / 4.0).tan());
    }

    /// Screen position of a horizon vector, if it is in front of the viewer.
    pub fn project(&self, v: Vec3) -> Option<(f64, f64)> {
        let z = dot(v, self.forward);
        if z < -0.3 {
            return None;
        }
        let s = 2.0 / (1.0 + z);
        let x = dot(v, self.right) * s;
        let y = dot(v, self.up) * s;
        Some((
            self.width / 2.0 + self.k * x,
            self.height / 2.0 - self.k * y,
        ))
    }

    /// The horizon vector under a screen position.
    pub fn unproject(&self, sx: f64, sy: f64) -> Vec3 {
        let x = (sx - self.width / 2.0) / self.k;
        let y = (self.height / 2.0 - sy) / self.k;
        let rho2 = x * x + y * y;
        let z = (4.0 - rho2) / (4.0 + rho2);
        let a = (1.0 + z) / 2.0;
        let (x, y) = (x * a, y * a);
        normalise([
            self.forward[0] * z + self.right[0] * x + self.up[0] * y,
            self.forward[1] * z + self.right[1] * x + self.up[1] * y,
            self.forward[2] * z + self.right[2] * x + self.up[2] * y,
        ])
    }

    /// Pixels per degree at the middle of the view.
    pub fn px_per_degree(&self) -> f64 {
        self.k * std::f64::consts::PI / 180.0
    }

    pub fn on_screen(&self, x: f64, y: f64, margin: f64) -> bool {
        x > -margin && y > -margin && x < self.width + margin && y < self.height + margin
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_round_trips_and_centres_the_view() {
        let mut c = Camera::new(123.0, 34.0, 90.0);
        c.set_size(1600.0, 900.0);
        let (x, y) = c.project(from_alt_az(34.0, 123.0)).unwrap();
        assert!((x - 800.0).abs() < 1e-6 && (y - 450.0).abs() < 1e-6);
        let v = from_alt_az(40.0, 140.0);
        let (x, y) = c.project(v).unwrap();
        let back = c.unproject(x, y);
        assert!((0..3).all(|i| (back[i] - v[i]).abs() < 1e-9));
        // Up on the sky is up on the screen, east is to the right when facing south.
        let mut south = Camera::new(180.0, 30.0, 90.0);
        south.set_size(1600.0, 900.0);
        let (_, higher) = south.project(from_alt_az(40.0, 180.0)).unwrap();
        assert!(higher < 450.0);
        let (to_east, _) = south.project(from_alt_az(30.0, 170.0)).unwrap();
        assert!(to_east < 800.0, "facing south, east is on the left");
    }
}
