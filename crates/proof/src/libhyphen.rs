//! Hyphenation in languages beyond the built-in English, from the hyphenation pattern files
//! (`hyph_de_DE.dic`) that LibreOffice and the system's hyphenation library use, found as described
//! in [`crate::files`].
//!
//! The format: the encoding on the first line, then Liang patterns (`ab1c`), keywords
//! (`LEFTHYPHENMIN 2`, `COMPOUNDLEFTHYPHENMIN 2`, …) and optionally `NEXTLEVEL`. With two levels,
//! the first level finds the boundaries of compound words (`Donau|dampf|schiff…`) and the second
//! hyphenates each part. Patterns may be any length (German ones run past 30 letters).

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};

use crate::files;

/// One level of Liang patterns.
#[derive(Default, Debug)]
struct Level {
    /// Letters (with `.` for a word edge) → values between them (`letters + 1` entries).
    map: HashMap<String, Vec<u8>>,
    max_len: usize,
}

impl Level {
    fn insert(&mut self, pat: &str) {
        let mut letters = String::new();
        let mut vals = vec![0u8];
        for c in pat.chars() {
            if let Some(d) = c.to_digit(10) {
                if let Some(v) = vals.last_mut() {
                    *v = d as u8;
                }
            } else {
                letters.push(c);
                vals.push(0);
            }
        }
        let n = letters.chars().count();
        if n == 0 || n > 64 {
            return;
        }
        self.max_len = self.max_len.max(n);
        self.map.insert(letters, vals);
    }

    /// Odd-valued positions inside `word` (lowercase chars): break before char `i`.
    fn points(&self, word: &[char]) -> Vec<usize> {
        if self.map.is_empty() || word.len() < 2 {
            return Vec::new();
        }
        let mut padded = Vec::with_capacity(word.len() + 2);
        padded.push('.');
        padded.extend_from_slice(word);
        padded.push('.');
        let mut vals = vec![0u8; padded.len() + 1];
        for start in 0..padded.len() {
            let mut key = String::new();
            for (len, c) in padded.iter().skip(start).take(self.max_len).enumerate() {
                key.push(*c);
                if let Some(v) = self.map.get(&key) {
                    for (k, x) in v.iter().enumerate().take(len + 2) {
                        if let Some(slot) = vals.get_mut(start + k) {
                            *slot = (*slot).max(*x);
                        }
                    }
                }
            }
        }
        // vals[p] sits before padded char p; word char i is padded char i + 1.
        (1..word.len()).filter(|&i| vals.get(i + 1).is_some_and(|v| v % 2 == 1)).collect()
    }
}

/// A language's hyphenation patterns.
#[derive(Debug, Default)]
pub struct Hyphenator {
    levels: Vec<Level>,
    pub left_min: usize,
    pub right_min: usize,
    pub compound_left_min: usize,
    pub compound_right_min: usize,
    /// Where the patterns came from (`None` when parsed from memory).
    pub source: Option<PathBuf>,
}

impl Hyphenator {
    /// Parse a hyphenation file's text (already decoded; the encoding line is skipped).
    pub fn parse(text: &str) -> Hyphenator {
        let mut h =
            Hyphenator { levels: vec![Level::default()], left_min: 2, right_min: 2, compound_left_min: 2, compound_right_min: 2, source: None };
        for (i, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('%') {
                continue;
            }
            // The first line names the encoding (`ISO8859-1`, `UTF-8`); patterns are lowercase.
            if i == 0 && line.starts_with(|c: char| c.is_ascii_uppercase()) && !line.contains(' ') {
                continue;
            }
            let mut parts = line.split_whitespace();
            let first = parts.next().unwrap_or("");
            let num = |p: Option<&str>| p.and_then(|x| x.parse::<usize>().ok()).map(|n| n.min(16));
            match first {
                "NEXTLEVEL" => h.levels.push(Level::default()),
                "LEFTHYPHENMIN" => h.left_min = num(parts.next()).unwrap_or(2),
                "RIGHTHYPHENMIN" => h.right_min = num(parts.next()).unwrap_or(2),
                "COMPOUNDLEFTHYPHENMIN" => h.compound_left_min = num(parts.next()).unwrap_or(2),
                "COMPOUNDRIGHTHYPHENMIN" => h.compound_right_min = num(parts.next()).unwrap_or(2),
                "NOHYPHEN" | "NOHYPHENS" => {}
                _ if first.chars().all(|c| c.is_uppercase() || c == '_') && first.len() > 2 => {}
                _ => {
                    for tok in std::iter::once(first).chain(parts) {
                        // Non-standard hyphenation (`ck1/k=k,1,2`) is skipped.
                        if !tok.contains('/')
                            && let Some(lv) = h.levels.last_mut()
                        {
                            lv.insert(tok);
                        }
                    }
                }
            }
        }
        h.levels.retain(|l| !l.map.is_empty());
        h
    }

    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }

    /// Break points (char indices; a hyphen goes before that char) of one lowercase word made of
    /// letters only. The paragraph's limits are applied by the caller.
    pub fn points(&self, word: &[char]) -> Vec<usize> {
        let n = word.len();
        if !(2..=100).contains(&n) {
            return Vec::new();
        }
        let r = catch_unwind(AssertUnwindSafe(|| {
            let (Some(first), rest) = (self.levels.first(), self.levels.get(1..).unwrap_or_default()) else { return Vec::new() };
            let mut pts = first.points(word);
            if let Some(second) = rest.first() {
                // Compound boundaries from level one, then syllables inside each part.
                let bounds: Vec<usize> = std::iter::once(0).chain(pts.iter().copied()).chain(std::iter::once(n)).collect();
                for pair in bounds.windows(2) {
                    let [a, b] = pair else { continue };
                    let Some(part) = word.get(*a..*b) else { continue };
                    for p in second.points(part) {
                        let (l, r) = (p, part.len() - p);
                        let lmin = if *a == 0 { self.left_min } else { self.compound_left_min };
                        let rmin = if *b == n { self.right_min } else { self.compound_right_min };
                        if l >= lmin && r >= rmin {
                            pts.push(a + p);
                        }
                    }
                }
                pts.sort_unstable();
                pts.dedup();
            }
            pts.retain(|&p| p >= self.left_min.max(1) && n - p >= self.right_min.max(1));
            pts
        }));
        r.unwrap_or_default()
    }
}

/// Hyphenation pattern files on this machine: (language stem such as `de_DE`, path).
pub fn available() -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    for dir in files::search_dirs() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut paths: Vec<PathBuf> = rd.flatten().map(|e| e.path()).take(5000).collect();
        paths.sort();
        for p in paths {
            let Some(name) = p.file_name().and_then(|s| s.to_str()) else { continue };
            if let Some(stem) = name.strip_prefix("hyph_").and_then(|s| s.strip_suffix(".dic"))
                && !out.iter().any(|(s, _)| s == stem)
            {
                out.push((stem.to_string(), p.clone()));
            }
        }
    }
    out
}

fn load(path: &std::path::Path) -> Option<Hyphenator> {
    let bytes = files::read(path)?;
    let first = bytes.split(|b| *b == b'\n').next().unwrap_or_default();
    let enc = String::from_utf8_lossy(first).trim().to_string();
    let enc = if enc.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') { enc } else { String::new() };
    let text = files::decode(&bytes, &enc)?;
    let mut h = Hyphenator::parse(&text);
    if h.is_empty() {
        return None;
    }
    h.source = Some(path.to_path_buf());
    log::info!("hyphenation patterns loaded from {}", path.display());
    Some(h)
}

enum Slot {
    Loading,
    Ready(Arc<Hyphenator>),
    Missing,
}

fn slots() -> &'static RwLock<HashMap<String, Slot>> {
    static S: OnceLock<RwLock<HashMap<String, Slot>>> = OnceLock::new();
    S.get_or_init(Default::default)
}

fn key(lang: &str) -> String {
    lang.split(['.', '@']).next().unwrap_or("").replace('_', "-").to_ascii_lowercase()
}

/// Use patterns given as text for `lang` (the web, tests).
pub fn register(lang: &str, text: &str) {
    let h = Hyphenator::parse(text);
    if let Ok(mut s) = slots().write() {
        s.insert(key(lang), Slot::Ready(Arc::new(h)));
    }
    files::bump();
}

/// The hyphenator for `lang`; like [`crate::hunspell::speller`], without `wait` a pattern file
/// loads in the background and `None` comes back meanwhile.
pub fn hyphenator(lang: &str, wait: bool) -> Option<Arc<Hyphenator>> {
    let k = key(lang);
    if k.is_empty() || k.len() > 35 {
        return None;
    }
    loop {
        match slots().read().ok()?.get(&k) {
            Some(Slot::Ready(h)) => return Some(h.clone()),
            Some(Slot::Missing) => return None,
            Some(Slot::Loading) if wait => {}
            Some(Slot::Loading) => return None,
            None => break,
        }
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::sleep(std::time::Duration::from_millis(10));
        #[cfg(target_arch = "wasm32")]
        return None;
    }
    let all = available();
    let names: Vec<String> = all.iter().map(|(s, _)| s.clone()).collect();
    let path = files::stems(&k, &names).into_iter().find_map(|s| all.iter().find(|(n, _)| *n == s).map(|(_, p)| p.clone()));
    {
        let mut s = slots().write().ok()?;
        if s.contains_key(&k) {
            drop(s);
            return hyphenator(lang, wait);
        }
        s.insert(k.clone(), if path.is_some() { Slot::Loading } else { Slot::Missing });
    }
    let path = path?;
    let finish = move |k: String, path: PathBuf| {
        let slot = load(&path).map_or(Slot::Missing, |h| Slot::Ready(Arc::new(h)));
        if let Ok(mut s) = slots().write() {
            s.insert(k, slot);
        }
        files::bump();
    };
    #[cfg(not(target_arch = "wasm32"))]
    if !wait {
        let guard = files::BusyGuard::new();
        let spawned = std::thread::Builder::new().name("hyphenation".into()).spawn(move || {
            let _guard = guard;
            finish(k, path);
        });
        if spawned.is_err()
            && let Ok(mut s) = slots().write()
        {
            s.insert(key(lang), Slot::Missing);
        }
        return None;
    }
    finish(k.clone(), path);
    match slots().read().ok()?.get(&k) {
        Some(Slot::Ready(h)) => Some(h.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cut(h: &Hyphenator, w: &str) -> String {
        let chars: Vec<char> = w.chars().collect();
        let pts = h.points(&chars);
        chars.iter().enumerate().map(|(i, c)| if pts.contains(&i) { format!("-{c}") } else { c.to_string() }).collect()
    }

    #[test]
    fn two_level_patterns() {
        // Level 1 splits the compound "hausboot"; level 2 hyphenates "boote" inside it.
        let h = Hyphenator::parse("UTF-8\nLEFTHYPHENMIN 2\nRIGHTHYPHENMIN 2\ns1b\nNEXTLEVEL\no1t\n");
        assert_eq!(cut(&h, "hausboote"), "haus-boo-te");
        assert_eq!(cut(&h, "haus"), "haus");
        // Long patterns are fine.
        let long = Hyphenator::parse("UTF-8\ndonaudampfschiff1fahrt\n");
        assert_eq!(cut(&long, "donaudampfschifffahrt"), "donaudampfschiff-fahrt");
    }

    #[test]
    fn hostile_input_is_harmless() {
        let h = Hyphenator::parse("UTF-8\n9\n1\n.1a1.\nLEFTHYPHENMIN 999999999999\nab/c=d,1,1\n");
        let _ = cut(&h, "aaaa");
        let _ = cut(&h, "");
        assert!(hyphenator("../../etc", true).is_none());
    }

    #[test]
    fn installed_german_patterns() {
        let Some(h) = hyphenator("de-DE", true) else { return };
        let w = |s: &str| cut(&h, s);
        assert!(w("donaudampfschifffahrtsgesellschaft").matches('-').count() >= 6, "{}", w("donaudampfschifffahrtsgesellschaft"));
        assert!(w("silbentrennung").contains("sil-ben"), "{}", w("silbentrennung"));
        assert!(w("rechtschreibprüfung").contains("recht-schreib"), "{}", w("rechtschreibprüfung"));
    }
}
