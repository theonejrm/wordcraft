//! Where proofing data for languages beyond the built-in English comes from, and the state the
//! layout watches while it loads.
//!
//! WordCraft bundles no third-party dictionaries. It uses the Hunspell dictionaries (`xx_YY.aff` +
//! `xx_YY.dic`) and hyphenation patterns (`hyph_xx_YY.dic`) found in, in order:
//! 1. `WORDCRAFT_DICTIONARIES` (a path list, like `PATH`);
//! 2. the user's folder: `~/.local/share/wordcraft/dictionaries` (or `$XDG_DATA_HOME`),
//!    `~/Library/Application Support/WordCraft/dictionaries` on macOS,
//!    `%APPDATA%\WordCraft\dictionaries` on Windows;
//! 3. the system's: `/usr/share/hunspell`, `/usr/share/myspell`, `/usr/share/hyphen` (and the
//!    `/usr/local`, Homebrew and `~/Library/Spelling` equivalents);
//! 4. an installed LibreOffice's dictionary extensions.
//!
//! On the web nothing is searched; dictionaries can be registered from bytes.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

static GENERATION: AtomicU64 = AtomicU64::new(0);
static BUSY: AtomicUsize = AtomicUsize::new(0);

/// Changes whenever proofing results may have changed without the text changing (a dictionary
/// finished loading, a grammar server answered). Layout caches include it.
pub fn generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

pub(crate) fn bump() {
    GENERATION.fetch_add(1, Ordering::Relaxed);
}

/// Is proofing work running in the background (dictionaries loading, grammar requests)? The UI
/// keeps repainting while it is, so results appear without input.
pub fn busy() -> bool {
    BUSY.load(Ordering::Relaxed) > 0
}

/// Counts one piece of background work for [`busy`] while alive.
pub(crate) struct BusyGuard;

impl BusyGuard {
    pub(crate) fn new() -> BusyGuard {
        BUSY.fetch_add(1, Ordering::Relaxed);
        BusyGuard
    }
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        BUSY.fetch_sub(1, Ordering::Relaxed);
    }
}

/// The largest dictionary or pattern file read (the biggest real ones are a few MB).
pub const MAX_FILE: u64 = 64 << 20;

/// The user's dictionary folder (created by the user; listed in File ▸ Options).
pub fn user_dir() -> Option<PathBuf> {
    #[cfg(target_arch = "wasm32")]
    {
        None
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
        if cfg!(target_os = "windows") {
            return env("APPDATA").map(|d| d.join("WordCraft").join("dictionaries"));
        }
        if cfg!(target_os = "macos") {
            return env("HOME").map(|h| h.join("Library/Application Support/WordCraft/dictionaries"));
        }
        env("XDG_DATA_HOME").or_else(|| env("HOME").map(|h| h.join(".local/share"))).map(|d| d.join("wordcraft").join("dictionaries"))
    }
}

/// Every folder searched, in priority order (existing ones only).
pub fn search_dirs() -> Vec<PathBuf> {
    #[cfg(target_arch = "wasm32")]
    {
        Vec::new()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut dirs: Vec<PathBuf> = Vec::new();
        if let Some(list) = std::env::var_os("WORDCRAFT_DICTIONARIES") {
            dirs.extend(std::env::split_paths(&list));
        }
        dirs.extend(user_dir());
        let home = std::env::var_os("HOME").map(PathBuf::from);
        for d in [
            "/usr/share/hunspell",
            "/usr/share/myspell",
            "/usr/share/myspell/dicts",
            "/usr/share/hyphen",
            "/usr/local/share/hunspell",
            "/usr/local/share/myspell",
            "/usr/local/share/hyphen",
            "/opt/homebrew/share/hunspell",
            "/usr/pkg/share/hunspell",
        ] {
            dirs.push(PathBuf::from(d));
        }
        if let Some(h) = &home {
            dirs.push(h.join("Library/Spelling"));
        }
        // LibreOffice ships its dictionaries as extensions (dict-de, dict-en, …).
        let mut office: Vec<PathBuf> =
            ["/usr/lib/libreoffice/share/extensions", "/usr/lib64/libreoffice/share/extensions", "/opt/libreoffice/share/extensions"]
                .iter()
                .map(PathBuf::from)
                .collect();
        office.push(PathBuf::from("/Applications/LibreOffice.app/Contents/Resources/extensions"));
        for var in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(p) = std::env::var_os(var) {
                office.push(PathBuf::from(p).join("LibreOffice").join("share").join("extensions"));
            }
        }
        for o in office {
            if let Ok(rd) = std::fs::read_dir(&o) {
                let mut subs: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.is_dir()).take(200).collect();
                subs.sort();
                dirs.extend(subs);
            }
        }
        let mut seen = std::collections::HashSet::new();
        dirs.into_iter().filter(|d| d.is_dir() && seen.insert(d.clone())).collect()
    }
}

/// Read a proofing file (size-capped).
pub(crate) fn read(path: &std::path::Path) -> Option<Vec<u8>> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > MAX_FILE {
        log::warn!("skipping {}: not a file or larger than {} MB", path.display(), MAX_FILE >> 20);
        return None;
    }
    std::fs::read(path).ok()
}

/// Decode a dictionary file in one of the encodings Hunspell files use (`SET` in the `.aff`, or
/// the first line of a hyphenation file). `None` for encodings WordCraft can't read yet.
pub fn decode(bytes: &[u8], encoding: &str) -> Option<String> {
    let e = encoding.trim().to_ascii_uppercase().replace(['-', '_'], "");
    match e.as_str() {
        "" | "UTF8" => Some(String::from_utf8_lossy(bytes).into_owned()),
        "ISO88591" | "LATIN1" => Some(bytes.iter().map(|&b| char::from(b)).collect()),
        "ISO885915" | "LATIN9" => Some(
            bytes
                .iter()
                .map(|&b| match b {
                    0xA4 => '€',
                    0xA6 => 'Š',
                    0xA8 => 'š',
                    0xB4 => 'Ž',
                    0xB8 => 'ž',
                    0xBC => 'Œ',
                    0xBD => 'œ',
                    0xBE => 'Ÿ',
                    _ => char::from(b),
                })
                .collect(),
        ),
        "CP1252" | "WINDOWS1252" => Some(
            bytes
                .iter()
                .map(|&b| match b {
                    0x80 => '€',
                    0x8A => 'Š',
                    0x8C => 'Œ',
                    0x8E => 'Ž',
                    0x91 => '‘',
                    0x92 => '’',
                    0x93 => '“',
                    0x94 => '”',
                    0x96 => '–',
                    0x97 => '—',
                    0x9A => 'š',
                    0x9C => 'œ',
                    0x9E => 'ž',
                    0x9F => 'Ÿ',
                    _ => char::from(b),
                })
                .collect(),
        ),
        _ => None,
    }
}

/// File-name stems to look for, most specific first: `de-AT` → `de_AT`, `de-AT`, `de_AT_frami`,
/// then any `de_*` file (Germany first), then `de`.
pub(crate) fn stems(lang: &str, present: &[String]) -> Vec<String> {
    let base = lang.split(['.', '@']).next().unwrap_or("").replace('-', "_");
    let mut parts = base.split('_').filter(|p| !p.is_empty());
    let primary = parts.next().unwrap_or("").to_ascii_lowercase();
    if primary.is_empty() || !primary.chars().all(|c| c.is_ascii_alphabetic()) {
        return Vec::new();
    }
    let region = parts.find(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_alphabetic())).map(str::to_ascii_uppercase);
    let mut out = Vec::new();
    if let Some(r) = &region {
        out.push(format!("{primary}_{r}"));
        out.push(format!("{primary}-{r}"));
        out.push(format!("{primary}_{r}_frami"));
    }
    // The language's main region first (de_DE for German), then the others in name order.
    let main = format!("{primary}_{}", primary.to_ascii_uppercase());
    let mut others: Vec<&String> =
        present.iter().filter(|s| s.starts_with(&format!("{primary}_")) || s.starts_with(&format!("{primary}-"))).collect();
    others.sort_by_key(|s| (!s.starts_with(&main), s.contains("frami"), (*s).clone()));
    out.extend(others.into_iter().cloned());
    out.push(primary);
    let mut seen = std::collections::HashSet::new();
    out.retain(|s| seen.insert(s.clone()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodings() {
        assert_eq!(decode(b"Gr\xfc\xdfe", "ISO8859-1").as_deref(), Some("Grüße"));
        assert_eq!(decode(b"\xa4", "ISO8859-15").as_deref(), Some("€"));
        assert_eq!(decode("Grüße".as_bytes(), "UTF-8").as_deref(), Some("Grüße"));
        assert_eq!(decode(b"x", "KOI8-R"), None);
    }

    #[test]
    fn file_stems() {
        let present = vec!["de_AT".to_string(), "de_DE".to_string(), "en_US".to_string()];
        let s = stems("de-CH", &present);
        assert_eq!(s.first().map(String::as_str), Some("de_CH"));
        // No Swiss dictionary: Germany's before Austria's.
        let after: Vec<&str> = s.iter().skip(3).map(String::as_str).collect();
        assert_eq!(after, ["de_DE", "de_AT", "de"]);
        assert_eq!(stems("de_DE.UTF-8", &present).first().map(String::as_str), Some("de_DE"));
        assert!(stems("../etc", &present).is_empty());
        assert!(stems("", &present).is_empty());
    }
}
