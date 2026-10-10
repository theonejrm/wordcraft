//! WordCraft proofing.
//!
//! - **Spelling** against the public-domain Moby word list (~160k words, the same list that
//!   drives hyphenation), with inflections (`-s`, `-es`, `-ed`, `-ing`, `-ly`, `'s`…), a user
//!   dictionary and ignore lists; suggestions by edit distance.
//! - **Grammar** checks that are cheap and reliable: repeated words, `a`/`an`, capital letter
//!   after a sentence end, spaces before punctuation, doubled spaces.
//! - **Hyphenation** ([`hyphen`]): dictionary, Liang patterns, heuristic.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod dict;
pub mod files;
pub mod german;
pub mod hunspell;
pub mod hyphen;
pub mod languagetool;
pub mod libhyphen;
pub mod patterns;

pub use files::{busy, generation};

use std::collections::HashSet;
use std::sync::{OnceLock, RwLock};

/// A problem found in text: byte range, kind, message and suggestions.
#[derive(Clone, Debug, PartialEq)]
pub struct Issue {
    pub start: usize,
    pub end: usize,
    pub kind: IssueKind,
    pub message: String,
    pub suggestions: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssueKind {
    Spelling,
    Grammar,
}

fn user_words() -> &'static RwLock<HashSet<String>> {
    static U: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();
    U.get_or_init(|| RwLock::new(HashSet::new()))
}

/// Add a word to the user dictionary ("Add to Dictionary").
pub fn add_word(w: &str) {
    user_words().write().unwrap_or_else(|e| e.into_inner()).insert(w.to_lowercase());
}

/// Words in the user dictionary.
pub fn user_dictionary() -> Vec<String> {
    let mut v: Vec<String> = user_words().read().unwrap_or_else(|e| e.into_inner()).iter().cloned().collect();
    v.sort();
    v
}

/// Common words the source list lacks (modern vocabulary, contractions).
const COMMON: &str = "a an the and or but if of to in on at by for with from as is am are was were be been being it its we us our you your he him his she her they them their i me my mine this that these those there here what which who whom whose when where why how all any both each few more most other some such no nor not only own same so than too very can will just should now do does did done has have had having may might must shall would could up down out over under again further then once off into onto upon about above below between through during before after until while because per via vs etc ok yes yeah oh ah hi hey bye one two three four five six seven eight nine ten first last next new old get got go goes went gone see saw seen say said make made take took come came give gave know knew think thought look want need use used find found tell told ask asked feel felt try left keep let put mean set run show turn move live play pay hear hold bring begin seem help talk start might";

const EXTRA: &[&str] = &[
    "don't",
    "doesn't",
    "didn't",
    "can't",
    "won't",
    "isn't",
    "aren't",
    "wasn't",
    "weren't",
    "shouldn't",
    "wouldn't",
    "couldn't",
    "i'm",
    "i've",
    "i'd",
    "i'll",
    "you're",
    "you've",
    "you'll",
    "you'd",
    "we're",
    "we've",
    "we'll",
    "they're",
    "they've",
    "they'll",
    "it's",
    "that's",
    "there's",
    "here's",
    "what's",
    "let's",
    "email",
    "emails",
    "online",
    "website",
    "websites",
    "internet",
    "software",
    "app",
    "apps",
    "smartphone",
    "blog",
    "login",
    "logout",
    "username",
    "wifi",
    "toolbar",
    "workflow",
    "workflows",
    "dataset",
    "datasets",
    "metadata",
    "startup",
    "startups",
    "covid",
    "selfie",
    "podcast",
    "podcasts",
    "hashtag",
    "wordcraft",
    "artcraft",
    "discord",
    "github",
    "rust",
    "ok",
    "okay",
    "unlabelled",
    "labelled",
    "colour",
    "colours",
    "favourite",
    "centre",
    "organise",
];

fn known_core(w: &str) -> bool {
    let d = dict::Dictionary::en_us();
    if d.get(w).is_some() || EXTRA.contains(&w) || COMMON.split(' ').any(|c| c == w) {
        return true;
    }
    if user_words().read().unwrap_or_else(|e| e.into_inner()).contains(w) {
        return true;
    }
    // Inflections.
    let stems: [(&str, &[&str]); 12] = [
        ("'s", &[""]),
        ("s", &[""]),
        ("es", &["", "e"]),
        ("ies", &["y"]),
        ("ed", &["", "e"]),
        ("ied", &["y"]),
        ("ing", &["", "e"]),
        ("ly", &["", "le"]),
        ("ily", &["y"]),
        ("er", &["", "e"]),
        ("est", &["", "e"]),
        ("ness", &[""]),
    ];
    for (suf, adds) in stems {
        if let Some(stem) = w.strip_suffix(suf)
            && stem.len() >= 2
        {
            for a in adds {
                let s = format!("{stem}{a}");
                if d.get(&s).is_some() || EXTRA.contains(&s.as_str()) {
                    return true;
                }
            }
            // Doubled consonant: running → run, stopped → stop.
            let b = stem.as_bytes();
            if (suf == "ing" || suf == "ed" || suf == "er" || suf == "est")
                && b.len() >= 3
                && b.last() == b.get(b.len() - 2)
                && let Some(s) = stem.get(..stem.len() - 1)
                && d.get(s).is_some()
            {
                return true;
            }
        }
    }
    // Prefixes.
    for pre in ["un", "re", "non", "pre", "over", "under", "co", "mis"] {
        if let Some(rest) = w.strip_prefix(pre)
            && rest.len() >= 3
            && d.get(rest).is_some()
        {
            return true;
        }
    }
    false
}

/// Words every language accepts without a dictionary: single letters, words with digits,
/// ALL-CAPS acronyms up to six letters, anything without letters.
fn always_fine(w: &str) -> bool {
    w.chars().count() <= 1
        || w.chars().any(|c| c.is_ascii_digit())
        || !w.chars().any(char::is_alphabetic)
        || (w.chars().all(|c| !c.is_lowercase()) && w.chars().count() <= 6)
}

/// Is `word` spelled correctly? Numbers, single letters, ALL-CAPS acronyms, URLs and words with
/// digits are accepted.
pub fn is_correct(word: &str) -> bool {
    let w = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'');
    let w = w.trim_matches('\'');
    if always_fine(w) {
        return true;
    }
    if !w.is_ascii() && !w.chars().any(|c| c.is_ascii_alphabetic()) {
        return true; // other scripts: no dictionary
    }
    let lower = w.to_lowercase().replace('’', "'");
    if known_core(&lower) {
        return true;
    }
    // Hyphenated compounds: each part.
    if lower.contains('-') {
        return lower.split('-').all(|p| p.is_empty() || known_core(p));
    }
    false
}

/// Spelling suggestions for a misspelled word (best first, at most `max`).
pub fn suggest(word: &str, max: usize) -> Vec<String> {
    let lower = word.to_lowercase();
    let n = lower.chars().count();
    if n == 0 || n > 40 {
        return Vec::new();
    }
    let d = dict::Dictionary::en_us();
    let mut scored: Vec<(usize, i64, String)> = Vec::new();
    let target: Vec<char> = lower.chars().collect();
    for (w, _) in d.iter() {
        let wn = w.chars().count();
        if wn + 2 < n || wn > n + 2 {
            continue;
        }
        let dist = damerau(&target, w);
        if dist <= 2 {
            let same_first = w.chars().next() == target.first().copied();
            scored.push((dist, if same_first { 0 } else { 1 }, w.to_string()));
        }
    }
    scored.sort_by(|a, b| (a.0, a.1, a.2.len().abs_diff(n)).cmp(&(b.0, b.1, b.2.len().abs_diff(n))).then(a.2.cmp(&b.2)));
    let cap = word.chars().next().is_some_and(char::is_uppercase);
    let upper = word.chars().all(|c| !c.is_lowercase());
    scored
        .into_iter()
        .take(max)
        .map(|(_, _, w)| {
            if upper && n > 1 {
                w.to_uppercase()
            } else if cap {
                let mut c = w.chars();
                c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or(w)
            } else {
                w
            }
        })
        .collect()
}

/// Optimal string alignment distance, early-exit at 3.
fn damerau(a: &[char], b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let (n, m) = (a.len(), b.len());
    let mut prev2 = vec![0usize; m + 1];
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut cur = vec![0usize; m + 1];
    for i in 1..=n {
        if let Some(c) = cur.get_mut(0) {
            *c = i;
        }
        let mut row_min = i;
        for j in 1..=m {
            let (ai, bj) = (a.get(i - 1), b.get(j - 1));
            let cost = usize::from(ai != bj);
            let mut v = (prev.get(j).copied().unwrap_or(9) + 1)
                .min(cur.get(j - 1).copied().unwrap_or(9) + 1)
                .min(prev.get(j - 1).copied().unwrap_or(9) + cost);
            if i > 1 && j > 1 && a.get(i - 1) == b.get(j - 2) && a.get(i - 2) == b.get(j - 1) {
                v = v.min(prev2.get(j - 2).copied().unwrap_or(9) + 1);
            }
            if let Some(c) = cur.get_mut(j) {
                *c = v;
            }
            row_min = row_min.min(v);
        }
        if row_min > 2 {
            return 3;
        }
        std::mem::swap(&mut prev2, &mut prev);
        std::mem::swap(&mut prev, &mut cur);
    }
    prev.get(m).copied().unwrap_or(9)
}

/// Words in `text`: (byte start, byte end).
pub fn words(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let is_w = |c: char| c.is_alphanumeric() || c == '\'' || c == '’' || c == '-';
    for (i, c) in text.char_indices() {
        match (start, is_w(c)) {
            (None, true) => start = Some(i),
            (Some(s), false) => {
                out.push((s, i));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        out.push((s, text.len()));
    }
    // Trim surrounding apostrophes/hyphens.
    out.into_iter()
        .filter_map(|(a, b)| {
            let w = text.get(a..b)?;
            let lead = w.len() - w.trim_start_matches(['\'', '’', '-']).len();
            let trail = w.len() - w.trim_end_matches(['\'', '’', '-']).len();
            (a + lead < b - trail).then_some((a + lead, b - trail))
        })
        .collect()
}

/// Is text in this language judged by the built-in English tools? Untagged text counts as
/// English. Other languages use installed dictionaries ([`hunspell`], [`libhyphen`]).
pub fn is_english(lang: Option<&str>) -> bool {
    lang.is_none_or(|l| l.is_empty() || l.get(..2).is_some_and(|p| p.eq_ignore_ascii_case("en")))
}

fn is_german(lang: Option<&str>) -> bool {
    lang.and_then(|l| l.get(..2)).is_some_and(|p| p.eq_ignore_ascii_case("de"))
}

/// The language of the text at a byte offset of a paragraph (`None`: untagged, i.e. English).
pub type LangAt<'a> = &'a dyn Fn(usize) -> Option<String>;

fn is_url_like(text: &str, a: usize, b: usize) -> bool {
    let ts = text.get(..a).and_then(|t| t.rfind(char::is_whitespace)).map(|i| i + 1).unwrap_or(0);
    let te = text.get(b..).and_then(|t| t.find(char::is_whitespace)).map(|i| b + i).unwrap_or(text.len());
    let token = text.get(ts..te).unwrap_or("");
    token.contains("://") || token.contains('@') || token.starts_with("www.")
}

/// Spelling issues with every word judged in its own language: English by the built-in list,
/// other languages by their installed dictionary, and text in a language without one isn't
/// judged. Without `wait`, a dictionary that is still loading means "not checked yet".
pub fn check_spelling_in(text: &str, lang_at: LangAt, wait: bool) -> Vec<Issue> {
    let mut v = Vec::new();
    let mut spellers: Vec<(String, Option<std::sync::Arc<hunspell::Speller>>)> = Vec::new();
    for (a, b) in words(text) {
        let Some(w) = text.get(a..b) else { continue };
        if is_url_like(text, a, b) {
            continue;
        }
        let lang = lang_at(a);
        let ok = if is_english(lang.as_deref()) {
            is_correct(w)
        } else {
            let l = lang.unwrap_or_default();
            let sp = match spellers.iter().find(|(k, _)| *k == l) {
                Some((_, sp)) => sp.clone(),
                None => {
                    let sp = hunspell::speller(&l, wait);
                    spellers.push((l, sp.clone()));
                    sp
                }
            };
            let Some(sp) = sp else { continue };
            let core = w.trim_matches(['\'', '-']);
            always_fine(core)
                || sp.check(w)
                // Abbreviations are listed with their period (bzw., ca.).
                || (text.get(b..).is_some_and(|t| t.starts_with('.')) && sp.check(&format!("{w}.")))
                // Hyphenated compounds the dictionary doesn't list: each part.
                || (w.contains('-') && w.split('-').all(|p| p.is_empty() || always_fine(p) || sp.check(p)))
        };
        if !ok {
            v.push(Issue { start: a, end: b, kind: IssueKind::Spelling, message: "Possible spelling mistake".into(), suggestions: Vec::new() });
        }
    }
    v
}

/// Normalized language key for comparisons (`de-de`); `en-us` for untagged text.
fn lang_key(lang: Option<&str>) -> String {
    lang.filter(|l| !l.is_empty()).unwrap_or("en-US").replace('_', "-").to_ascii_lowercase()
}

/// Grammar issues with each part of the text checked by its language's rules: LanguageTool for
/// the paragraph's main language when a server is configured and has answered, the built-in rules
/// otherwise (English rules for English, German rules for German, spacing for the rest).
pub fn check_grammar_in(text: &str, lang_at: LangAt, wait: bool) -> Vec<Issue> {
    let ws = words(text);
    if ws.is_empty() && !text.contains("  ") {
        return Vec::new();
    }
    // The paragraph's main language, by letters.
    let mut weights: Vec<(String, Option<String>, usize)> = Vec::new();
    for (a, b) in &ws {
        let l = lang_at(*a);
        let k = lang_key(l.as_deref());
        match weights.iter_mut().find(|(key, _, _)| *key == k) {
            Some(w) => w.2 += b - a,
            None => weights.push((k, l, b - a)),
        }
    }
    let main = weights.iter().max_by_key(|w| w.2).map(|w| (w.0.clone(), w.1.clone().unwrap_or_else(|| "en-US".into())));
    let server = match &main {
        Some((_, tag)) if languagetool::url().is_some() => languagetool::grammar(text, tag, wait),
        _ => None,
    };
    let covered = |k: &str| server.is_some() && main.as_ref().is_some_and(|(mk, _)| mk == k);
    let mut out: Vec<Issue> = Vec::new();
    if let Some(issues) = &server {
        out.extend(issues.iter().filter(|i| covered(&lang_key(lang_at(i.start).as_deref()))).cloned());
    }
    let english = check_grammar(text);
    let german = german::check(text);
    for i in english {
        let l = lang_at(i.start);
        if is_english(l.as_deref()) && !covered(&lang_key(l.as_deref())) {
            out.push(i);
        }
    }
    for i in german {
        let l = lang_at(i.start);
        let k = lang_key(l.as_deref());
        if covered(&k) || is_english(l.as_deref()) {
            continue;
        }
        if is_german(l.as_deref()) || i.message == "Extra space" {
            out.push(i);
        }
    }
    out.sort_by_key(|i| i.start);
    out
}

/// Spelling suggestions in a language (English: the built-in list; others: their dictionary).
pub fn suggest_in(word: &str, lang: Option<&str>, max: usize) -> Vec<String> {
    if is_english(lang) {
        return suggest(word, max);
    }
    lang.and_then(|l| hunspell::speller(l, true)).map(|sp| sp.suggest(word, max)).unwrap_or_default()
}

/// Hyphenation points of `word` in a language (char indices, limits applied); empty for a
/// language without patterns, or (without `wait`) while they load.
pub fn hyphen_points_in(word: &str, lim: &hyphen::Limits, lang: Option<&str>, wait: bool) -> Vec<usize> {
    if is_english(lang) {
        return hyphen::hyphen_points(word, lim);
    }
    let Some(h) = lang.and_then(|l| libhyphen::hyphenator(l, wait)) else { return Vec::new() };
    let (pts, _) = hyphen::word_breaks_with(word, &|core: &[char]| (h.points(core), hyphen::Source::Patterns));
    hyphen::limit_points(word, pts, lim)
}

/// Spelling issues in a paragraph of text (no suggestions; ask [`suggest`] on demand).
pub fn check_spelling(text: &str) -> Vec<Issue> {
    let mut v = Vec::new();
    for (a, b) in words(text) {
        let Some(w) = text.get(a..b) else { continue };
        // Skip URLs/emails.
        let ts = text.get(..a).and_then(|t| t.rfind(char::is_whitespace)).map(|i| i + 1).unwrap_or(0);
        let te = text.get(b..).and_then(|t| t.find(char::is_whitespace)).map(|i| b + i).unwrap_or(text.len());
        let token = text.get(ts..te).unwrap_or("");
        if token.contains("://") || token.contains('@') || token.starts_with("www.") {
            continue;
        }
        if !is_correct(w) {
            v.push(Issue { start: a, end: b, kind: IssueKind::Spelling, message: "Possible spelling mistake".into(), suggestions: Vec::new() });
        }
    }
    v
}

/// Grammar issues in a paragraph of text.
pub fn check_grammar(text: &str) -> Vec<Issue> {
    let mut v = Vec::new();
    let ws = words(text);
    for pair in ws.windows(2) {
        let [(a0, b0), (a1, b1)] = pair else { continue };
        let (Some(w0), Some(w1), Some(gap)) = (text.get(*a0..*b0), text.get(*a1..*b1), text.get(*b0..*a1)) else { continue };
        if gap.trim().is_empty()
            && w0.eq_ignore_ascii_case(w1)
            && w0.chars().all(char::is_alphabetic)
            && !matches!(w0.to_lowercase().as_str(), "had" | "that" | "bye" | "so")
        {
            v.push(Issue { start: *a0, end: *b1, kind: IssueKind::Grammar, message: "Repeated word".into(), suggestions: vec![w0.to_string()] });
        }
        if gap == " " && (w0 == "a" || w0 == "A") && w1.chars().next().is_some_and(|c| "aeiouAEIOU".contains(c)) && !starts_consonant_sound(w1) {
            v.push(Issue {
                start: *a0,
                end: *b0,
                kind: IssueKind::Grammar,
                message: "Use \"an\" before a vowel sound".into(),
                suggestions: vec![if w0 == "A" { "An" } else { "an" }.into()],
            });
        }
        if gap == " "
            && (w0 == "an" || w0 == "An")
            && w1.chars().next().is_some_and(|c| c.is_alphabetic() && !"aeiouAEIOUhH".contains(c))
            && !w1.chars().all(|c| c.is_uppercase())
        {
            v.push(Issue {
                start: *a0,
                end: *b0,
                kind: IssueKind::Grammar,
                message: "Use \"a\" before a consonant sound".into(),
                suggestions: vec![if w0 == "An" { "A" } else { "a" }.into()],
            });
        }
    }
    // Space before punctuation; doubled spaces.
    for (i, _) in text.match_indices(" ,").chain(text.match_indices(" .")).chain(text.match_indices(" ;")) {
        if text.get(i + 2..).is_some_and(|r| r.starts_with('.') || r.starts_with(',')) {
            continue; // "..." or numbers like " .5"
        }
        v.push(Issue {
            start: i,
            end: i + 2,
            kind: IssueKind::Grammar,
            message: "Remove the space before the punctuation".into(),
            suggestions: vec![text.get(i + 1..i + 2).unwrap_or("").to_string()],
        });
    }
    for (i, _) in text.match_indices("  ") {
        if i > 0 && text.get(..i).is_some_and(|t| t.ends_with(' ')) {
            continue;
        }
        v.push(Issue { start: i, end: i + 2, kind: IssueKind::Grammar, message: "Extra space".into(), suggestions: vec![" ".into()] });
    }
    // Sentence start capital.
    let mut after_end = true;
    for (a, b) in &ws {
        let Some(w) = text.get(*a..*b) else { continue };
        if after_end && w.chars().next().is_some_and(char::is_lowercase) && w.chars().all(char::is_alphabetic) {
            let fixed: String = w.chars().next().map(|f| f.to_uppercase().chain(w.chars().skip(1)).collect()).unwrap_or_default();
            v.push(Issue {
                start: *a,
                end: *b,
                kind: IssueKind::Grammar,
                message: "Capitalize the first word of a sentence".into(),
                suggestions: vec![fixed],
            });
        }
        let tail = text.get(*b..).unwrap_or("");
        let next_non_space = tail.trim_start_matches(['"', '”', ')', '\'']).chars().next();
        after_end = matches!(next_non_space, Some('.' | '!' | '?')) && tail.trim_start_matches(['.', '!', '?', '"', '”', ')']).starts_with(' ');
    }
    v.sort_by_key(|i| i.start);
    v
}

fn starts_consonant_sound(w: &str) -> bool {
    let l = w.to_lowercase();
    ["uni", "use", "usu", "eu", "one", "once", "ubiq", "uti", "ure", "uro"].iter().any(|p| l.starts_with(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictionary_loads() {
        assert!(dict::Dictionary::en_us().len() > 100_000);
    }

    #[test]
    fn spelling() {
        for w in [
            "if",
            "we",
            "hello",
            "world",
            "running",
            "studios",
            "don't",
            "Thursday",
            "NASA",
            "2026",
            "co-operate",
            "unhappy",
            "colour",
            "it's",
            "stopped",
        ] {
            assert!(is_correct(w), "{w}");
        }
        for w in ["helo", "wrold", "sentense", "recieve"] {
            assert!(!is_correct(w), "{w}");
        }
        let s = suggest("recieve", 5);
        assert!(s.contains(&"receive".to_string()), "{s:?}");
        assert!(suggest("Wrold", 3).contains(&"World".to_string()));
    }

    #[test]
    fn spelling_issues_in_text() {
        let t = "This sentense has a mispelled word, see https://example.com/xyzq.";
        let v = check_spelling(t);
        let bad: Vec<&str> = v.iter().map(|i| &t[i.start..i.end]).collect();
        assert_eq!(bad, vec!["sentense", "mispelled"]);
    }

    #[test]
    fn grammar_issues() {
        let t = "this is is a apple , and an car.  Done";
        let v = check_grammar(t);
        let msgs: Vec<&str> = v.iter().map(|i| i.message.as_str()).collect();
        assert!(msgs.iter().any(|m| m.contains("Repeated")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("\"an\"")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("\"a\" before")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("space before")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("Capitalize")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("Extra space")), "{msgs:?}");
        assert!(check_grammar("A university is an hour away.").is_empty(), "{:?}", check_grammar("A university is an hour away."));
    }

    #[test]
    fn user_dictionary_works() {
        assert!(!is_correct("zxqwordcrafty"));
        add_word("zxqwordcrafty");
        assert!(is_correct("zxqwordcrafty"));
    }

    #[test]
    fn hyphenation() {
        let l = hyphen::Limits::default();
        assert!(hyphen::hyphenate_word("hyphenation", &l).matches('-').count() >= 2);
    }

    /// English and another language in one paragraph: each part by its own rules.
    #[test]
    fn mixed_languages() {
        hunspell::register("xr-RR", hunspell::tests::AFF, hunspell::tests::DIC).unwrap();
        let text = "This sentense is fine. haus hasu ist ist gut";
        let split = text.find("haus").unwrap_or(0);
        let lang = |o: usize| if o >= split { Some("xr-RR".to_string()) } else { None };
        let bad: Vec<&str> = check_spelling_in(text, &lang, true).iter().map(|i| &text[i.start..i.end]).collect();
        // "ist" and "gut" aren't in the tiny dictionary either.
        assert_eq!(bad.first().copied(), Some("sentense"));
        assert!(bad.contains(&"hasu") && !bad.contains(&"haus"), "{bad:?}");
        // German rules don't run on xr text, but the spacing rule does; English rules only on English.
        let g = check_grammar_in("Das ist ist gut.  Fine", &|_| Some("de-DE".into()), true);
        assert!(g.iter().any(|i| i.message == "Repeated word") && g.iter().any(|i| i.message == "Extra space"), "{g:?}");
        let g = check_grammar_in("a apple", &|_| Some("fr-FR".into()), true);
        assert!(g.is_empty(), "{g:?}");
        assert_eq!(suggest_in("hasu", Some("xr-RR"), 3).first().map(String::as_str), Some("haus"));
        assert!(suggest_in("hasu", Some("zz-ZZ"), 3).is_empty());
    }

    #[test]
    fn hyphenation_by_language() {
        libhyphen::register("xh-HH", "UTF-8\ns1b\nNEXTLEVEL\no1t\n");
        let lim = hyphen::Limits::default();
        let pts = hyphen_points_in("Hausboote", &lim, Some("xh-HH"), true);
        assert_eq!(pts, vec![4, 7]);
        assert!(hyphen_points_in("Hausboote", &lim, Some("zz-ZZ"), true).is_empty());
        assert!(!hyphen_points_in("hyphenation", &lim, None, true).is_empty());
    }

    /// LanguageTool answers replace the built-in rules for the paragraph's language.
    #[test]
    fn grammar_from_a_languagetool_server() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming().take(4) {
                let Ok(mut s) = stream else { continue };
                let mut buf = vec![0u8; 65536];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let body = if req.contains("language=xl-LL") {
                    r#"{"matches":[{"message":"Komma fehlt","offset":3,"length":4,"replacements":[{"value":"eins,"}],"rule":{"issueType":"grammar","category":{"id":"PUNCTUATION"}}}]}"#
                } else {
                    r#"{"matches":[]}"#
                };
                let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes());
            }
        });
        languagetool::configure(Some(&format!("http://127.0.0.1:{port}"))).unwrap();
        let text = "Es eins  zwei";
        let g = check_grammar_in(text, &|_| Some("xl-LL".into()), true);
        languagetool::configure(None).unwrap();
        // The server's comma issue; its answer covers the paragraph, so the spacing rule is skipped.
        assert_eq!(g.len(), 1, "{g:?}");
        assert_eq!(&text[g[0].start..g[0].end], "eins");
        assert_eq!(g[0].message, "Komma fehlt");
    }

    proptest::proptest! {
        #[test]
        fn never_panics(s in "\\PC{0,80}") {
            let _ = check_spelling(&s);
            let _ = check_grammar(&s);
            let _ = suggest(&s, 3);
            let _ = hyphen::hyphen_points(&s, &hyphen::Limits::default());
            let de = |_: usize| Some("de-DE".to_string());
            let _ = check_spelling_in(&s, &de, false);
            let _ = check_grammar_in(&s, &de, false);
            let _ = german::check(&s);
            let _ = hyphen_points_in(&s, &hyphen::Limits::default(), Some("de-DE"), false);
        }
    }
}
