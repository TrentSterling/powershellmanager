//! Small native adaptation of TrontColors/Boxel's ColorMagic palette families.
//! Reference: boxel/crates/boxel/src/color.rs (owner-authored color utilities).
//! Palette generation is separate from UI surface/ink contrast correction.
use super::ThemeSettings;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Flavor {
    #[default]
    Auto,
    Pastel,
    Neon,
    Vintage,
    Dark,
    Earthy,
    Jewel,
}

impl Flavor {
    pub const ALL: [Self; 7] = [
        Self::Auto,
        Self::Pastel,
        Self::Neon,
        Self::Vintage,
        Self::Dark,
        Self::Earthy,
        Self::Jewel,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Surprise me",
            Self::Pastel => "Pastel",
            Self::Neon => "Neon",
            Self::Vintage => "Vintage",
            Self::Dark => "Dark",
            Self::Earthy => "Earthy",
            Self::Jewel => "Jewel",
        }
    }
    fn band(self) -> (i32, i32, i32, i32) {
        match self {
            Self::Pastel => (40, 80, 70, 90),
            Self::Neon => (85, 100, 50, 65),
            Self::Vintage => (20, 50, 40, 75),
            Self::Dark => (10, 60, 5, 25),
            Self::Earthy => (25, 55, 25, 55),
            Self::Jewel | Self::Auto => (60, 90, 25, 45),
        }
    }
}

/// Undo only what a roll changes, not later edits to layout or peg positions.
#[derive(Clone, Copy)]
pub(crate) struct Palette {
    accent: [u8; 3],
    secondary: [u8; 3],
    colors: [[u8; 3]; 4],
    angle: f32,
}
impl Palette {
    pub fn capture(s: &ThemeSettings) -> Self {
        Self {
            accent: s.accent,
            secondary: s.secondary,
            colors: s.stops.map(|stop| stop.color),
            angle: s.gradient_angle,
        }
    }
    pub fn apply(self, s: &mut ThemeSettings) {
        s.accent = self.accent;
        s.secondary = self.secondary;
        s.gradient_angle = self.angle;
        for (stop, color) in s.stops.iter_mut().zip(self.colors) {
            stop.color = color;
        }
    }
}

// SplitMix64 keeps adjacent seeds distinct and avoids the zero-state trap.
// This is a palette PRNG, never a source of security-sensitive randomness.
struct Rng(u64);
impl Rng {
    fn range(&mut self, low: i32, high: i32) -> i32 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^= z >> 31;
        low + (z % (high - low + 1) as u64) as i32
    }
}

fn hsl(h: f32, s: f32, l: f32) -> [u8; 3] {
    let l = l / 100.0;
    let a = (s / 100.0) * l.min(1.0 - l);
    [0.0, 8.0, 4.0].map(|n| {
        let k = (n + h / 30.0).rem_euclid(12.0);
        ((l - a * (-1.0_f32).max((k - 3.0).min((9.0 - k).min(1.0)))) * 255.0).round() as u8
    })
}

pub(crate) fn randomize(s: &mut ThemeSettings, seed: u64, flavor: Flavor) -> Flavor {
    let mut rng = Rng(seed);
    let flavor = if flavor == Flavor::Auto {
        Flavor::ALL[rng.range(1, 6) as usize]
    } else {
        flavor
    };
    let (smin, smax, lmin, lmax) = flavor.band();
    let base = rng.range(0, 359) as f32;
    // Ordered related hues, not independent RGB noise. Mostly analogous arcs,
    // with an occasional split-complementary turn for a two-accent theme.
    let offsets = match rng.range(0, 3) {
        0 => [0.0, 18.0, 42.0, 65.0],
        1 => [0.0, -22.0, -48.0, -80.0],
        2 => [0.0, 28.0, 145.0, 170.0],
        _ => [0.0, 40.0, 80.0, 120.0],
    };
    let saturation = rng.range(smin, smax) as f32;
    let lightness = rng.range(lmin, lmax) as f32;
    for (i, stop) in s.stops.iter_mut().enumerate() {
        stop.color = hsl(
            base + offsets[i],
            (saturation + rng.range(-7, 7) as f32).clamp(smin as f32, smax as f32),
            (lightness + [0.0, -9.0, 6.0, -3.0][i]).clamp(lmin as f32, lmax as f32),
        );
    }
    // Dark-family gradients still need visible accents. Light themes use a
    // deeper accent; raw swatches stay intact and ink is contrast-corrected.
    let accent_lightness = if s.dark {
        lightness.clamp(55.0, 78.0)
    } else {
        lightness.clamp(32.0, 50.0)
    };
    s.accent = hsl(base, saturation.max(45.0), accent_lightness);
    s.secondary = hsl(base + offsets[3], saturation.max(45.0), accent_lightness);
    s.gradient_angle = rng.range(0, 359) as f32;
    flavor
}

#[cfg(test)]
#[path = "tests/magic.rs"]
mod tests;
