//! Dates and times as people read them: local time, and the conventions of a region (`de-DE`
//! writes 10.10.2026 and 14:30, `en-US` writes 10/10/2026 and 2:30 PM).
//!
//! Stored timestamps (comments, revisions, document properties) stay UTC ISO 8601 via
//! [`crate::cmd::now_iso`]; this module is for text that lands in the document or the UI.

use chrono::{Datelike, Local, Timelike};

/// A local wall-clock moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalTime {
    pub year: i32,
    /// 1–12.
    pub month: u32,
    /// 1–31.
    pub day: u32,
    /// 0 = Monday … 6 = Sunday.
    pub weekday: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// The current local time (the system time zone; the browser's on the web).
pub fn now_local() -> LocalTime {
    let t = Local::now();
    LocalTime {
        year: t.year(),
        month: t.month(),
        day: t.day(),
        weekday: t.weekday().num_days_from_monday(),
        hour: t.hour(),
        minute: t.minute(),
        second: t.second(),
    }
}

/// The region's default pictures: short date, long date, time.
struct Pictures {
    short: &'static str,
    long: &'static str,
    time: &'static str,
}

const US: Pictures = Pictures { short: "M/d/yyyy", long: "MMMM d, yyyy", time: "h:mm AM/PM" };
const GERMAN: Pictures = Pictures { short: "dd.MM.yyyy", long: "d. MMMM yyyy", time: "HH:mm" };
const BRITISH: Pictures = Pictures { short: "dd/MM/yyyy", long: "d MMMM yyyy", time: "HH:mm" };
// Regions whose month names we don't have: the long date is the short one.
const SLASH: Pictures = Pictures { short: "dd/MM/yyyy", long: "dd/MM/yyyy", time: "HH:mm" };
const DOT: Pictures = Pictures { short: "dd.MM.yyyy", long: "dd.MM.yyyy", time: "HH:mm" };
const DASH: Pictures = Pictures { short: "dd-MM-yyyy", long: "dd-MM-yyyy", time: "HH:mm" };
const ISO: Pictures = Pictures { short: "yyyy-MM-dd", long: "yyyy-MM-dd", time: "HH:mm" };

/// `de_DE.UTF-8` → (`de`, `de`), `en-GB` → (`en`, `gb`).
fn split(locale: &str) -> (String, String) {
    let base = locale.split(['.', '@']).next().unwrap_or("").replace('_', "-").to_ascii_lowercase();
    let mut parts = base.split('-').filter(|p| !p.is_empty());
    let lang = parts.next().unwrap_or("").to_string();
    // The region is the first two-letter (or three-digit) subtag after the language.
    let region = parts.find(|p| p.len() == 2 || (p.len() == 3 && p.chars().all(|c| c.is_ascii_digit()))).unwrap_or("").to_string();
    (lang, region)
}

fn pictures(locale: &str) -> &'static Pictures {
    let (lang, region) = split(locale);
    match lang.as_str() {
        "de" => &GERMAN,
        "en" if region.is_empty() || matches!(region.as_str(), "us" | "ph" | "pr" | "um" | "vi" | "as" | "gu") => &US,
        "en" => &BRITISH,
        "fr" | "it" | "es" | "pt" | "el" | "ca" | "ga" | "mt" => &SLASH,
        "nl" => &DASH,
        "pl" | "cs" | "sk" | "ru" | "uk" | "be" | "fi" | "nb" | "nn" | "no" | "da" | "is" | "tr" | "ro" | "hr" | "sl" | "sr" | "bg" | "et" | "lv" => {
            &DOT
        }
        "sv" | "lt" => &ISO,
        _ => &US,
    }
}

/// The region's short date picture (`dd.MM.yyyy` for `de-DE`, `M/d/yyyy` for `en-US`).
pub fn short_date_picture(locale: &str) -> &'static str {
    pictures(locale).short
}

/// The region's long date picture (`d. MMMM yyyy` for `de-DE`, `MMMM d, yyyy` for `en-US`).
pub fn long_date_picture(locale: &str) -> &'static str {
    pictures(locale).long
}

/// The region's time picture (`HH:mm` for `de-DE`, `h:mm AM/PM` for `en-US`).
pub fn time_picture(locale: &str) -> &'static str {
    pictures(locale).time
}

/// A picture by name: `short`, `long` and `time` resolve for the region; anything else is a
/// picture already.
pub fn resolve_picture<'a>(picture: &'a str, locale: &str) -> &'a str {
    match picture {
        "short" => short_date_picture(locale),
        "long" => long_date_picture(locale),
        "time" => time_picture(locale),
        other => other,
    }
}

struct Names {
    months: [&'static str; 12],
    months_short: [&'static str; 12],
    /// Monday first.
    days: [&'static str; 7],
    days_short: [&'static str; 7],
}

const ENGLISH_NAMES: Names = Names {
    months: ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
    months_short: ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"],
    days: ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"],
    days_short: ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"],
};

const GERMAN_NAMES: Names = Names {
    months: ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"],
    months_short: ["Jan", "Feb", "Mär", "Apr", "Mai", "Jun", "Jul", "Aug", "Sep", "Okt", "Nov", "Dez"],
    days: ["Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag", "Sonntag"],
    days_short: ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"],
};

fn names(locale: &str) -> &'static Names {
    if split(locale).0 == "de" { &GERMAN_NAMES } else { &ENGLISH_NAMES }
}

/// Format `t` with a Word date picture (`M/d/yyyy`, `d. MMMM yyyy`, `dddd`, `HH:mm`, `h:mm am/pm`)
/// in the language of `locale`. Text in single quotes is copied as is.
pub fn format(picture: &str, locale: &str, t: &LocalTime) -> String {
    let names = names(locale);
    let month = |full: bool| {
        let i = t.month.saturating_sub(1) as usize;
        let list = if full { &names.months } else { &names.months_short };
        list.get(i).copied().unwrap_or("")
    };
    let day = |full: bool| {
        let list = if full { &names.days } else { &names.days_short };
        list.get(t.weekday as usize).copied().unwrap_or("")
    };
    let chars: Vec<char> = picture.chars().take(256).collect();
    let mut out = String::new();
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        if c == '\'' {
            let end = chars.iter().skip(i + 1).position(|&q| q == '\'').map_or(chars.len(), |n| i + 1 + n);
            out.extend(chars.get(i + 1..end).unwrap_or_default());
            i = end + 1;
            continue;
        }
        // am/pm markers: `AM/PM`, `am/pm`.
        let rest: String = chars.get(i..).unwrap_or_default().iter().take(5).collect();
        if rest.eq_ignore_ascii_case("am/pm") {
            let pm = t.hour >= 12;
            let upper = c == 'A';
            out.push_str(match (pm, upper) {
                (false, true) => "AM",
                (true, true) => "PM",
                (false, false) => "am",
                (true, false) => "pm",
            });
            i += 5;
            continue;
        }
        let mut n = 1;
        while chars.get(i + n) == Some(&c) {
            n += 1;
        }
        let h12 = if t.hour.is_multiple_of(12) { 12 } else { t.hour % 12 };
        match (c, n) {
            ('y', 3..) => out.push_str(&format!("{:04}", t.year)),
            ('y', _) => out.push_str(&format!("{:02}", t.year.rem_euclid(100))),
            ('M', 4..) => out.push_str(month(true)),
            ('M', 3) => out.push_str(month(false)),
            ('M', 2) => out.push_str(&format!("{:02}", t.month)),
            ('M', _) => out.push_str(&t.month.to_string()),
            ('d', 4..) => out.push_str(day(true)),
            ('d', 3) => out.push_str(day(false)),
            ('d', 2) => out.push_str(&format!("{:02}", t.day)),
            ('d', _) => out.push_str(&t.day.to_string()),
            ('H', 2..) => out.push_str(&format!("{:02}", t.hour)),
            ('H', _) => out.push_str(&t.hour.to_string()),
            ('h', 2..) => out.push_str(&format!("{h12:02}")),
            ('h', _) => out.push_str(&h12.to_string()),
            ('m', 2..) => out.push_str(&format!("{:02}", t.minute)),
            ('m', _) => out.push_str(&t.minute.to_string()),
            ('s', 2..) => out.push_str(&format!("{:02}", t.second)),
            ('s', _) => out.push_str(&t.second.to_string()),
            _ => out.extend(std::iter::repeat_n(c, n)),
        }
        i += n;
    }
    out
}

/// The system's region for dates (`LC_ALL`, `LC_TIME`, then the platform's locale), as a BCP 47
/// tag such as `de-DE`. `None` when there is none or it is `C`/`POSIX`.
pub fn system_locale() -> Option<String> {
    #[cfg(all(unix, not(target_arch = "wasm32")))]
    let env = ["LC_ALL", "LC_TIME"].iter().find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()));
    #[cfg(not(all(unix, not(target_arch = "wasm32"))))]
    let env: Option<String> = None;
    let tag = env.or_else(sys_locale::get_locale)?;
    let base = tag.split(['.', '@']).next().unwrap_or("").replace('_', "-");
    if base.is_empty() || base.eq_ignore_ascii_case("c") || base.eq_ignore_ascii_case("posix") || base.len() > 35 {
        return None;
    }
    Some(base)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Saturday 10 October 2026, 14:05:09.
    const T: LocalTime = LocalTime { year: 2026, month: 10, day: 10, weekday: 5, hour: 14, minute: 5, second: 9 };
    const MORNING: LocalTime = LocalTime { year: 2026, month: 3, day: 2, weekday: 0, hour: 0, minute: 7, second: 0 };

    #[test]
    fn german_conventions() {
        for l in ["de-DE", "de_DE.UTF-8", "de-AT", "de-CH", "de"] {
            assert_eq!(format(short_date_picture(l), l, &T), "10.10.2026", "{l}");
            assert_eq!(format(long_date_picture(l), l, &T), "10. Oktober 2026", "{l}");
            assert_eq!(format(time_picture(l), l, &T), "14:05", "{l}");
        }
        assert_eq!(format("dddd, d. MMMM yyyy", "de-DE", &T), "Samstag, 10. Oktober 2026");
        assert_eq!(format("ddd, d. MMM yy", "de-DE", &MORNING), "Mo, 2. Mär 26");
        assert_eq!(format("HH:mm:ss", "de-DE", &MORNING), "00:07:00");
    }

    #[test]
    fn us_and_european_conventions() {
        assert_eq!(format(short_date_picture("en-US"), "en-US", &T), "10/10/2026");
        assert_eq!(format(long_date_picture("en-US"), "en-US", &T), "October 10, 2026");
        assert_eq!(format(time_picture("en-US"), "en-US", &T), "2:05 PM");
        assert_eq!(format("h:mm am/pm", "en-US", &MORNING), "12:07 am");
        assert_eq!(format(short_date_picture("en-GB"), "en-GB", &MORNING), "02/03/2026");
        assert_eq!(format(long_date_picture("en-GB"), "en-GB", &T), "10 October 2026");
        assert_eq!(format(short_date_picture("fr-FR"), "fr-FR", &MORNING), "02/03/2026");
        assert_eq!(format(short_date_picture("nl-NL"), "nl-NL", &MORNING), "02-03-2026");
        assert_eq!(format(short_date_picture("pl-PL"), "pl-PL", &MORNING), "02.03.2026");
        assert_eq!(format(short_date_picture("sv-SE"), "sv-SE", &MORNING), "2026-03-02");
        for l in ["en-GB", "fr-FR", "it", "es-ES", "nl", "pl", "sv-SE"] {
            assert_eq!(time_picture(l), "HH:mm", "{l}");
        }
        assert_eq!(short_date_picture(""), "M/d/yyyy");
        assert_eq!(short_date_picture("xx-YY"), "M/d/yyyy");
    }

    #[test]
    fn named_pictures_and_literals() {
        assert_eq!(resolve_picture("short", "de-DE"), "dd.MM.yyyy");
        assert_eq!(resolve_picture("time", "en-US"), "h:mm AM/PM");
        assert_eq!(resolve_picture("yyyy", "de-DE"), "yyyy");
        assert_eq!(format("'Stand:' dd.MM.", "de-DE", &T), "Stand: 10.10.");
        assert_eq!(format("'unterminated", "de-DE", &T), "unterminated");
        assert_eq!(format("", "de-DE", &T), "");
    }

    #[test]
    fn now_is_a_real_date() {
        let t = now_local();
        assert!((1..=12).contains(&t.month) && (1..=31).contains(&t.day) && t.weekday < 7 && t.hour < 24);
    }
}
