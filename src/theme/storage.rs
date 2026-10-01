use super::*;
use serde_json::{json, Value};

pub const MAX_BYTES: usize = 16 * 1024;

impl ThemeSettings {
    pub fn encode(self) -> String {
        let s = self.normalized();
        json!({
            "format": "trontop-theme", "version": 4, "dark": s.dark,
            "accent": s.accent, "secondary": s.secondary,
            "stops": s.stops.map(|p| json!({"position": p.position, "color": p.color})),
            "gradient_enabled": s.gradient_enabled, "gradient_angle": s.gradient_angle,
            "gradient_strength": s.gradient_strength, "panel_opacity": s.frost,
            "frost_light": s.frost_light, "surface_tint": s.surface_tint,
            "text_strength": s.text_strength, "font": s.font.key(),
            "roundness": s.roundness, "zebra_strength": s.zebra_strength,
            "column_strength": s.column_strength, "hover_strength": s.hover_strength,
            "high_contrast": s.high_contrast,
        })
        .to_string()
    }

    pub fn decode(value: &str) -> Option<Self> {
        if value.len() > MAX_BYTES {
            return None;
        }
        let value = value.trim();
        if !value.starts_with('{') {
            return legacy(value);
        }
        let v: Value = serde_json::from_str(value).ok()?;
        let version = v["version"].as_u64()?;
        if v["format"].as_str()? != "trontop-theme" || !matches!(version, 3 | 4) {
            return None;
        }
        let rgb = |v: &Value| -> Option<[u8; 3]> {
            let a = v.as_array()?;
            if a.len() != 3 {
                return None;
            }
            Some([
                u8::try_from(a[0].as_u64()?).ok()?,
                u8::try_from(a[1].as_u64()?).ok()?,
                u8::try_from(a[2].as_u64()?).ok()?,
            ])
        };
        let number = |v: &Value| -> Option<f32> {
            let n = v.as_f64()? as f32;
            n.is_finite().then_some(n)
        };
        let stops = v["stops"].as_array()?;
        if stops.len() != 4 {
            return None;
        }
        let mut pegs = Self::default().stops;
        for (peg, value) in pegs.iter_mut().zip(stops) {
            *peg = Stop {
                position: number(&value["position"])?,
                color: rgb(&value["color"])?,
            };
        }
        let frost = number(&v["panel_opacity"])?;
        Some(
            Self {
                dark: v["dark"].as_bool()?,
                accent: rgb(&v["accent"])?,
                secondary: rgb(&v["secondary"])?,
                stops: pegs,
                gradient_enabled: v["gradient_enabled"].as_bool()?,
                gradient_angle: number(&v["gradient_angle"])?,
                gradient_strength: number(&v["gradient_strength"])?,
                frost,
                frost_light: if version == 3 {
                    frost
                } else {
                    number(&v["frost_light"])?
                },
                surface_tint: if version == 3 {
                    0.08
                } else {
                    number(&v["surface_tint"])?
                },
                text_strength: if version == 3 {
                    0.0
                } else {
                    number(&v["text_strength"])?
                },
                font: if version == 3 {
                    typography::FontChoice::Sans
                } else {
                    typography::FontChoice::from_key(v["font"].as_str()?)?
                },
                roundness: number(&v["roundness"])?,
                zebra_strength: number(&v["zebra_strength"])?,
                column_strength: number(&v["column_strength"])?,
                hover_strength: number(&v["hover_strength"])?,
                high_contrast: v["high_contrast"].as_bool()?,
            }
            .normalized(),
        )
    }
}

fn legacy(value: &str) -> Option<ThemeSettings> {
    let fields: Vec<_> = value.split(';').collect();
    if fields.len() != 8 {
        return None;
    }
    let boolean = |s| match s {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    };
    let rgb = |s: &str| -> Option<[u8; 3]> {
        s.split(',')
            .map(str::parse)
            .collect::<Result<Vec<u8>, _>>()
            .ok()?
            .try_into()
            .ok()
    };
    let number = |s: &str| -> Option<f32> {
        let n: f32 = s.parse().ok()?;
        n.is_finite().then_some(n)
    };
    let accent = rgb(fields[1])?;
    let secondary = rgb(fields[2])?;
    let frost = number(fields[6])?;
    let stops = std::array::from_fn(|i| {
        let c = mix(super::rgb(accent), super::rgb(secondary), i as f32 / 3.0);
        Stop {
            position: i as f32 / 3.0,
            color: [c.r(), c.g(), c.b()],
        }
    });
    Some(
        ThemeSettings {
            dark: boolean(fields[0])?,
            accent,
            secondary,
            stops,
            gradient_enabled: boolean(fields[3])?,
            gradient_angle: number(fields[4])?,
            gradient_strength: number(fields[5])?,
            frost,
            frost_light: frost,
            roundness: number(fields[7])?,
            ..Default::default()
        }
        .normalized(),
    )
}

#[cfg(test)]
#[path = "tests/storage.rs"]
mod tests;
