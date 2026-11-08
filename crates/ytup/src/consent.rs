//! Press through Google's consent pages in the owner's Chrome with UI Automation: choose the
//! account, the notice an app in Testing shows ("Google hasn't verified this app"), the scope
//! checkboxes, Continue. It only acts on pages of accounts.google.com, never types anything, and
//! stops when Google asks for a password or a second factor: that is the owner's to do.

use crate::uia::{self, Item, Uia};
use anyhow::{Result, bail};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub fn press_through(account: &str, done: &AtomicBool, timeout: Duration) -> Result<Vec<String>> {
    let u = Uia::new()?;
    let start = Instant::now();
    let mut log: Vec<String> = Vec::new();
    let mut last: Option<(String, Instant)> = None;
    while !done.load(Ordering::Relaxed) && start.elapsed() < timeout {
        std::thread::sleep(Duration::from_millis(800));
        let Some(items) = consent_page(&u)? else { continue };
        if let Some(why) = owner_needed(&items) {
            bail!("{why}");
        }
        let Some((label, idx)) = next_step(&items, account) else { continue };
        // The page has not changed since the last press yet: give it time.
        if let Some((l, at)) = &last {
            if *l == label && at.elapsed() < Duration::from_secs(5) {
                continue;
            }
        }
        let how = uia::press(&items[idx].el)?;
        log.push(format!("{label} ({how})"));
        last = Some((label, Instant::now()));
    }
    Ok(log)
}

/// The elements of the Chrome window that shows a page of accounts.google.com, if any.
fn consent_page(u: &Uia) -> Result<Option<Vec<Item>>> {
    for (w, _, _) in u.chrome_windows()? {
        let items = u.all(&w)?;
        if items.iter().any(|i| i.role == "document" && i.value.starts_with("https://accounts.google.com/")) {
            return Ok(Some(items));
        }
    }
    Ok(None)
}

fn owner_needed(items: &[Item]) -> Option<String> {
    let has = |s: &str| items.iter().any(|i| i.name.contains(s));
    if items.iter().any(|i| i.role == "edit" && i.name.to_lowercase().contains("password")) {
        return Some("Google asks for the account password: an owner action (ytup never types a password)".into());
    }
    if has("2-Step Verification") || has("Verify it’s you") || has("Verify it's you") {
        return Some("Google asks the owner to confirm it is them (2-Step Verification): an owner action".into());
    }
    if has("This app is blocked") || has("Access blocked") {
        return Some("Google blocked the app for this account".into());
    }
    None
}

fn next_step(items: &[Item], account: &str) -> Option<(String, usize)> {
    let has = |s: &str| items.iter().any(|i| i.name.contains(s));
    let button = |pred: &dyn Fn(&str) -> bool| items.iter().position(|i| i.role == "button" && i.enabled && i.rect[1] >= 88 && pred(&i.name));
    if has("Choose an account") || has("Choose your account") {
        // The personal account (its entry carries the e-mail), never a brand account listed
        // under it (a brand account's entry has no e-mail: "React Brand Account").
        if account.is_empty() {
            return None;
        }
        if let Some(i) = items.iter().position(|i| matches!(i.role, "link" | "button") && i.name.contains(account)) {
            return Some(("the account".into(), i));
        }
    }
    if has("hasn’t verified this app") || has("hasn't verified this app") {
        if let Some(i) = button(&|n| n == "Continue") {
            return Some(("the Testing-app notice".into(), i));
        }
    }
    for (idx, it) in items.iter().enumerate() {
        if it.role == "checkbox" && it.enabled && uia::toggled(&it.el) == Some(false) {
            return Some((format!("scope checkbox {:?}", it.name.chars().take(60).collect::<String>()), idx));
        }
    }
    button(&|n| n == "Continue" || n == "Allow").map(|i| ("Continue".into(), i))
}
