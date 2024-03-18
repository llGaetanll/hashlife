use sdl2::pixels::Color;

pub trait ColorInterpolationExt<T> {
    fn lerp(&self, other: &T, r: f64) -> T;
}

impl ColorInterpolationExt<Color> for Color {
    fn lerp(&self, other: &Color, p: f64) -> Color {
        assert!((0f64..=1f64).contains(&p), "lerp p lives in [0, 1]");

        // interpolate a channel
        let f = |a: u8, b: u8| {
            ((a as f64) * p + (b as f64) * (1f64 - p)).round() as u8
        };

        Color {
            r: f(self.r, other.r),
            g: f(self.g, other.g),
            b: f(self.b, other.b),
            a: f(self.a, other.a),
        }
    }
}
