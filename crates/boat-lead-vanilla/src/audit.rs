use anyhow::{Context,Result,bail};
use std::{collections::BTreeSet,path::Path,process::Command};

/// Audits only the explicit changed-file set, never scans the checkout recursively.
pub fn audit(root:&Path)->Result<()> {
    let tracked=Command::new("git").args(["diff","--name-only","HEAD"]).current_dir(root).output()?;
    let untracked=Command::new("git").args(["ls-files","--others","--exclude-standard"]).current_dir(root).output()?;
    let paths:BTreeSet<String>=String::from_utf8(tracked.stdout)?.lines().chain(String::from_utf8(untracked.stdout)?.lines()).map(str::to_owned).collect();
    let mut checked=0;
    let mut max_authored_span=0;
    for name in &paths {
        let bytes=std::fs::read(root.join(name))?;
        let Ok(text)=std::str::from_utf8(&bytes) else {continue;};
        if text.lines().any(|l|l.starts_with("<<<<<<< ") || l.starts_with(">>>>>>> ")){bail!("merge marker: {name}");}
        if name.starts_with("crates/boat-lead-vanilla/src/") && name.ends_with(".rs") {
            let lines:Vec<_>=text.lines().collect();
            let mut starts:Vec<_>=lines.iter().enumerate().filter(|(_,l)| {
                let l=l.trim_start(); l.starts_with("fn ") || l.starts_with("pub fn ")
            }).map(|(i,_)|i).collect();
            starts.push(lines.len());
            for pair in starts.windows(2) {
                let span=pair[1]-pair[0];
                max_authored_span=max_authored_span.max(span);
                if span>150 {bail!("authored function span {span}>150: {name}:{}",pair[0]+1);}
            }
        }
        checked+=1;
    }
    // The crate's authored Rust functions are short. Bound the changed core launch function
    // explicitly; old untouched file functions and borrowed Java are not relabeled as ours.
    let core=std::fs::read_to_string(root.join("crates/mctas/src/vanilla/mod.rs"))?;
    let start=core.lines().position(|l|l.contains("fn start_profile(")).context("start_profile")?;
    let end=core.lines().skip(start+1).position(|l|l.starts_with("    pub fn game_port(")).context("next method")?;
    if end>150 {bail!("start_profile span {end}>150");}
    println!("PASS markers: {checked} changed text files; PASS start_profile span: {end} lines; PASS authored Rust max conservative span: {max_authored_span} lines (all <=150)");
    Ok(())
}
