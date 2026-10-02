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

#[test]
fn every_required_json_field_rejects_missing_or_malformed_values() {
    let base: Value = serde_json::from_str(&ThemeSettings::default().encode()).unwrap();
    let fields = base
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    for key in fields {
        let mut missing = base.clone();
        missing.as_object_mut().unwrap().remove(&key);
        assert!(
            ThemeSettings::decode(&missing.to_string()).is_none(),
            "missing {key}"
        );
        let mut malformed = base.clone();
        malformed[&key] = json!({"unexpected": true});
        assert!(
            ThemeSettings::decode(&malformed.to_string()).is_none(),
            "malformed {key}"
        );
    }
    for invalid in [
        "{",
        "{\"version\":4,",
        "{\"format\":\"other\",\"version\":4}",
    ] {
        assert!(ThemeSettings::decode(invalid).is_none());
    }
    for key in ["accent", "secondary"] {
        for invalid in [
            json!([]),
            json!([1, 2]),
            json!([1, 2, 3, 4]),
            json!(["1", 2, 3]),
            json!([1, "2", 3]),
            json!([1, 2, "3"]),
            json!([256, 2, 3]),
            json!([1, 256, 3]),
            json!([1, 2, 256]),
        ] {
            let mut v = base.clone();
            v[key] = invalid;
            assert!(
                ThemeSettings::decode(&v.to_string()).is_none(),
                "invalid RGB {key}: {}",
                v[key]
            );
        }
    }
    for (key, invalid) in [
        ("position", json!("0.5")),
        ("color", json!([1, 2])),
        ("color", json!([1, 2, 999])),
    ] {
        let mut v = base.clone();
        v["stops"][2][key] = invalid;
        assert!(
            ThemeSettings::decode(&v.to_string()).is_none(),
            "invalid stop {key}"
        );
    }
    let mut v = base.clone();
    v["font"] = json!("missing-font");
    assert!(ThemeSettings::decode(&v.to_string()).is_none());
    for version in [0, 2, 5, u64::MAX] {
        let mut v = base.clone();
        v["version"] = json!(version);
        assert!(ThemeSettings::decode(&v.to_string()).is_none());
    }
}

#[test]
fn legacy_import_validates_each_field_and_retains_disabled_light_preferences() {
    let base = ["0", "1,2,3", "4,5,6", "0", "90", "0.3", "0.8", "8"];
    let theme = ThemeSettings::decode(&base.join(";")).unwrap();
    assert!(!theme.dark);
    assert!(!theme.gradient_enabled);
    assert_eq!(theme.accent, [1, 2, 3]);
    assert_eq!(theme.secondary, [4, 5, 6]);
    assert_eq!(theme.frost, 0.8);
    assert_eq!(theme.frost_light, 0.8);
    for (index, invalid) in [
        (0, "false"),
        (1, "bad,2,3"),
        (2, "4,bad,6"),
        (2, "4,5"),
        (3, "true"),
        (4, "bad"),
        (5, "NaN"),
        (6, "bad"),
        (7, "inf"),
    ] {
        let mut fields = base;
        fields[index] = invalid;
        assert!(
            ThemeSettings::decode(&fields.join(";")).is_none(),
            "legacy field {index}"
        );
    }
}
