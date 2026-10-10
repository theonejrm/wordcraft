//! Grammar and style checking by a LanguageTool server the user runs (LanguageTool is a separate,
//! LGPL-licensed program; WordCraft only talks to it over HTTP). Off until configured with
//! [`configure`]; meant for a server on the same machine, so text never leaves it.
//!
//! The layout asks per paragraph without waiting: the first request for a text goes to a
//! background thread and its result shows up later ([`crate::files::generation`] changes).
//! Results are cached by language and text. Spelling stays with the Hunspell dictionaries, so
//! LanguageTool's own spelling matches are dropped.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::{Issue, IssueKind};

/// The server answered, failed, or hasn't been asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Off,
    Unknown,
    Ok,
    Failed(String),
}

#[derive(Clone)]
enum Entry {
    Pending,
    Done(Vec<Issue>),
    Failed,
}

struct State {
    url: Option<String>,
    status: Status,
    cache: HashMap<(String, u64), Entry>,
    /// Text hash → the answer count when its latest answer arrived ([`stamp`]).
    stamps: HashMap<u64, u64>,
    #[cfg(not(target_arch = "wasm32"))]
    queue: Option<std::sync::mpsc::Sender<Job>>,
}

fn state() -> &'static Mutex<State> {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    S.get_or_init(|| {
        Mutex::new(State {
            url: None,
            status: Status::Off,
            cache: HashMap::new(),
            stamps: HashMap::new(),
            #[cfg(not(target_arch = "wasm32"))]
            queue: None,
        })
    })
}

const MAX_CACHE: usize = 4000;

static ANSWERS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// How many answers have arrived. When it changes, layouts are rebuilt, but only paragraphs
/// whose [`stamp`] changed are laid out again.
pub fn answers() -> u64 {
    ANSWERS.load(std::sync::atomic::Ordering::Relaxed)
}

/// Changes when an answer for this paragraph text arrives (0: none yet, or the server is off).
pub fn stamp(text: &str) -> u64 {
    let Ok(s) = state().lock() else { return 0 };
    if s.url.is_none() || s.stamps.is_empty() {
        return 0;
    }
    s.stamps.get(&hash(text)).copied().unwrap_or(0)
}
/// Paragraphs longer than this aren't sent.
const MAX_TEXT: usize = 20_000;

fn hash(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

/// Check a server address: `http://host[:port][/path]`, no credentials. Returns it without a
/// trailing slash.
pub fn normalize_url(url: &str) -> Result<String, String> {
    let u = url.trim().trim_end_matches('/');
    let rest = u.strip_prefix("http://").ok_or_else(|| "use an http:// address, e.g. http://localhost:8081".to_string())?;
    let host = rest.split('/').next().unwrap_or("");
    if host.is_empty() || host.contains('@') || u.len() > 300 || u.chars().any(char::is_whitespace) {
        return Err(format!("`{url}` isn't a server address"));
    }
    Ok(u.to_string())
}

/// Turn LanguageTool on for `url` (`http://localhost:8081`), or off with `None`.
pub fn configure(url: Option<&str>) -> Result<(), String> {
    let url = url.map(normalize_url).transpose()?;
    let mut s = state().lock().map_err(|_| "proofing state unavailable".to_string())?;
    if s.url != url {
        s.cache.clear();
        s.stamps.clear();
    }
    s.status = if url.is_some() { Status::Unknown } else { Status::Off };
    s.url = url;
    drop(s);
    crate::files::bump();
    Ok(())
}

/// The configured server, if any.
pub fn url() -> Option<String> {
    state().lock().ok().and_then(|s| s.url.clone())
}

/// How the last request went.
pub fn status() -> Status {
    state().lock().map(|s| s.status.clone()).unwrap_or(Status::Off)
}

fn set_status(st: Status) {
    if let Ok(mut s) = state().lock()
        && s.url.is_some()
    {
        s.status = st;
    }
}

/// Grammar issues for `text` in `lang` from the server: `Some` when known (cached), `None` when
/// off, failed, or (without `wait`) still being checked.
pub fn grammar(text: &str, lang: &str, wait: bool) -> Option<Vec<Issue>> {
    if text.trim().is_empty() {
        return Some(Vec::new());
    }
    if text.len() > MAX_TEXT || lang.is_empty() || lang.len() > 35 {
        return None;
    }
    let key = (lang.to_ascii_lowercase(), hash(text));
    let url = {
        let mut s = state().lock().ok()?;
        let url = s.url.clone()?;
        if matches!(s.status, Status::Failed(_)) && !wait {
            return None;
        }
        match s.cache.get(&key) {
            Some(Entry::Done(v)) => return Some(v.clone()),
            Some(Entry::Failed) if !wait => return None,
            Some(Entry::Pending) if !wait => return None,
            _ => {}
        }
        if s.cache.len() > MAX_CACHE {
            s.cache.clear();
            s.stamps.clear();
        }
        if !wait {
            s.cache.insert(key.clone(), Entry::Pending);
        }
        url
    };
    #[cfg(target_arch = "wasm32")]
    {
        let _ = url;
        None
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if wait {
            let r = http::check(&url, text, lang);
            store(&key, &r);
            return r.ok();
        }
        enqueue(lang.to_string(), text.to_string(), key.1);
        None
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn store(key: &(String, u64), r: &Result<Vec<Issue>, String>) {
    let n = ANSWERS.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
    if let Ok(mut s) = state().lock() {
        s.stamps.insert(key.1, n);
    }
    match r {
        Ok(v) => {
            set_status(Status::Ok);
            if let Ok(mut s) = state().lock() {
                s.cache.insert(key.clone(), Entry::Done(v.clone()));
            }
        }
        Err(e) => {
            log::warn!("LanguageTool: {e}");
            set_status(Status::Failed(e.clone()));
            if let Ok(mut s) = state().lock() {
                s.cache.insert(key.clone(), Entry::Failed);
            }
        }
    }
}

/// A queued request; it counts as background work from the moment it is queued.
#[cfg(not(target_arch = "wasm32"))]
struct Job {
    lang: String,
    text: String,
    hash: u64,
    _busy: crate::files::BusyGuard,
}

#[cfg(not(target_arch = "wasm32"))]
fn enqueue(lang: String, text: String, h: u64) {
    let Ok(mut s) = state().lock() else { return };
    if s.queue.is_none() {
        let (tx, rx) = std::sync::mpsc::channel::<Job>();
        let spawned = std::thread::Builder::new().name("languagetool".into()).spawn(move || {
            while let Ok(job) = rx.recv() {
                let key = (job.lang.to_ascii_lowercase(), job.hash);
                let Some(url) = url() else {
                    if let Ok(mut s) = state().lock() {
                        s.cache.remove(&key);
                    }
                    continue;
                };
                let r = http::check(&url, &job.text, &job.lang);
                store(&key, &r);
            }
        });
        if spawned.is_err() {
            return;
        }
        s.queue = Some(tx);
    }
    if let Some(q) = &s.queue {
        let _ = q.send(Job { lang, text, hash: h, _busy: crate::files::BusyGuard::new() });
    }
}

/// Ask the server which languages it has (a quick reachability check).
pub fn ping(url: &str) -> Result<usize, String> {
    let url = normalize_url(url)?;
    #[cfg(target_arch = "wasm32")]
    {
        let _ = url;
        Err("LanguageTool isn't available on the web".into())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let body = http::request(&url, "GET", "/v2/languages", None)?;
        let v: serde_json::Value = serde_json::from_str(&body).map_err(|e| format!("unexpected answer: {e}"))?;
        let n = v.as_array().map_or(0, Vec::len);
        set_status(Status::Ok);
        Ok(n)
    }
}

/// Byte offset in `text` of a UTF-16 offset (LanguageTool counts Java chars).
fn byte_at_utf16(text: &str, units: usize) -> Option<usize> {
    let mut u = 0;
    for (i, c) in text.char_indices() {
        if u >= units {
            return (u == units).then_some(i);
        }
        u += c.len_utf16();
    }
    (u == units).then_some(text.len())
}

/// Issues from a LanguageTool `/v2/check` answer.
pub fn parse_matches(text: &str, body: &str) -> Result<Vec<Issue>, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|e| format!("unexpected answer: {e}"))?;
    let matches = v.get("matches").and_then(|m| m.as_array()).ok_or("unexpected answer: no matches")?;
    let mut out = Vec::new();
    for m in matches.iter().take(2000) {
        let rule = m.get("rule");
        let issue_type = rule.and_then(|r| r.get("issueType")).and_then(|t| t.as_str()).unwrap_or("");
        let category = rule.and_then(|r| r.get("category")).and_then(|c| c.get("id")).and_then(|c| c.as_str()).unwrap_or("");
        if issue_type == "misspelling" || category == "TYPOS" {
            continue;
        }
        let (Some(off), Some(len)) = (m.get("offset").and_then(|x| x.as_u64()), m.get("length").and_then(|x| x.as_u64())) else { continue };
        let (Ok(off), Ok(len)) = (usize::try_from(off), usize::try_from(len)) else { continue };
        let (Some(start), Some(end)) = (byte_at_utf16(text, off), byte_at_utf16(text, off.saturating_add(len))) else { continue };
        if end <= start {
            continue;
        }
        let message = m.get("message").and_then(|x| x.as_str()).unwrap_or("").chars().take(500).collect();
        let suggestions = m
            .get("replacements")
            .and_then(|r| r.as_array())
            .map(|r| r.iter().filter_map(|x| x.get("value").and_then(|v| v.as_str())).take(6).map(str::to_string).collect())
            .unwrap_or_default();
        out.push(Issue { start, end, kind: IssueKind::Grammar, message, suggestions });
    }
    Ok(out)
}

#[cfg(not(target_arch = "wasm32"))]
mod http {
    //! A minimal HTTP/1.1 client for a LanguageTool server (plain http, one request per
    //! connection, `Content-Length` or chunked answers).

    use std::io::{Read, Write};
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;

    const MAX_ANSWER: usize = 8 << 20;

    fn form(s: &str) -> String {
        let mut out = String::with_capacity(s.len() * 3 / 2);
        for b in s.bytes() {
            match b {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(char::from(b)),
                b' ' => out.push('+'),
                _ => out.push_str(&format!("%{b:02X}")),
            }
        }
        out
    }

    pub(super) fn check(url: &str, text: &str, lang: &str) -> Result<Vec<super::Issue>, String> {
        // Objects in the paragraph are U+0001 placeholders; a space keeps the offsets.
        let sent = text.replace('\u{1}', " ");
        let body = format!("language={}&text={}", form(lang), form(&sent));
        let answer = request(url, "POST", "/v2/check", Some(&body))?;
        super::parse_matches(&sent, &answer)
    }

    pub(super) fn request(url: &str, method: &str, path: &str, body: Option<&str>) -> Result<String, String> {
        let rest = url.strip_prefix("http://").ok_or("not an http:// address")?;
        let (hostport, base) = rest.split_once('/').map_or((rest, ""), |(h, p)| (h, p));
        let addr_str = if hostport.contains(':') && !hostport.ends_with(']') { hostport.to_string() } else { format!("{hostport}:80") };
        // `localhost` is often ::1 first while the server listens on 127.0.0.1 only: try each address.
        let addrs: Vec<_> = addr_str.to_socket_addrs().map_err(|e| format!("{hostport}: {e}"))?.take(8).collect();
        let mut last = format!("{hostport}: no address");
        let mut connected = None;
        for addr in addrs {
            match TcpStream::connect_timeout(&addr, Duration::from_secs(3)) {
                Ok(s) => {
                    connected = Some(s);
                    break;
                }
                Err(e) => last = format!("can't reach {url}: {e}"),
            }
        }
        let mut stream = connected.ok_or(last)?;
        let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
        let full_path = if base.is_empty() { path.to_string() } else { format!("/{base}{path}") };
        let mut req = format!(
            "{method} {full_path} HTTP/1.1\r\nHost: {hostport}\r\nUser-Agent: WordCraft\r\nAccept: application/json\r\nConnection: close\r\n"
        );
        if let Some(b) = body {
            req.push_str(&format!("Content-Type: application/x-www-form-urlencoded; charset=utf-8\r\nContent-Length: {}\r\n", b.len()));
        }
        req.push_str("\r\n");
        req.push_str(body.unwrap_or(""));
        stream.write_all(req.as_bytes()).map_err(|e| format!("sending to {url}: {e}"))?;
        let mut raw = Vec::new();
        stream.take(MAX_ANSWER as u64).read_to_end(&mut raw).map_err(|e| format!("reading from {url}: {e}"))?;
        let split = raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or("malformed answer")?;
        let head = String::from_utf8_lossy(raw.get(..split).unwrap_or_default()).into_owned();
        let mut payload = raw.get(split + 4..).unwrap_or_default().to_vec();
        let status: u16 = head.split_whitespace().nth(1).and_then(|s| s.parse().ok()).ok_or("malformed answer")?;
        let chunked = head.lines().any(|l| l.to_ascii_lowercase().starts_with("transfer-encoding:") && l.to_ascii_lowercase().contains("chunked"));
        if chunked {
            payload = dechunk(&payload).ok_or("malformed chunked answer")?;
        }
        let text = String::from_utf8_lossy(&payload).into_owned();
        if status != 200 {
            return Err(format!("the server answered {status}: {}", text.chars().take(200).collect::<String>()));
        }
        Ok(text)
    }

    fn dechunk(mut data: &[u8]) -> Option<Vec<u8>> {
        let mut out = Vec::new();
        loop {
            let line_end = data.windows(2).position(|w| w == b"\r\n")?;
            let size_line = String::from_utf8_lossy(data.get(..line_end)?).into_owned();
            let size = usize::from_str_radix(size_line.split(';').next()?.trim(), 16).ok()?;
            data = data.get(line_end + 2..)?;
            if size == 0 {
                return Some(out);
            }
            out.extend_from_slice(data.get(..size)?);
            if out.len() > MAX_ANSWER {
                return None;
            }
            data = data.get(size + 2..)?;
        }
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn chunked_answers() {
            assert_eq!(super::dechunk(b"3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n").as_deref(), Some(&b"abcde"[..]));
            assert_eq!(super::dechunk(b"zz\r\n"), None);
            assert_eq!(super::dechunk(b"10\r\nabc"), None);
            assert_eq!(super::form("a b&ü"), "a+b%26%C3%BC");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses() {
        assert_eq!(normalize_url("http://localhost:8081/").as_deref(), Ok("http://localhost:8081"));
        assert!(normalize_url("https://api.example.com").is_err());
        assert!(normalize_url("http://user:pw@host").is_err());
        assert!(normalize_url("http://").is_err());
    }

    #[test]
    fn answers_map_utf16_offsets_and_drop_spelling() {
        // "Grüße 😀 an alle die kommen": the emoji is 2 UTF-16 units, 4 bytes.
        let text = "Grüße 😀 an alle die kommen.";
        let body = r#"{"matches":[
            {"message":"Komma fehlt","offset":16,"length":4,"replacements":[{"value":", die"}],"rule":{"issueType":"grammar","category":{"id":"PUNCTUATION"}}},
            {"message":"Tippfehler","offset":0,"length":5,"replacements":[],"rule":{"issueType":"misspelling","category":{"id":"TYPOS"}}},
            {"message":"kaputt","offset":999,"length":3,"rule":{}}
        ]}"#;
        let v = parse_matches(text, body).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(text.get(v[0].start..v[0].end), Some(" die"));
        assert_eq!(v[0].suggestions, vec![", die"]);
        assert!(parse_matches(text, "nonsense").is_err());
    }

    /// A fake server on a local port answers like LanguageTool.
    #[test]
    fn talks_to_a_server() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming().take(2) {
                let mut s = stream.unwrap();
                let mut buf = [0u8; 8192];
                let n = s.read(&mut buf).unwrap();
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let body = if req.starts_with("GET /v2/languages") {
                    r#"[{"name":"German","code":"de","longCode":"de-DE"}]"#.to_string()
                } else {
                    assert!(req.contains("language=de-DE"), "{req}");
                    r#"{"matches":[{"message":"Wiederholung","offset":4,"length":7,"replacements":[{"value":"ist"}],"rule":{"issueType":"duplication","category":{"id":"MISC"}}}]}"#.to_string()
                };
                let answer = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{body}\r\n0\r\n\r\n",
                    body.len()
                );
                s.write_all(answer.as_bytes()).unwrap();
            }
        });
        let url = format!("http://127.0.0.1:{port}");
        assert_eq!(ping(&url), Ok(1));
        let r = http::check(&url, "Das ist ist gut.", "de-DE").unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].start, 4);
        assert!(ping("http://127.0.0.1:9").is_err(), "nothing listens on the discard port");
    }
}
