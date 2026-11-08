//! ytup: upload a video to the owner's YouTube channel through the YouTube Data API v3.
//!
//! YouTube's Terms of Service forbid driving the YouTube site with automation; the Data API is
//! the supported way for a program to upload. It needs an OAuth client and the owner's consent
//! once: both happen in the owner's own Chrome on this box (his default profile, where he is
//! signed in), read and pressed through Windows UI Automation, the accessibility API screen
//! readers use. Nothing here reads a cookie or a stored password. The refresh token the consent
//! returns stays in the state directory (the owner's Windows profile).
//!
//!   ytup auth [--auto] [--timeout-s N]      the OAuth consent in the owner's Chrome (once; again
//!                                           when Google expires it: Testing apps, 7 days)
//!   ytup whoami                             the authorized channel (title, id, handle)
//!   ytup list [--max N]                     the channel's latest uploads with their privacy
//!   ytup upload --file F --title T --description-file D [--tags a,b] [--category 20]
//!               [--privacy private|unlisted|public] [--thumbnail P] [--made-for-kids]
//!               [--allow-duplicate]         resumable; private unless told otherwise; a rerun
//!                                           resumes, and never uploads the same file twice
//!
//! One-time setup in the owner's Google Cloud console (driven the same way, see NOTES):
//!   ytup chrome-open --url U                open U in the owner's Chrome (new window)
//!   ytup windows                            the Chrome windows UI Automation sees
//!   ytup dump [--title S] [--grep S] [--raw]   the elements of a Chrome window's page
//!   ytup shot --out F.png [--title S]       a picture of a Chrome window
//!   ytup press --name S|@account [--role R] [--title S] [--exact]   press a button/link/checkbox
//!   ytup type --name S --text T|@account [--title S]  put text into an edit field
//!   ytup save-client --title S --project P  keep the new OAuth client's id and secret (unprinted)
//!   ytup remember-account --title S         keep the signed-in account's e-mail (for --auto)
//!   ytup close --title S                    close a Chrome window

mod api;
#[cfg(windows)]
mod auth;
#[cfg(windows)]
mod chrome;
#[cfg(windows)]
mod consent;
mod png;
#[cfg(windows)]
mod uia;
mod upload;

use anyhow::{Result, bail};

pub fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1).cloned())
}

pub fn has(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

/// Mask e-mail addresses and OAuth client secrets in a line of page text (dumps go to logs).
pub fn mask_emails(s: &str) -> String {
    s.split(' ')
        .map(|w| match w.find('@') {
            Some(at) if at > 0 && w[at..].contains('.') => format!("{}…@…", &w[..1]),
            _ if w.contains("GOCSPX-") => "GOCSPX-…".to_string(),
            _ => w.to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The signed-in account's e-mail, from the account button Google pages carry
/// ("Account: Christopher Chedeau (name@example.com)" / "Google Account: ...").
#[cfg(windows)]
fn account_email(items: &[uia::Item]) -> Result<String> {
    items
        .iter()
        .filter(|i| i.role == "button" && i.name.contains("Account: "))
        .find_map(|i| {
            let open = i.name.rfind('(')?;
            let close = i.name[open..].find(')')? + open;
            let email = i.name[open + 1..close].trim().to_string();
            email.contains('@').then_some(email)
        })
        .ok_or_else(|| anyhow::anyhow!("no account button (\"Account: Name (e-mail)\") on the page"))
}

/// Where the client, the token and the upload states live: `YTUP_STATE`, else
/// `%USERPROFILE%\yt-upload\state` (the owner's own Windows profile).
pub fn state_dir() -> Result<std::path::PathBuf> {
    let dir = match std::env::var_os("YTUP_STATE") {
        Some(d) => std::path::PathBuf::from(d),
        None => std::path::PathBuf::from(std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).ok_or_else(|| anyhow::anyhow!("no USERPROFILE"))?).join("yt-upload").join("state"),
    };
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Err(e) = run(&args) {
        eprintln!("ytup: {e:#}");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn run(_args: &[String]) -> Result<()> {
    bail!("ytup runs on Windows (the owner's browser and account live there): build it with cargo xwin for x86_64-pc-windows-msvc")
}

#[cfg(windows)]
fn run(args: &[String]) -> Result<()> {
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");
    let title = flag(args, "--title").unwrap_or_default();
    match cmd {
        "chrome-open" => chrome::open(&flag(args, "--url").ok_or_else(|| anyhow::anyhow!("--url"))?),
        "windows" => {
            let u = uia::Uia::new()?;
            for (_, t, h) in u.chrome_windows()? {
                println!("{:?} hwnd={:?}", mask_emails(&t), h.0);
            }
            Ok(())
        }
        "dump" => {
            let u = uia::Uia::new()?;
            let (w, t, _) = u.window(&title)?;
            println!("window {:?}", mask_emails(&t));
            let grep = flag(args, "--grep").map(|g| g.to_lowercase());
            for it in u.all(&w)? {
                let line = format!("{} {:?} aid={:?} value={:?} rect={:?}{}", it.role, it.name, it.aid, it.value, it.rect, if it.enabled { "" } else { " disabled" });
                let line = if has(args, "--raw") { line } else { mask_emails(&line) };
                if grep.as_ref().is_none_or(|g| line.to_lowercase().contains(g)) {
                    println!("{line}");
                }
            }
            Ok(())
        }
        "save-client" => {
            // The "OAuth client created" dialog shows the client ID and secret once: keep them in
            // the state directory (never printed).
            let u = uia::Uia::new()?;
            let (w, _, _) = u.window(&title)?;
            let items = u.all(&w)?;
            let texts: Vec<String> = items.iter().flat_map(|i| [i.name.clone(), i.value.clone()]).collect();
            let find = |pred: &dyn Fn(&str) -> bool| texts.iter().flat_map(|t| t.split_whitespace()).find(|w| pred(w)).map(str::to_string);
            let id = find(&|w| w.ends_with(".apps.googleusercontent.com")).ok_or_else(|| anyhow::anyhow!("no client ID on the page"))?;
            let secret = find(&|w| w.starts_with("GOCSPX-")).ok_or_else(|| anyhow::anyhow!("no client secret on the page"))?;
            let project = flag(args, "--project").unwrap_or_default();
            let out = state_dir()?.join("client.json");
            std::fs::write(&out, serde_json::to_string_pretty(&serde_json::json!({"client_id": id, "client_secret": secret, "project": project}))?)?;
            println!("saved the client ({}…) to {}", &id[..12.min(id.len())], out.display());
            Ok(())
        }
        "close" => {
            let u = uia::Uia::new()?;
            let (w, t, _) = u.window(&title)?;
            uia::close(&w)?;
            println!("closed {:?}", mask_emails(&t));
            Ok(())
        }
        "shot" => {
            let u = uia::Uia::new()?;
            let (_, _, h) = u.window(&title)?;
            let out = flag(args, "--out").unwrap_or_else(|| "shot.png".into());
            uia::shot(h, std::path::Path::new(&out))
        }
        "press" | "type" => {
            let u = uia::Uia::new()?;
            let (w, _, _) = u.window(&title)?;
            let items = u.all(&w)?;
            // `@account`: the e-mail of the signed-in Google account, as the page's account
            // button shows it ("Account: Name (e-mail)"), never printed.
            let resolve = |v: String| -> Result<String> { if v == "@account" { account_email(&items) } else { Ok(v) } };
            let name = resolve(flag(args, "--name").ok_or_else(|| anyhow::anyhow!("--name"))?)?;
            let role = flag(args, "--role");
            let exact = has(args, "--exact") || flag(args, "--name").as_deref() == Some("@account");
            // `--min-y`: skip the browser's own tab strip and toolbar (a page's "Close" vs the tab's).
            let min_y: i32 = flag(args, "--min-y").and_then(|v| v.parse().ok()).unwrap_or(88);
            let hit = items.iter().find(|i| {
                let name_ok = if exact { i.name == name } else { i.name.to_lowercase().contains(&name.to_lowercase()) };
                name_ok && role.as_ref().is_none_or(|r| i.role == r) && i.rect[1] >= min_y
            });
            let Some(it) = hit else { bail!("no element named {:?}{}", mask_emails(&name), role.map(|r| format!(" with role {r}")).unwrap_or_default()) };
            if cmd == "press" {
                println!("{} {:?}: {}", it.role, mask_emails(&it.name), uia::press(&it.el)?);
            } else {
                uia::set_value(&it.el, &resolve(flag(args, "--text").unwrap_or_default())?)?;
                println!("typed into {} {:?}", it.role, mask_emails(&it.name));
            }
            Ok(())
        }
        "remember-account" => {
            // The consent's account chooser is pressed by this e-mail (kept with the client).
            let u = uia::Uia::new()?;
            let (w, _, _) = u.window(&title)?;
            let email = account_email(&u.all(&w)?)?;
            let p = state_dir()?.join("client.json");
            let mut v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&p)?)?;
            v["account"] = serde_json::json!(email);
            std::fs::write(&p, serde_json::to_string_pretty(&v)?)?;
            println!("remembered the account ({}) in {}", mask_emails(&email), p.display());
            Ok(())
        }
        "auth" => auth::run(has(args, "--auto"), std::time::Duration::from_secs(flag(args, "--timeout-s").and_then(|v| v.parse().ok()).unwrap_or(600))),
        "whoami" => {
            let ch = api::channel(&api::access_token()?)?;
            println!("channel {:?} id={} handle={} uploads={}", ch.title, ch.id, ch.custom_url, ch.uploads);
            Ok(())
        }
        "list" => {
            let tok = api::access_token()?;
            let ch = api::channel(&tok)?;
            for (id, t, p) in api::uploads(&tok, &ch, flag(args, "--max").and_then(|v| v.parse().ok()).unwrap_or(10))? {
                println!("{id} {p:9} {t}");
            }
            Ok(())
        }
        "upload" => cmd_upload(args),
        _ => {
            println!("{}", include_str!("main.rs").lines().take_while(|l| l.starts_with("//!")).map(|l| l.trim_start_matches("//!").trim_start_matches(' ')).collect::<Vec<_>>().join("\n"));
            Ok(())
        }
    }
}

fn cmd_upload(args: &[String]) -> Result<()> {
    let need = |k: &str| flag(args, k).ok_or_else(|| anyhow::anyhow!("{k} is required"));
    let file = std::path::PathBuf::from(need("--file")?);
    let description = match flag(args, "--description-file") {
        Some(p) => std::fs::read_to_string(&p).map_err(|e| anyhow::anyhow!("{p}: {e}"))?.trim_end().to_string(),
        None => String::new(),
    };
    let meta = upload::Meta {
        title: need("--title")?,
        description,
        tags: flag(args, "--tags").map(|t| t.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default(),
        category: flag(args, "--category").unwrap_or_else(|| "20".into()),
        privacy: flag(args, "--privacy").unwrap_or_else(|| "private".into()),
        made_for_kids: has(args, "--made-for-kids"),
    };
    let thumb = flag(args, "--thumbnail").map(std::path::PathBuf::from);
    upload::run(&file, &meta, thumb.as_deref(), has(args, "--allow-duplicate")).map(|_| ())
}
