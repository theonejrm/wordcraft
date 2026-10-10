//! Grammar checks for German text that are cheap and reliable: repeated words, spaces around
//! punctuation, a capital letter after a sentence end (not after abbreviations such as `z. B.` or
//! ordinals such as `3.`). Deeper checks come from a LanguageTool server when one is set up
//! ([`crate::languagetool`]).

use crate::{Issue, IssueKind, words};

/// Words that may legitimately appear twice in a row (`…, die die Arbeit macht`).
const MAY_REPEAT: &[&str] =
    &["der", "die", "das", "den", "dem", "des", "sie", "er", "es", "wir", "ihr", "man", "was", "wer", "ob", "zu", "so", "da", "nur", "sehr"];

/// Abbreviations after which a lowercase word does not start a sentence.
const ABBREVIATIONS: &[&str] = &[
    "bzw", "ca", "usw", "etc", "vgl", "ggf", "evtl", "inkl", "zzgl", "ggü", "Nr", "Dr", "Prof", "Hr", "Fr", "Abs", "Art", "Mio", "Mrd", "bspw",
    "insb", "sog", "u", "z", "d", "o", "i", "s", "Std", "Min", "Tel", "St", "geb", "verh", "gest", "Jh", "Jhd", "allg", "eigtl", "max", "min", "Bd",
    "Hrsg", "Aufl", "ff", "f",
];

/// Grammar issues in German text.
pub fn check(text: &str) -> Vec<Issue> {
    let mut v = Vec::new();
    let ws = words(text);
    for pair in ws.windows(2) {
        let [(a0, b0), (a1, b1)] = pair else { continue };
        let (Some(w0), Some(w1), Some(gap)) = (text.get(*a0..*b0), text.get(*a1..*b1), text.get(*b0..*a1)) else { continue };
        if gap.trim().is_empty()
            && w0.to_lowercase() == w1.to_lowercase()
            && w0.chars().all(char::is_alphabetic)
            && !MAY_REPEAT.contains(&w0.to_lowercase().as_str())
        {
            v.push(Issue { start: *a0, end: *b1, kind: IssueKind::Grammar, message: "Repeated word".into(), suggestions: vec![w0.to_string()] });
        }
    }
    for p in [" ,", " .", " ;", " :", " !", " ?"] {
        for (i, _) in text.match_indices(p) {
            // "..." and decimals like " .5" are fine; so is a lone " -".
            if text.get(i + 2..).is_some_and(|r| r.starts_with('.') || r.starts_with(',') || r.starts_with(|c: char| c.is_ascii_digit())) {
                continue;
            }
            v.push(Issue {
                start: i,
                end: i + 2,
                kind: IssueKind::Grammar,
                message: "Remove the space before the punctuation".into(),
                suggestions: vec![p.trim().to_string()],
            });
        }
    }
    for (i, _) in text.match_indices("  ") {
        if i > 0 && text.get(..i).is_some_and(|t| t.ends_with(' ')) {
            continue;
        }
        v.push(Issue { start: i, end: i + 2, kind: IssueKind::Grammar, message: "Extra space".into(), suggestions: vec![" ".into()] });
    }
    // A lowercase word after the end of a sentence.
    for pair in ws.windows(2) {
        let [(a0, b0), (a1, b1)] = pair else { continue };
        let (Some(w0), Some(w1), Some(gap)) = (text.get(*a0..*b0), text.get(*a1..*b1), text.get(*b0..*a1)) else { continue };
        let ends = gap.trim_end().trim_end_matches(['"', '“', '”', '»', '«', ')', '\'']);
        let sentence_end = matches!(ends, "." | "!" | "?" | "!?" | "?!") && gap.ends_with(' ');
        if !sentence_end || !w1.chars().next().is_some_and(char::is_lowercase) || !w1.chars().all(char::is_alphabetic) {
            continue;
        }
        // Abbreviations (z. B., bzw.), ordinals (3.), dates (10.10.) and ellipses don't end sentences.
        if ends == "." && (ABBREVIATIONS.contains(&w0) || w0.chars().count() == 1 || w0.chars().all(|c| c.is_ascii_digit())) {
            continue;
        }
        let fixed: String = w1.chars().next().map(|f| f.to_uppercase().chain(w1.chars().skip(1)).collect()).unwrap_or_default();
        v.push(Issue {
            start: *a1,
            end: *b1,
            kind: IssueKind::Grammar,
            message: "Capitalize the first word of a sentence".into(),
            suggestions: vec![fixed],
        });
    }
    v.sort_by_key(|i| i.start);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn messages(t: &str) -> Vec<(String, String)> {
        check(t).into_iter().map(|i| (t.get(i.start..i.end).unwrap_or("").to_string(), i.message)).collect()
    }

    #[test]
    fn finds_typical_slips() {
        let m = messages("Das ist ist gut , oder?  ja. das auch!");
        assert!(m.contains(&("ist ist".into(), "Repeated word".into())), "{m:?}");
        assert!(m.contains(&(" ,".into(), "Remove the space before the punctuation".into())), "{m:?}");
        assert!(m.contains(&("  ".into(), "Extra space".into())), "{m:?}");
        assert!(m.contains(&("das".into(), "Capitalize the first word of a sentence".into())), "{m:?}");
    }

    #[test]
    fn leaves_correct_german_alone() {
        for t in [
            "Wir sehen die Frau, die die Arbeit macht.",
            "Das gilt z. B. für Äpfel bzw. für Birnen usw. und mehr.",
            "Am 3. Oktober und am 10.10. ist frei.",
            "Er sagte „Hallo“. Danach ging er.",
            "Kosten: ca. zehn Euro … und so weiter.",
        ] {
            assert!(check(t).is_empty(), "{t}: {:?}", messages(t));
        }
    }
}
