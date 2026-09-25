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
        Some(
            Self {
                dark: v["dark"].as_bool()?,
                accent: rgb(&v["accent"])?,
                secondary: rgb(&v["secondary"])?,
                stops: pegs,
                gradient_enabled: v["gradient_enabled"].as_bool()?,
                gradient_angle: number(&v["gradient_angle"])?,
                gradient_strength: number(&v["gradient_strength"])?,
                frost: number(&v["panel_opacity"])?,
                frost_light: if version == 3 {
                    number(&v["panel_opacity"])?
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
            frost: number(fields[6])?,
            frost_light: number(fields[6])?,
            roundness: number(fields[7])?,
            ..Default::default()
        }
        .normalized(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extended_controls_roundtrip_and_v3_migrates_without_losing_opacity() {
        let s = ThemeSettings {
            frost: 0.0,
            frost_light: 0.35,
            gradient_strength: 1.0,
            surface_tint: 0.9,
            text_strength: 0.7,
            font: typography::FontChoice::RajdhaniBold,
            ..Default::default()
        };
        assert_eq!(ThemeSettings::decode(&s.encode()), Some(s));
        let mut old: Value = serde_json::from_str(&s.encode()).unwrap();
        old["version"] = json!(3);
        old["panel_opacity"] = json!(0.62);
        for field in ["frost_light", "surface_tint", "text_strength", "font"] {
            old.as_object_mut().unwrap().remove(field);
        }
        let migrated = ThemeSettings::decode(&old.to_string()).unwrap();
        assert_eq!(migrated.frost, 0.62);
        assert_eq!(migrated.frost_light, 0.62);
        assert_eq!(migrated.font, typography::FontChoice::Sans);
        assert_eq!(ThemeSettings::decode(&migrated.encode()), Some(migrated));
        old["version"] = json!(4);
        assert!(
            ThemeSettings::decode(&old.to_string()).is_none(),
            "incomplete v4 must not silently reset new preferences"
        );
    }

    #[test]
    fn presets_roundtrip_and_legacy_retains_original_ramp() {
        for (_, s) in ThemeSettings::presets() {
            assert_eq!(ThemeSettings::decode(&s.encode()), Some(s));
        }
        let s = ThemeSettings::decode("1;168,85,247;46,230,215;1;132;0.34;0.8;8").unwrap();
        for i in 0..101 {
            let p = i as f32 / 100.0;
            let a = s.gradient_color(p).to_array();
            let b = mix(rgb(s.accent), rgb(s.secondary), p).to_array();
            for (a, b) in a.into_iter().zip(b) {
                assert!((a as i16 - b as i16).abs() <= 1);
            }
        }
        assert_eq!(ThemeSettings::decode(&s.encode()), Some(s));
    }

    #[test]
    fn bad_imports_are_bounded_and_rejected() {
        for value in [
            "x;1,2,3;4,5,6;1;90;0.3;0.8;8",
            "1;1,2,3,4;4,5,6;1;90;0.3;0.8;8",
            "1;1,2,3;4,5,6;1;NaN;0.3;0.8;8",
            "1;1,2,3;4,5,6;1;90;inf;0.8;8",
            "{}",
        ] {
            assert!(ThemeSettings::decode(value).is_none());
        }
        assert!(ThemeSettings::decode(&" ".repeat(MAX_BYTES + 1)).is_none());
        let base: Value = serde_json::from_str(&ThemeSettings::default().encode()).unwrap();
        for (key, value) in [
            ("version", json!(5)),
            ("stops", json!([])),
            ("roundness", json!(1e100)),
            ("accent", json!([256, 0, 0])),
            ("dark", json!("true")),
        ] {
            let mut v = base.clone();
            v[key] = value;
            assert!(ThemeSettings::decode(&v.to_string()).is_none(), "{key}");
        }
    }
}
