//! The OAuth consent, once, in the owner's own Chrome: Google's installed-app flow with PKCE and
//! a loopback redirect to this process. `--auto` presses through Google's consent pages with UI
//! Automation ([`crate::consent`]); without it the owner clicks them.

use crate::api;
use anyhow::{Result, anyhow, bail};
use base64::Engine;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// `n` characters of the PKCE alphabet from the OS-seeded hasher keys (RandomState).
pub fn random_token(n: usize) -> String {
    use std::hash::{BuildHasher, Hasher};
    const CH: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~";
    let mut out = String::new();
    while out.len() < n {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        h.write_usize(out.len());
        let mut x = h.finish();
        for _ in 0..8 {
            out.push(CH[(x % CH.len() as u64) as usize] as char);
            x /= CH.len() as u64;
        }
    }
    out.truncate(n);
    out
}

fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == key).then(|| percent_decode(v))
    })
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => {
                let hex = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
                match hex {
                    Some(v) => {
                        out.push(v);
                        i += 3;
                    }
                    None => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Wait for Google's redirect to the loopback with `code` (and our `state`).
fn accept(listener: &TcpListener, state: &str, deadline: Instant) -> Result<String> {
    loop {
        if Instant::now() > deadline {
            bail!("no consent came back in time");
        }
        let (mut s, _) = match listener.accept() {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        s.set_nonblocking(false)?;
        s.set_read_timeout(Some(Duration::from_secs(5)))?;
        let mut req = Vec::new();
        let mut buf = [0u8; 4096];
        while !req.windows(4).any(|w| w == b"\r\n\r\n") && req.len() < 65536 {
            match s.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => req.extend_from_slice(&buf[..n]),
            }
        }
        let line = String::from_utf8_lossy(&req).lines().next().unwrap_or("").to_string();
        let target = line.split_whitespace().nth(1).unwrap_or("");
        let query = target.split_once('?').map(|(_, q)| q).unwrap_or("");
        let reply = |s: &mut std::net::TcpStream, msg: &str| {
            let body = format!("<!doctype html><title>ytup</title><p style=\"font:16px sans-serif\">{msg}</p>");
            let _ = s.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
        };
        if let Some(err) = query_param(query, "error") {
            reply(&mut s, "ytup: the authorization was not granted.");
            bail!("Google answered error={err}");
        }
        match (query_param(query, "code"), query_param(query, "state")) {
            (Some(code), Some(st)) if st == state => {
                reply(&mut s, "ytup: authorized. You can close this window.");
                return Ok(code);
            }
            _ => reply(&mut s, "ytup: waiting for Google's answer."),
        }
    }
}

pub fn run(auto: bool, timeout: Duration) -> Result<()> {
    let client = api::load_client()?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    listener.set_nonblocking(true)?;
    let redirect = format!("http://127.0.0.1:{}", listener.local_addr()?.port());
    let verifier = random_token(64);
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let state = random_token(24);
    let mut url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent&code_challenge={challenge}&code_challenge_method=S256&state={state}",
        api::enc(&client.id),
        api::enc(&redirect),
        api::enc(api::SCOPES)
    );
    if !client.account.is_empty() {
        url += &format!("&login_hint={}", api::enc(&client.account));
    }
    crate::chrome::open(&url)?;
    println!("consent page opened in the owner's Chrome (redirect {redirect})");
    let done = Arc::new(AtomicBool::new(false));
    let clicker = auto.then(|| {
        let (done, account) = (done.clone(), client.account.clone());
        std::thread::spawn(move || crate::consent::press_through(&account, &done, timeout))
    });
    let code = accept(&listener, &state, Instant::now() + timeout);
    done.store(true, Ordering::Relaxed);
    if let Some(h) = clicker {
        match h.join().map_err(|_| anyhow!("the consent clicker panicked"))? {
            Ok(steps) => println!("consent pages pressed: {}", steps.join(", ")),
            Err(e) => eprintln!("consent clicker: {e:#}"),
        }
    }
    api::exchange_code(&client, &code?, &redirect, &verifier)?;
    println!("authorized: the refresh token is saved in {}", crate::state_dir()?.join("token.json").display());
    Ok(())
}
