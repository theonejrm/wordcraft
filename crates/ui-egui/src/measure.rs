//! Lengths and numbers as the user reads and types them: in the chosen unit (centimetres in
//! Germany, inches in the US; File ▸ Options ▸ Measurement units) and with the region's decimal
//! separator (`2,5 cm`, `10,5 pt`). The document and every command keep points.

use egui::{Response, Ui};
use wordcraft_engine::region;
use wordcraft_geom::{Unit, parse_measure};

use crate::WordApp;

/// The `units` setting that follows the region.
pub const AUTO: &str = "auto";

/// The `units` setting values and their units (besides [`AUTO`]).
pub const UNITS: [(&str, Unit); 4] = [("cm", Unit::Centimeters), ("mm", Unit::Millimeters), ("in", Unit::Inches), ("pt", Unit::Points)];

/// Canonical `units` setting for user input, `None` when unknown.
pub fn normalize_pref(v: &str) -> Option<&'static str> {
    let v = v.trim().to_ascii_lowercase();
    if v == AUTO {
        return Some(AUTO);
    }
    let v = match v.as_str() {
        "inch" | "inches" | "\"" => "in",
        "centimeters" | "centimetres" => "cm",
        "millimeters" | "millimetres" => "mm",
        "points" | "point" => "pt",
        other => other,
    };
    UNITS.iter().find(|(k, _)| *k == v).map(|(k, _)| *k)
}

/// How lengths and numbers are shown: a unit and a locale for separators.
#[derive(Clone, Debug, PartialEq)]
pub struct Measure {
    pub unit: Unit,
    pub locale: String,
}

impl Measure {
    /// The app's: the `units` setting (or the region's unit) and the session's locale.
    pub fn of(app: &WordApp) -> Measure {
        let locale = app.session.locale.clone();
        let unit = UNITS.iter().find(|(k, _)| *k == app.ui.units).map(|(_, u)| *u).unwrap_or_else(|| region::unit(&locale));
        Measure { unit, locale }
    }

    fn decimals(&self) -> usize {
        match self.unit {
            Unit::Millimeters | Unit::Points => 1,
            _ => 2,
        }
    }

    /// A number with the region's decimal separator: `1,15`.
    pub fn number(&self, v: f64, decimals: usize) -> String {
        region::decimal(v, decimals, &self.locale)
    }

    /// A count with the region's grouping: `1.234`.
    pub fn count(&self, n: u64) -> String {
        region::count(n, &self.locale)
    }

    /// A length given in points, in the display unit: `2,5 cm`, `1"`.
    pub fn length(&self, pt: f32) -> String {
        let v = f64::from(pt) / f64::from(self.unit.pt_per_unit());
        format!("{}{}", self.number(v, self.decimals()), self.unit.suffix())
    }

    /// Parse a typed length (`2,5`, `2,5 cm`, `1"`, `12 pt`): a bare number is in the display
    /// unit. Points.
    pub fn parse_length(&self, s: &str) -> Option<f32> {
        let t = s.trim();
        // `1.234,5 cm` → `1234.5 cm` in comma regions, before the unit parser sees it.
        let split = t.find(|c: char| c.is_alphabetic() || c == '"').unwrap_or(t.len());
        let (num, unit) = t.split_at(split);
        let n = region::parse_decimal(num, &self.locale)?;
        parse_measure(&format!("{n}{unit}"), self.unit)
    }

    /// A drag-or-type field for a length held in points.
    pub fn length_field(&self, ui: &mut Ui, pt: &mut f32, min_pt: f32, max_pt: f32) -> Response {
        let k = self.unit.pt_per_unit();
        let mut v = f64::from(*pt / k);
        let speed = match self.unit {
            Unit::Inches => 0.05,
            Unit::Centimeters => 0.1,
            _ => 1.0,
        };
        let me = self.clone();
        let parser = self.clone();
        let r = ui.add(
            egui::DragValue::new(&mut v)
                .speed(speed)
                .range(f64::from(min_pt / k)..=f64::from(max_pt / k))
                .custom_formatter(move |n, _| me.number(n, me.decimals()))
                .custom_parser(move |s| parser.parse_length(s).map(|p| f64::from(p / parser.unit.pt_per_unit())))
                .suffix(self.unit.suffix()),
        );
        if r.changed() {
            *pt = (v as f32 * k).clamp(min_pt, max_pt);
        }
        r
    }

    /// A drag-or-type field for a plain number with the region's decimal separator.
    pub fn number_field(
        &self,
        ui: &mut Ui,
        v: &mut f32,
        range: std::ops::RangeInclusive<f32>,
        speed: f64,
        decimals: usize,
        suffix: &str,
    ) -> Response {
        let fmt = self.clone();
        let parse = self.clone();
        ui.add(
            egui::DragValue::new(v)
                .speed(speed)
                .range(range)
                .custom_formatter(move |n, _| fmt.number(n, decimals))
                .custom_parser(move |s| region::parse_decimal(s.trim().trim_end_matches(char::is_alphabetic).trim(), &parse.locale))
                .suffix(suffix),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn de() -> Measure {
        Measure { unit: Unit::Centimeters, locale: "de-DE".into() }
    }

    #[test]
    fn german_lengths_read_and_parse_in_centimetres() {
        let m = de();
        assert_eq!(m.length(70.866_14), "2,5 cm");
        assert_eq!(m.length(72.0), "2,54 cm");
        assert_eq!(m.length(0.0), "0 cm");
        let close = |a: Option<f32>, b: f32| a.is_some_and(|a| (a - b).abs() < 0.01);
        assert!(close(m.parse_length("2,5"), 70.866));
        assert!(close(m.parse_length("2,5 cm"), 70.866));
        assert!(close(m.parse_length("1\""), 72.0));
        assert!(close(m.parse_length("12 pt"), 12.0));
        assert!(close(m.parse_length("1.234,5 mm"), 1234.5 * 72.0 / 25.4));
        assert_eq!(m.parse_length("abc"), None);
        assert_eq!(m.number(1.15, 2), "1,15");
        assert_eq!(m.count(12345), "12.345");
    }

    #[test]
    fn us_lengths_read_in_inches() {
        let m = Measure { unit: Unit::Inches, locale: "en-US".into() };
        assert_eq!(m.length(72.0), "1\"");
        assert_eq!(m.length(54.0), "0.75\"");
        assert!(m.parse_length("1.5").is_some_and(|p| (p - 108.0).abs() < 0.01));
        assert_eq!(m.count(12345), "12,345");
    }

    #[test]
    fn unit_settings() {
        assert_eq!(normalize_pref("AUTO"), Some(AUTO));
        assert_eq!(normalize_pref("Centimeters"), Some("cm"));
        assert_eq!(normalize_pref("in"), Some("in"));
        assert_eq!(normalize_pref("furlong"), None);
    }

    #[test]
    fn the_units_setting_follows_the_region_and_persists() {
        let mut app = crate::WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::new()), Default::default());
        assert_eq!(app.ui.units, AUTO);
        assert_eq!(Measure::of(&app).unit, Unit::Inches, "tests run as en-US");
        app.session.locale = "de-DE".into();
        assert_eq!(Measure::of(&app).unit, Unit::Centimeters);
        let r = app.run("ui.units", serde_json::json!({"value": "Inches"})).unwrap();
        assert_eq!(r["units"], "in");
        assert_eq!(Measure::of(&app).unit, Unit::Inches);
        assert!(app.run("ui.units", serde_json::json!({"value": "parsec"})).is_err());
        assert_eq!(app.ui.units, "in", "an unknown unit keeps the setting");
        let restored: crate::UiState = serde_json::from_str(&serde_json::to_string(&app.ui).unwrap()).unwrap();
        assert_eq!(restored.units, "in");
        let legacy: crate::UiState = serde_json::from_str(r#"{"tab":"Home"}"#).unwrap();
        assert_eq!(legacy.units, AUTO);
    }
}
