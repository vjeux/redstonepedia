//! The YouTube Data API v3 and Google's OAuth endpoints, over curl (Windows ships curl.exe).
//! Request headers, the bearer token among them, reach curl through a config file in the state
//! directory (deleted after the call), never through its command line; bodies go on stdin.

use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

/// Upload videos and thumbnails; read the channel and its uploads (the duplicate check).
pub const SCOPES: &str = "https://www.googleapis.com/auth/youtube.upload https://www.googleapis.com/auth/youtube.readonly";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const API: &str = "https://www.googleapis.com/youtube/v3";
const UPLOAD_API: &str = "https://www.googleapis.com/upload/youtube/v3";

pub struct Resp {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Resp {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().rev().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
    pub fn json(&self) -> Result<Value> {
        serde_json::from_slice(&self.body).with_context(|| format!("HTTP {}: not JSON: {}", self.status, self.text()))
    }
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).chars().take(600).collect()
    }
}

fn curl() -> &'static str {
    if cfg!(windows) { r"C:\Windows\System32\curl.exe" } else { "curl" }
}

static SEQ: AtomicU64 = AtomicU64::new(0);

/// One HTTP request.
pub fn http(method: &str, url: &str, headers: &[(&str, String)], body: Option<&[u8]>) -> Result<Resp> {
    let dir = crate::state_dir()?;
    let tag = format!("{}-{}", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed));
    let (cfg, hdr, out) = (dir.join(format!("req-{tag}.cfg")), dir.join(format!("req-{tag}.hdr")), dir.join(format!("req-{tag}.out")));
    let mut c = String::from("header = \"Expect:\"\n");
    for (k, v) in headers {
        c += &format!("header = \"{k}: {}\"\n", v.replace('\\', "\\\\").replace('"', "\\\""));
    }
    std::fs::write(&cfg, c)?;
    let mut cmd = Command::new(curl());
    cmd.args(["-sS", "--http1.1", "--max-time", "600", "-X", method, "-K"]).arg(&cfg).arg("-D").arg(&hdr).arg("-o").arg(&out).args(["-w", "%{http_code}"]);
    if body.is_some() {
        cmd.args(["--data-binary", "@-"]);
    }
    cmd.arg(url).stdin(if body.is_some() { Stdio::piped() } else { Stdio::null() }).stdout(Stdio::piped()).stderr(Stdio::piped());
    let result = (|| -> Result<Resp> {
        let mut child = cmd.spawn().with_context(|| format!("starting {}", curl()))?;
        if let Some(b) = body {
            let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
            stdin.write_all(b)?;
        }
        let o = child.wait_with_output()?;
        let code = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if !o.status.success() && code.is_empty() {
            bail!("curl {method} failed: {}", String::from_utf8_lossy(&o.stderr).trim());
        }
        let status: u16 = code.parse().map_err(|_| anyhow!("curl {method}: no HTTP status ({}): {}", code, String::from_utf8_lossy(&o.stderr).trim()))?;
        let raw = std::fs::read_to_string(&hdr).unwrap_or_default();
        // The last header block is the final response's.
        let block = raw.split("\r\n\r\n").filter(|b| b.starts_with("HTTP/")).last().unwrap_or("");
        let headers = block.lines().skip(1).filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))).collect();
        Ok(Resp { status, headers, body: std::fs::read(&out).unwrap_or_default() })
    })();
    for f in [&cfg, &hdr, &out] {
        let _ = std::fs::remove_file(f);
    }
    result
}

pub fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn form(pairs: &[(&str, &str)]) -> String {
    pairs.iter().map(|(k, v)| format!("{k}={}", enc(v))).collect::<Vec<_>>().join("&")
}

pub struct Client {
    pub id: String,
    pub secret: String,
    pub account: String,
}

pub fn load_client() -> Result<Client> {
    let p = crate::state_dir()?.join("client.json");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).with_context(|| format!("no OAuth client at {} (ytup setup creates it)", p.display()))?)?;
    let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
    if s("client_id").is_empty() || s("client_secret").is_empty() {
        bail!("{} has no client_id/client_secret", p.display());
    }
    Ok(Client { id: s("client_id"), secret: s("client_secret"), account: s("account") })
}

fn token_path() -> Result<PathBuf> {
    Ok(crate::state_dir()?.join("token.json"))
}

/// Trade the consent's authorization code for tokens; keep the refresh token.
pub fn exchange_code(client: &Client, code: &str, redirect: &str, verifier: &str) -> Result<()> {
    let body = form(&[("code", code), ("client_id", &client.id), ("client_secret", &client.secret), ("redirect_uri", redirect), ("grant_type", "authorization_code"), ("code_verifier", verifier)]);
    let r = http("POST", TOKEN_URL, &[("Content-Type", "application/x-www-form-urlencoded".into())], Some(body.as_bytes()))?;
    let v = r.json()?;
    let refresh = v["refresh_token"].as_str().ok_or_else(|| anyhow!("token exchange: HTTP {} without a refresh token: {}", r.status, v.get("error").cloned().unwrap_or_default()))?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
    let saved = json!({"refresh_token": refresh, "scope": v["scope"], "obtained_unix": now});
    std::fs::write(token_path()?, serde_json::to_string_pretty(&saved)?)?;
    Ok(())
}

/// A fresh access token from the saved refresh token (lives in memory only).
pub fn access_token() -> Result<String> {
    let client = load_client()?;
    let p = token_path()?;
    let t: Value = serde_json::from_str(&std::fs::read_to_string(&p).with_context(|| format!("not authorized yet: no {} (run `ytup auth --auto`)", p.display()))?)?;
    let refresh = t["refresh_token"].as_str().ok_or_else(|| anyhow!("{} has no refresh_token", p.display()))?;
    let body = form(&[("client_id", &client.id), ("client_secret", &client.secret), ("refresh_token", refresh), ("grant_type", "refresh_token")]);
    let r = http("POST", TOKEN_URL, &[("Content-Type", "application/x-www-form-urlencoded".into())], Some(body.as_bytes()))?;
    let v = r.json()?;
    if let Some(tok) = v["access_token"].as_str() {
        return Ok(tok.to_string());
    }
    if v["error"] == "invalid_grant" {
        bail!("the saved authorization was revoked or expired (an app in Testing gets 7-day refresh tokens): run `ytup auth --auto` again");
    }
    bail!("token refresh: HTTP {}: {}", r.status, v.get("error").cloned().unwrap_or_default())
}

fn bearer(tok: &str) -> (&'static str, String) {
    ("Authorization", format!("Bearer {tok}"))
}

pub fn get(tok: &str, path_and_query: &str) -> Result<Value> {
    let r = http("GET", &format!("{API}/{path_and_query}"), &[bearer(tok)], None)?;
    if r.status != 200 {
        bail!("GET {path_and_query}: HTTP {}: {}", r.status, r.text());
    }
    r.json()
}

pub struct Channel {
    pub id: String,
    pub title: String,
    pub custom_url: String,
    pub uploads: String,
}

pub fn channel(tok: &str) -> Result<Channel> {
    let v = get(tok, "channels?part=snippet,contentDetails&mine=true")?;
    let c = v["items"].get(0).ok_or_else(|| anyhow!("the authorized account has no YouTube channel"))?;
    Ok(Channel {
        id: c["id"].as_str().unwrap_or("").into(),
        title: c["snippet"]["title"].as_str().unwrap_or("").into(),
        custom_url: c["snippet"]["customUrl"].as_str().unwrap_or("").into(),
        uploads: c["contentDetails"]["relatedPlaylists"]["uploads"].as_str().unwrap_or("").into(),
    })
}

/// The channel's uploads, newest first: (video id, title, privacy status), up to `max`.
pub fn uploads(tok: &str, ch: &Channel, max: usize) -> Result<Vec<(String, String, String)>> {
    let mut out = Vec::new();
    let mut page = String::new();
    while out.len() < max {
        let v = get(tok, &format!("playlistItems?part=snippet,status&maxResults=50&playlistId={}{}", enc(&ch.uploads), if page.is_empty() { String::new() } else { format!("&pageToken={}", enc(&page)) }))?;
        for it in v["items"].as_array().cloned().unwrap_or_default() {
            let id = it["snippet"]["resourceId"]["videoId"].as_str().unwrap_or("").to_string();
            let title = it["snippet"]["title"].as_str().unwrap_or("").to_string();
            let privacy = it["status"]["privacyStatus"].as_str().unwrap_or("").to_string();
            out.push((id, title, privacy));
        }
        match v["nextPageToken"].as_str() {
            Some(p) => page = p.to_string(),
            None => break,
        }
    }
    out.truncate(max);
    Ok(out)
}

pub fn video(tok: &str, id: &str) -> Result<Option<Value>> {
    let v = get(tok, &format!("videos?part=snippet,status,processingDetails&id={}", enc(id)))?;
    Ok(v["items"].get(0).cloned())
}

pub fn sha256_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut f = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

pub fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("mp4") | Some("m4v") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mov") => "video/quicktime",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        _ => "application/octet-stream",
    }
}

pub fn upload_url() -> String {
    format!("{UPLOAD_API}/videos?uploadType=resumable&part=snippet,status&notifySubscribers=false")
}

pub fn thumbnail_url(video_id: &str) -> String {
    format!("{UPLOAD_API}/thumbnails/set?videoId={}&uploadType=media", enc(video_id))
}

pub fn bearer_header(tok: &str) -> (&'static str, String) {
    bearer(tok)
}
