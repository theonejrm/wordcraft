//! Spelling in languages with a Hunspell dictionary (German, French, …) via [`spellbook`]: the
//! same dictionaries LibreOffice and Firefox use, found as described in [`crate::files`].
//!
//! Dictionaries load on first use. The layout asks without waiting (a dictionary still loading
//! means "not checked yet", and [`crate::files::generation`] changes when it is ready); commands
//! such as Spelling & Grammar wait. Dictionary files are untrusted input: parsing and checking
//! run behind `catch_unwind`, and a broken file only means no checking for that language.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};

use crate::files;

/// A dictionary file pair found on this machine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    /// File stem: `de_DE`, `de_DE_frami`, `en_US`.
    pub name: String,
    pub aff: PathBuf,
    pub dic: PathBuf,
}

/// A loaded dictionary.
pub struct Speller {
    dict: spellbook::Dictionary,
    /// Where it came from (`None` when registered from memory).
    pub source: Option<Found>,
}

enum Slot {
    Loading,
    Ready(Arc<Speller>),
    Missing,
}

fn slots() -> &'static RwLock<HashMap<String, Slot>> {
    static S: OnceLock<RwLock<HashMap<String, Slot>>> = OnceLock::new();
    S.get_or_init(Default::default)
}

fn found_cache() -> &'static RwLock<Option<Vec<Found>>> {
    static F: OnceLock<RwLock<Option<Vec<Found>>>> = OnceLock::new();
    F.get_or_init(Default::default)
}

/// `de_DE.UTF-8`, `de-de` → `de-DE`-style key (lowercase).
fn key(lang: &str) -> String {
    lang.split(['.', '@']).next().unwrap_or("").replace('_', "-").to_ascii_lowercase()
}

/// Every Hunspell dictionary on this machine (scanned once; [`rescan`] looks again).
pub fn available() -> Vec<Found> {
    if let Ok(g) = found_cache().read()
        && let Some(v) = g.as_ref()
    {
        return v.clone();
    }
    let mut out: Vec<Found> = Vec::new();
    for dir in files::search_dirs() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut names: Vec<PathBuf> = rd.flatten().map(|e| e.path()).take(5000).collect();
        names.sort();
        for dic in names {
            if dic.extension().and_then(|e| e.to_str()) != Some("dic") {
                continue;
            }
            let Some(stem) = dic.file_stem().and_then(|s| s.to_str()).map(str::to_string) else { continue };
            // Hyphenation patterns and thesauri share the extension.
            if stem.starts_with("hyph_") || stem.starts_with("th_") {
                continue;
            }
            let aff = dic.with_extension("aff");
            if aff.is_file() && !out.iter().any(|f| f.name == stem) {
                out.push(Found { name: stem, aff, dic });
            }
        }
    }
    if let Ok(mut g) = found_cache().write() {
        *g = Some(out.clone());
    }
    out
}

/// Forget what was found and loaded (after the user adds dictionaries).
pub fn rescan() {
    if let Ok(mut g) = found_cache().write() {
        *g = None;
    }
    if let Ok(mut s) = slots().write() {
        s.retain(|_, v| matches!(v, Slot::Loading) || matches!(v, Slot::Ready(sp) if sp.source.is_none()));
    }
    files::bump();
}

/// The dictionary file pair for a language tag, if one is installed.
pub fn locate(lang: &str) -> Option<Found> {
    let all = available();
    let names: Vec<String> = all.iter().map(|f| f.name.clone()).collect();
    files::stems(lang, &names).into_iter().find_map(|s| all.iter().find(|f| f.name == s).cloned())
}

fn encoding_of(aff: &[u8]) -> String {
    // `SET` is ASCII in every encoding; look at the first few KB only.
    let head = String::from_utf8_lossy(aff.get(..aff.len().min(8192)).unwrap_or_default()).into_owned();
    head.lines().find_map(|l| l.trim().strip_prefix("SET ").map(|e| e.trim().to_string())).unwrap_or_default()
}

fn build(aff: &str, dic: &str) -> Option<spellbook::Dictionary> {
    match catch_unwind(AssertUnwindSafe(|| spellbook::Dictionary::new(aff, dic))) {
        Ok(Ok(d)) => Some(d),
        Ok(Err(e)) => {
            log::warn!("dictionary not loaded: {e}");
            None
        }
        Err(_) => {
            log::error!("dictionary parser failed; that language isn't checked");
            None
        }
    }
}

fn load(found: &Found) -> Option<Speller> {
    let aff_bytes = files::read(&found.aff)?;
    let dic_bytes = files::read(&found.dic)?;
    let enc = encoding_of(&aff_bytes);
    let Some(aff) = files::decode(&aff_bytes, &enc) else {
        log::warn!("{}: encoding {enc} isn't supported yet", found.aff.display());
        return None;
    };
    let dic = files::decode(&dic_bytes, &enc)?;
    let dict = build(&aff, &dic)?;
    log::info!("spelling dictionary {} loaded from {}", found.name, found.dic.display());
    Some(Speller { dict, source: Some(found.clone()) })
}

/// Use a dictionary given as text for `lang` (the web, tests), replacing any loaded one.
pub fn register(lang: &str, aff: &str, dic: &str) -> Result<(), String> {
    let dict = build(aff, dic).ok_or_else(|| "the dictionary could not be parsed".to_string())?;
    if let Ok(mut s) = slots().write() {
        s.insert(key(lang), Slot::Ready(Arc::new(Speller { dict, source: None })));
    }
    files::bump();
    Ok(())
}

/// The speller for `lang`. Without `wait`, a dictionary that isn't loaded yet starts loading in
/// the background and `None` comes back until it is ready.
pub fn speller(lang: &str, wait: bool) -> Option<Arc<Speller>> {
    let k = key(lang);
    if k.is_empty() || k.len() > 35 {
        return None;
    }
    loop {
        match slots().read().ok()?.get(&k) {
            Some(Slot::Ready(s)) => return Some(s.clone()),
            Some(Slot::Missing) => return None,
            Some(Slot::Loading) if wait => {}
            Some(Slot::Loading) => return None,
            None => break,
        }
        // Another thread is loading it.
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::sleep(std::time::Duration::from_millis(10));
        #[cfg(target_arch = "wasm32")]
        return None;
    }
    let found = locate(&k);
    let Some(found) = found else {
        if let Ok(mut s) = slots().write() {
            s.insert(k, Slot::Missing);
        }
        return None;
    };
    {
        let mut s = slots().write().ok()?;
        if s.contains_key(&k) {
            drop(s);
            return speller(lang, wait);
        }
        s.insert(k.clone(), Slot::Loading);
    }
    let finish = move |k: String, found: Found| {
        let slot = match load(&found) {
            Some(sp) => Slot::Ready(Arc::new(sp)),
            None => Slot::Missing,
        };
        if let Ok(mut s) = slots().write() {
            s.insert(k, slot);
        }
        files::bump();
    };
    #[cfg(not(target_arch = "wasm32"))]
    if !wait {
        let guard = files::BusyGuard::new();
        let spawned = std::thread::Builder::new().name("dictionary".into()).spawn(move || {
            let _guard = guard;
            finish(k, found);
        });
        if spawned.is_err()
            && let Ok(mut s) = slots().write()
        {
            s.insert(key(lang), Slot::Missing);
        }
        return None;
    }
    finish(k.clone(), found);
    match slots().read().ok()?.get(&k) {
        Some(Slot::Ready(s)) => Some(s.clone()),
        _ => None,
    }
}

impl Speller {
    fn raw(&self, word: &str) -> bool {
        catch_unwind(AssertUnwindSafe(|| self.dict.check(word))).unwrap_or(true)
    }

    /// Is `word` spelled correctly? Typographic apostrophes count as `'`, and elisions such as
    /// `geht’s` or `hab’` are accepted when the full form is.
    pub fn check(&self, word: &str) -> bool {
        let w = word.replace(['’', 'ʼ'], "'");
        if self.raw(&w) {
            return true;
        }
        if let Some(stem) = w.strip_suffix("'s").filter(|s| s.chars().count() >= 2) {
            return self.raw(stem);
        }
        if let Some(stem) = w.strip_suffix('\'').filter(|s| s.chars().count() >= 2) {
            return self.raw(&format!("{stem}e")) || self.raw(stem);
        }
        false
    }

    /// Suggestions for a misspelled word (best first, at most `max`).
    pub fn suggest(&self, word: &str, max: usize) -> Vec<String> {
        if word.chars().count() > 60 {
            return Vec::new();
        }
        let mut out = Vec::new();
        if catch_unwind(AssertUnwindSafe(|| self.dict.suggest(word, &mut out))).is_err() {
            return Vec::new();
        }
        out.truncate(max);
        out
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A tiny German-like dictionary: compounds and a plural suffix.
    pub(crate) const AFF: &str =
        "SET UTF-8\nTRY esianrtolcdugmphbyfvkwzESIANRTOLCDUGMPHBYFVKWZ\nCOMPOUNDMIN 3\nCOMPOUNDFLAG X\nSFX P Y 1\nSFX P 0 en .\n";
    pub(crate) const DIC: &str = "5\nhaus/XP\ntür/XP\nschlüssel/X\ngeht\nGrüße\n";

    #[test]
    fn registered_dictionaries_check_compounds_and_suggest() {
        register("xx-TEST", AFF, DIC).unwrap();
        let s = speller("xx_test", false).unwrap();
        for w in ["haus", "häuser", "haustür", "schlüssel", "geht", "geht’s", "Grüße"] {
            let ok = s.check(w);
            assert_eq!(ok, w != "häuser", "{w}");
        }
        assert!(!s.check("hasu"));
        assert!(s.suggest("hasu", 3).contains(&"haus".to_string()));
        // A broken dictionary is an error, not a crash.
        let _ = register("xx-bad", "FLAG long\nSFX", "notanumber\n\u{0}");
    }

    #[test]
    fn unknown_languages_have_no_speller() {
        assert!(speller("zz-ZZ", true).is_none());
        assert!(speller("../../etc/passwd", true).is_none());
        assert!(speller("", true).is_none());
    }

    /// The real German dictionary, when this machine has one.
    #[test]
    fn installed_german_dictionary() {
        let Some(s) = speller("de-DE", true) else { return };
        for w in ["Nachbarschaftsfest", "Grüße", "Donaudampfschifffahrtsgesellschaft", "Straße", "geht’s", "gibt's"] {
            assert!(s.check(w), "{w}");
        }
        for w in ["Strasse", "Nachbarschafft", "Teh"] {
            assert!(!s.check(w), "{w}");
        }
        assert!(s.suggest("Nachbarschafft", 5).contains(&"Nachbarschaft".to_string()));
    }
}
