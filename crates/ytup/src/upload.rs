//! A resumable upload whose progress survives a crash or a lost connection. The session URL and
//! the file's hash live in `uploads/<hash>.json`: a rerun asks YouTube how many bytes it already
//! has and continues from there, and a finished upload is never sent twice (the same file by its
//! hash, or a video with the same title already on the channel).

use crate::api::{self, Resp};
use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::Duration;

pub struct Meta {
    pub title: String,
    pub description: String,
    pub tags: Vec<String>,
    pub category: String,
    pub privacy: String,
    pub made_for_kids: bool,
}

/// A multiple of 256 KiB, as resumable uploads require.
const CHUNK: u64 = 8 * 1024 * 1024;

enum Progress {
    Done(Value),
    At(u64),
    Gone,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn save(path: &Path, st: &Value) -> Result<()> {
    std::fs::write(path, serde_json::to_string_pretty(st)?)?;
    Ok(())
}

pub fn validate(m: &Meta) -> Result<()> {
    if m.title.trim().is_empty() || m.title.chars().count() > 100 {
        bail!("the title must have 1 to 100 characters ({})", m.title.chars().count());
    }
    if m.description.len() > 5000 {
        bail!("the description is {} bytes (YouTube's limit: 5000)", m.description.len());
    }
    if m.title.contains(['<', '>']) || m.description.contains(['<', '>']) {
        bail!("YouTube refuses '<' and '>' in titles and descriptions");
    }
    if m.tags.iter().map(|t| t.len() + if t.contains(' ') { 2 } else { 0 }).sum::<usize>() + m.tags.len() > 500 {
        bail!("the tags are over YouTube's 500 characters");
    }
    if !["private", "unlisted", "public"].contains(&m.privacy.as_str()) {
        bail!("--privacy is private, unlisted or public");
    }
    Ok(())
}

fn metadata(m: &Meta) -> Value {
    json!({
        "snippet": {"title": m.title, "description": m.description, "tags": m.tags, "categoryId": m.category,
                    "defaultLanguage": "en", "defaultAudioLanguage": "en"},
        "status": {"privacyStatus": m.privacy, "selfDeclaredMadeForKids": m.made_for_kids, "embeddable": true, "license": "youtube"},
    })
}

/// Upload `file` (or find it already there), set the thumbnail, read the video back.
/// Returns the video id.
pub fn run(file: &Path, m: &Meta, thumbnail: Option<&Path>, allow_duplicate: bool) -> Result<String> {
    validate(m)?;
    let size = std::fs::metadata(file).with_context(|| format!("{}", file.display()))?.len();
    let hash = api::sha256_file(file)?;
    let dir = crate::state_dir()?.join("uploads");
    std::fs::create_dir_all(&dir)?;
    let sp = dir.join(format!("{}.json", &hash[..16]));
    let mut st: Value = std::fs::read_to_string(&sp).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_else(|| json!({}));
    st["file"] = json!(file.display().to_string());
    st["size"] = json!(size);
    st["sha256"] = json!(hash);
    st["title"] = json!(m.title);
    st["privacy_requested"] = json!(m.privacy);
    save(&sp, &st)?;
    let mut tok = api::access_token()?;
    let ch = api::channel(&tok)?;
    println!("channel: {:?} {} {}", ch.title, ch.id, ch.custom_url);
    st["channel_id"] = json!(ch.id);
    if let Some(id) = st["video_id"].as_str().map(str::to_string) {
        if api::video(&tok, &id)?.is_some() {
            println!("this file is already on the channel: {id}");
            return finish(&tok, &id, m, thumbnail, &mut st, &sp);
        }
    }
    if !allow_duplicate {
        if let Some((id, _, privacy)) = api::uploads(&tok, &ch, 500)?.into_iter().find(|(_, t, _)| *t == m.title) {
            println!("the channel already has a video with this title: {id} ({privacy}); not uploading again (--allow-duplicate uploads anyway)");
            st["video_id"] = json!(id);
            save(&sp, &st)?;
            return finish(&tok, &id, m, thumbnail, &mut st, &sp);
        }
    }
    let id = send(&mut tok, file, size, m, &mut st, &sp)?;
    finish(&tok, &id, m, thumbnail, &mut st, &sp)
}

fn range_next(r: &Resp) -> u64 {
    r.header("Range").and_then(|v| v.rsplit('-').next()).and_then(|n| n.trim().parse::<u64>().ok()).map(|n| n + 1).unwrap_or(0)
}

fn query(tok: &str, session: &str, size: u64) -> Result<Progress> {
    let r = api::http("PUT", session, &[api::bearer_header(tok), ("Content-Range", format!("bytes */{size}"))], Some(&[]))?;
    Ok(match r.status {
        200 | 201 => Progress::Done(r.json()?),
        308 => Progress::At(range_next(&r)),
        404 | 410 => Progress::Gone,
        s => bail!("upload status: HTTP {s}: {}", r.text()),
    })
}

fn start_session(tok: &str, file: &Path, size: u64, m: &Meta) -> Result<String> {
    let headers = [
        api::bearer_header(tok),
        ("Content-Type", "application/json; charset=UTF-8".into()),
        ("X-Upload-Content-Length", size.to_string()),
        ("X-Upload-Content-Type", api::content_type(file).into()),
    ];
    let r = api::http("POST", &api::upload_url(), &headers, Some(metadata(m).to_string().as_bytes()))?;
    if r.status != 200 {
        bail!("starting the upload: HTTP {}: {}", r.status, r.text());
    }
    Ok(r.header("Location").ok_or_else(|| anyhow!("the upload start has no Location header"))?.to_string())
}

fn send(tok: &mut String, file: &Path, size: u64, m: &Meta, st: &mut Value, sp: &Path) -> Result<String> {
    let mut offset = 0u64;
    let mut session = st["session_url"].as_str().map(str::to_string);
    if let Some(s) = &session {
        match query(tok, s, size)? {
            Progress::Done(v) => return video_id(&v, st, sp),
            Progress::At(n) => {
                offset = n;
                println!("resuming the upload at {n} of {size} bytes");
            }
            Progress::Gone => session = None,
        }
    }
    let session = match session {
        Some(s) => s,
        None => {
            let s = start_session(tok, file, size, m)?;
            st["session_url"] = json!(s);
            st["started_unix"] = json!(now());
            save(sp, st)?;
            s
        }
    };
    let mut f = std::fs::File::open(file)?;
    let mut failures = 0u32;
    loop {
        let end = (offset + CHUNK).min(size) - 1;
        let mut buf = vec![0u8; (end - offset + 1) as usize];
        f.seek(SeekFrom::Start(offset))?;
        f.read_exact(&mut buf)?;
        let headers = [api::bearer_header(tok), ("Content-Type", api::content_type(file).into()), ("Content-Range", format!("bytes {offset}-{end}/{size}"))];
        let progress = match api::http("PUT", &session, &headers, Some(&buf)) {
            Ok(r) if r.status == 200 || r.status == 201 => Progress::Done(r.json()?),
            Ok(r) if r.status == 308 => Progress::At(range_next(&r)),
            Ok(r) if r.status == 401 => {
                *tok = api::access_token()?;
                query(tok, &session, size)?
            }
            Ok(r) if r.status >= 500 => retry(&mut failures, &format!("HTTP {}", r.status), tok, &session, size)?,
            Ok(r) => bail!("upload: HTTP {}: {}", r.status, r.text()),
            Err(e) => retry(&mut failures, &format!("{e:#}"), tok, &session, size)?,
        };
        match progress {
            Progress::Done(v) => return video_id(&v, st, sp),
            Progress::At(n) => {
                offset = n;
                st["acked_bytes"] = json!(n);
                save(sp, st)?;
                println!("uploaded {n} / {size} bytes ({:.0}%)", n as f64 * 100.0 / size as f64);
            }
            Progress::Gone => bail!("YouTube dropped the upload session; run the same command again to start over"),
        }
    }
}

fn retry(failures: &mut u32, why: &str, tok: &str, session: &str, size: u64) -> Result<Progress> {
    *failures += 1;
    if *failures > 8 {
        bail!("upload: giving up after {} failures (last: {why}); run the same command again to resume", *failures - 1);
    }
    let wait = Duration::from_secs(2u64.pow((*failures).min(6)));
    eprintln!("upload: {why}; retrying in {wait:?}");
    std::thread::sleep(wait);
    query(tok, session, size)
}

fn video_id(v: &Value, st: &mut Value, sp: &Path) -> Result<String> {
    let id = v["id"].as_str().ok_or_else(|| anyhow!("the finished upload has no video id: {v}"))?.to_string();
    st["video_id"] = json!(id);
    st["uploaded_unix"] = json!(now());
    save(sp, st)?;
    println!("uploaded: video {id}");
    Ok(id)
}

fn finish(tok: &str, id: &str, m: &Meta, thumbnail: Option<&Path>, st: &mut Value, sp: &Path) -> Result<String> {
    if let Some(t) = thumbnail {
        let th = api::sha256_file(t)?;
        if st["thumbnail_sha256"].as_str() != Some(th.as_str()) {
            let bytes = std::fs::read(t)?;
            let r = api::http("POST", &api::thumbnail_url(id), &[api::bearer_header(tok), ("Content-Type", api::content_type(t).into())], Some(&bytes))?;
            if r.status == 200 {
                st["thumbnail_sha256"] = json!(th);
                save(sp, st)?;
                println!("thumbnail set");
            } else {
                eprintln!("thumbnail NOT set: HTTP {}: {} (custom thumbnails need a phone-verified channel; the video itself is uploaded)", r.status, r.text());
            }
        }
    }
    let v = api::video(tok, id)?.ok_or_else(|| anyhow!("video {id} is not visible to the channel"))?;
    let privacy = v["status"]["privacyStatus"].as_str().unwrap_or("?");
    println!(
        "video {id}: privacy={privacy} upload={} processing={} title={:?}",
        v["status"]["uploadStatus"].as_str().unwrap_or("?"),
        v["processingDetails"]["processingStatus"].as_str().unwrap_or("?"),
        v["snippet"]["title"].as_str().unwrap_or("")
    );
    println!("https://youtu.be/{id}\nhttps://studio.youtube.com/video/{id}/edit");
    if privacy != m.privacy {
        bail!("video {id} is {privacy}, not {} as asked: check it in YouTube Studio", m.privacy);
    }
    Ok(id.to_string())
}
