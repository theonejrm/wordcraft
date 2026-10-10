//! Regional conventions besides dates ([`crate::dates`]): paper and margins for new documents,
//! measurement units, number separators and typographic quotes. Keyed by a locale tag such as
//! `de-DE` ([`crate::Session::locale`]).

use wordcraft_doc::Document;
use wordcraft_geom::{PAPER_SIZES, PT_PER_CM, Unit};

/// `de_DE.UTF-8` → (`de`, `de`), `en-GB` → (`en`, `gb`), `en` → (`en`, ``).
pub fn split(locale: &str) -> (String, String) {
    let base = locale.split(['.', '@']).next().unwrap_or("").replace('_', "-").to_ascii_lowercase();
    let mut parts = base.split('-').filter(|p| !p.is_empty());
    let lang = parts.next().unwrap_or("").to_string();
    let region = parts.find(|p| p.len() == 2 || (p.len() == 3 && p.chars().all(|c| c.is_ascii_digit()))).unwrap_or("").to_string();
    (lang, region)
}

/// The language part of a locale (`de` for `de-AT`).
pub fn language(locale: &str) -> String {
    split(locale).0
}

/// Regions where US Letter is the usual paper. A bare `en` (and an unknown locale) means `en-US`.
fn letter_region(locale: &str) -> bool {
    let (lang, region) = split(locale);
    match region.as_str() {
        "" => lang.is_empty() || lang == "en",
        r => matches!(r, "us" | "ca" | "mx" | "ph" | "cl" | "co" | "ve" | "gt" | "cr" | "pr" | "do" | "pa" | "sv" | "ni" | "hn"),
    }
}

/// Paper for new documents: `Letter` in the Americas' Letter regions, `A4` elsewhere.
pub fn paper(locale: &str) -> &'static str {
    if letter_region(locale) { "Letter" } else { "A4" }
}

/// Margins for new documents, in points: 1" on Letter, 2.5 cm on A4.
pub fn margin(locale: &str) -> f32 {
    if letter_region(locale) { 72.0 } else { 2.5 * PT_PER_CM }
}

/// The unit measurements are shown in: inches in the US (and a few others), centimetres elsewhere.
pub fn unit(locale: &str) -> Unit {
    let (lang, region) = split(locale);
    let inches = match region.as_str() {
        "" => lang.is_empty() || lang == "en",
        r => matches!(r, "us" | "lr" | "mm" | "pr"),
    };
    if inches { Unit::Inches } else { Unit::Centimeters }
}

/// Decimal and grouping separators: (`,`, `.`) in Germany, (`.`, `,`) in the US.
pub fn separators(locale: &str) -> (char, char) {
    let (lang, region) = split(locale);
    match (lang.as_str(), region.as_str()) {
        ("de" | "it" | "fr" | "rm", "ch" | "li") => ('.', '’'),
        ("de", "at") => (',', '\u{a0}'),
        ("de" | "nl" | "it" | "es" | "pt" | "da" | "id" | "tr" | "el" | "ro" | "hr" | "sl" | "sr" | "is" | "ca", _) => (',', '.'),
        ("fr" | "pl" | "cs" | "sk" | "ru" | "uk" | "be" | "bg" | "sv" | "nb" | "nn" | "no" | "fi" | "et" | "lv" | "lt" | "hu", _) => (',', '\u{a0}'),
        _ => ('.', ','),
    }
}

/// `v` with at most `decimals` decimals (trailing zeros dropped) and the region's decimal
/// separator: `10,5` for `de-DE`. No grouping (for values in fields people edit).
pub fn decimal(v: f64, decimals: usize, locale: &str) -> String {
    let v = if v.is_finite() { v } else { 0.0 };
    let s = format!("{v:.decimals$}");
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    let s = if s == "-0" { "0".to_string() } else { s };
    let (dec, _) = separators(locale);
    if dec == '.' { s } else { s.replace('.', &dec.to_string()) }
}

/// A count with the region's grouping: `1.234` for `de-DE`, `1,234` for `en-US`.
pub fn count(n: u64, locale: &str) -> String {
    let digits = n.to_string();
    let (_, group) = separators(locale);
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(group);
        }
        out.push(c);
    }
    out
}

/// Parse a number typed in the region's style: `10,5` (or `10.5`) for `de-DE`. Grouping
/// separators are ignored when the decimal one is unambiguous (`1.234,5`).
pub fn parse_decimal(s: &str, locale: &str) -> Option<f64> {
    let t: String = s.trim().chars().filter(|c| !c.is_whitespace() && *c != '\u{a0}' && *c != '’' && *c != '\'').collect();
    if t.is_empty() || t.len() > 64 {
        return None;
    }
    let (dec, group) = separators(locale);
    let normalized = if dec == ',' { if t.contains(',') { t.replace(group, "").replace(',', ".") } else { t } } else { t.replace(',', "") };
    normalized.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Typographic quotes: (open double, close double, open single, close single).
pub fn quotes(locale: &str) -> (char, char, char, char) {
    let (lang, region) = split(locale);
    match (lang.as_str(), region.as_str()) {
        ("de" | "it" | "fr" | "rm", "ch" | "li") => ('«', '»', '‹', '›'),
        ("de" | "cs" | "sk" | "sl" | "hr" | "lt" | "et" | "bg" | "is" | "ka", _) => ('„', '“', '‚', '‘'),
        ("pl" | "ro" | "hu", _) => ('„', '”', '‚', '’'),
        ("fr" | "it" | "es" | "ru" | "uk" | "be" | "nb" | "nn" | "no" | "el" | "ca", _) => ('«', '»', '‹', '›'),
        ("sv" | "fi", _) => ('”', '”', '’', '’'),
        _ => ('“', '”', '‘', '’'),
    }
}

/// Make a freshly created document regional: the region's paper and margins (orientation kept)
/// in every section, and `locale` as the document's default language (spelling, hyphenation,
/// quotes). Documents opened from files are never changed.
pub fn localize_new(doc: &mut Document, locale: &str) {
    let name = paper(locale);
    let Some((_, w, h)) = PAPER_SIZES.iter().find(|(n, _, _)| *n == name) else { return };
    let m = margin(locale);
    let adjust = |s: &mut wordcraft_doc::SectionProps| {
        let (pw, ph) = if s.landscape { (w.max(*h), w.min(*h)) } else { (w.min(*h), w.max(*h)) };
        s.page_w = pw;
        s.page_h = ph;
        s.margin_top = m;
        s.margin_bottom = m;
        s.margin_left = m;
        s.margin_right = m;
    };
    let ends: Vec<usize> = doc.sections().iter().map(|(e, _)| *e).collect();
    for e in ends {
        adjust(doc.section_mut(e));
    }
    if let Some(tag) = lang_tag(locale) {
        doc.styles.default_chr.lang = Some(tag);
    }
}

/// `de_DE.UTF-8` → `de-DE`, the form documents store (`w:lang`).
pub fn lang_tag(locale: &str) -> Option<String> {
    let (lang, region) = split(locale);
    if lang.is_empty() || lang.len() > 8 || !lang.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    Some(if region.is_empty() { lang } else { format!("{lang}-{}", region.to_ascii_uppercase()) })
}

/// Text width of the first section (page minus margins), for templates that set tab stops.
pub fn text_width(doc: &Document) -> f32 {
    let s = doc.sections().first().map(|(_, s)| (*s).clone()).unwrap_or_default();
    (s.page_w - s.margin_left - s.margin_right - s.gutter).max(72.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn german_conventions() {
        for l in ["de-DE", "de_DE.UTF-8", "de-AT", "de"] {
            assert_eq!(paper(l), "A4", "{l}");
            assert_eq!(unit(l), Unit::Centimeters, "{l}");
            assert!((margin(l) - 70.866).abs() < 0.01, "{l}");
            assert_eq!(quotes(l).0, '„', "{l}");
        }
        assert_eq!(decimal(10.5, 2, "de-DE"), "10,5");
        assert_eq!(decimal(2.0, 2, "de-DE"), "2");
        assert_eq!(decimal(-0.001, 2, "de-DE"), "0");
        assert_eq!(decimal(f64::NAN, 2, "de-DE"), "0");
        assert_eq!(count(1234567, "de-DE"), "1.234.567");
        assert_eq!(count(999, "de-DE"), "999");
        assert_eq!(parse_decimal("10,5", "de-DE"), Some(10.5));
        assert_eq!(parse_decimal("10.5", "de-DE"), Some(10.5));
        assert_eq!(parse_decimal("1.234,5", "de-DE"), Some(1234.5));
        assert_eq!(parse_decimal("abc", "de-DE"), None);
        assert_eq!(parse_decimal("inf", "de-DE"), None);
        assert_eq!(lang_tag("de_DE.UTF-8").as_deref(), Some("de-DE"));
        // Switzerland: decimal point, guillemets.
        assert_eq!(decimal(10.5, 1, "de-CH"), "10.5");
        assert_eq!(count(1234, "de-CH"), "1’234");
        assert_eq!(quotes("de-CH").0, '«');
    }

    #[test]
    fn us_and_other_conventions() {
        for l in ["en-US", "en", "", "es-MX", "fr-CA"] {
            assert_eq!(paper(l), "Letter", "{l}");
        }
        assert_eq!(unit("en-US"), Unit::Inches);
        assert_eq!(unit("en-GB"), Unit::Centimeters);
        assert_eq!(paper("en-GB"), "A4");
        assert_eq!(decimal(1.15, 2, "en-US"), "1.15");
        assert_eq!(count(1234, "en-US"), "1,234");
        assert_eq!(parse_decimal("1,234.5", "en-US"), Some(1234.5));
        assert_eq!(quotes("en-US"), ('“', '”', '‘', '’'));
        assert_eq!(quotes("fr-FR").0, '«');
    }

    #[test]
    fn new_documents_get_the_regions_paper_and_language() {
        let mut d = Document::new();
        localize_new(&mut d, "de-DE");
        let s = d.sections().first().map(|(_, s)| (*s).clone()).unwrap_or_default();
        assert_eq!(wordcraft_geom::paper_name(s.page_w, s.page_h), Some("A4"));
        assert!((s.margin_left - 70.866).abs() < 0.01);
        assert_eq!(d.styles.default_chr.lang.as_deref(), Some("de-DE"));
        let mut us = Document::new();
        localize_new(&mut us, "en-US");
        assert_eq!(us, Document::new(), "en-US new documents are Word's defaults");
    }
}
