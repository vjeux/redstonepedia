//! (vcompose, G1 Nov 13 23:21 D5 / Nov 14 00:06 / 03:39a B2) The binary twins of the composer's
//! rules, through the real `mctas mobfarm column` command: a clash exits 1 and a `--precedence` line
//! settles it; a part on a reserved cell exits non-zero naming the cell, the reserve, its owner and
//! its item, and the same with `--reserve-clash` exits 0 printing NOT CITABLE; an unused
//! `--reserve-clash` and an unnamed `--reserve` on a written record exit non-zero; a stand-in whose
//! support a `--clear` takes, or that names no support, exits non-zero, on a fixture and on V rev 2's
//! own command (column::traced_tests run the same rules on the library's path).
//!
//! Under ironfarm's provenance check (train 16b) these spawns need a STAMPED binary: run them through
//! `mctas test`, whose runner builds every piece's test binaries with the tested tree's stamp
//! (MCTAS_SOURCE_TREE); under a plain `cargo test` the binary is unstamped and every case fails
//! with "binary unstamped: provenance refused" (`named()`), on a clean worktree too.

use std::path::PathBuf;
use std::process::{Command, Output};

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vc-bin-twin-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A one-layer part: `head` header lines, then `row` (x along the row, z 0) from `palette`.
fn part(dir: &std::path::Path, n: &str, head: &str, palette: &str, row: &str) -> PathBuf {
    let p = dir.join(format!("{n}.txt"));
    std::fs::write(&p, format!("{head}name: {n}\ncircuit: mob-farm iron_ingot\nport output: 0 0 0\npalette:\n{palette}layer 0:\n  {row}\n")).unwrap();
    p
}

fn column(args: &[String]) -> Output {
    named(Command::new(env!("CARGO_BIN_EXE_mctas")).arg("mobfarm").arg("column").args(args).output().unwrap())
}

/// (G1 Nov 14 05:41, ironspec 05:42 (a)) ironfarm's provenance check refuses an UNSTAMPED binary with
/// exit 4 before any verdict (the evidence exit-code table, ironspec 02:57): every spawn here names
/// that refusal, so a stamping regression reads as what it is, never as a 0-vs-4 or 1-vs-4 mismatch in
/// a rule's case (`mctas test`'s runner stamps every binary it builds with the tested tree). Only the
/// gate's own fixed refusal marker counts (G1 06:22b): an exit 4 whose reason merely mentions
/// provenance comes back unchanged, to meet the case's own assertion.
const GATE_REFUSED: &str = "provenance (ironspec Nov 14 01:58 / 02:35 / 02:43): REFUSED: ";

fn named(o: Output) -> Output {
    if o.status.code() == Some(4) {
        let t = text(&o);
        if t.contains(&format!("{GATE_REFUSED}an UNSTAMPED binary")) {
            panic!("binary unstamped: provenance refused: {t}");
        }
        if t.contains(GATE_REFUSED) {
            panic!("provenance refused (a dirty worktree or a stale stamp, not the rule under test): {t}");
        }
    }
    o
}

fn text(o: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
}

#[test]
fn a_clash_exits_1_and_a_precedence_line_exits_0() {
    let dir = tmp("clash");
    let (a, b) = (part(&dir, "a", "", "  X stone\n", "X"), part(&dir, "b", "", "  X glass\n", "X"));
    let out = dir.join("out.txt");
    let run = |extra: &[&str]| {
        let mut v: Vec<String> = ["--part", &format!("A={}", a.display()), "--part", &format!("B={}", b.display()), "--port", "output=0,0,0", "--out", &out.display().to_string()].iter().map(|s| s.to_string()).collect();
        v.extend(extra.iter().map(|s| s.to_string()));
        column(&v)
    };
    let o = run(&[]);
    assert_eq!(o.status.code(), Some(1), "a clash exits 1: {}", text(&o));
    assert!(text(&o).contains("CLASH") && text(&o).contains("0,0,0"), "{}", text(&o));
    assert!(!out.exists(), "a failed compose writes nothing");
    let o = run(&["--precedence", "0,0,0=B:the test's ruling"]);
    assert_eq!(o.status.code(), Some(0), "settled by a precedence line, it exits 0: {}", text(&o));
    assert!(std::fs::read_to_string(&out).unwrap().contains("# precedence: 0,0,0=B (the test's ruling)"));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_reserved_cell_held_exits_non_zero_unless_acknowledged() {
    let dir = tmp("reserve");
    let a = part(&dir, "a", "", "  S stone\n  G glass\n", "SG");
    let out = dir.join("out.txt");
    let run = |extra: &[&str]| {
        let mut v: Vec<String> = ["--part", &format!("A={}", a.display()), "--port", "output=0,0,0", "--out", &out.display().to_string()].iter().map(|s| s.to_string()).collect();
        v.extend(extra.iter().map(|s| s.to_string()));
        column(&v)
    };
    // a part's cell inside a reserved box: non-zero, naming the cell, the reserve, its owner and item
    let o = run(&["--reserve", "RLINE=0,0,0,0,1,0:day-1:R-line supports"]);
    assert_ne!(o.status.code(), Some(0), "{}", text(&o));
    assert!(text(&o).contains("0,0,0 (reserved RLINE (for day-1, item R-line supports), held by A: minecraft:stone"), "{}", text(&o));
    assert!(!out.exists(), "a failed compose writes nothing");
    // acknowledged with the ruling that resolves it: exits 0, the record NOT CITABLE
    let o = run(&["--reserve", "RLINE=0,0,0,0,1,0:day-1:R-line supports", "--reserve-clash", "0,0,0=F3 r4:the stone moves"]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    assert!(text(&o).contains("NOT CITABLE"), "{}", text(&o));
    assert!(std::fs::read_to_string(&out).unwrap().contains("# reserve-clash: 0,0,0 (reserved RLINE (for day-1, item R-line supports), held by A: minecraft:stone; resolved by F3 r4: the stone moves)"));
    std::fs::remove_file(&out).ok();
    // an acknowledgement that matches no held reserved cell refuses
    let o = run(&["--reserve", "RLINE=0,0,0,0,1,0:day-1:R-line supports", "--reserve-clash", "0,0,0=F3 r4:x", "--reserve-clash", "5,5,5=F3 r4:y"]);
    assert_ne!(o.status.code(), Some(0), "{}", text(&o));
    assert!(text(&o).contains("--reserve-clash 5,5,5=F3 r4: no part holds a reserved cell there"), "{}", text(&o));
    // (G1 03:39a B1) a written record names each reserve's owner and item
    let o = run(&["--reserve", "EMPTY=4,0,4,4,0,4"]);
    assert_ne!(o.status.code(), Some(0), "{}", text(&o));
    assert!(text(&o).contains("--reserve EMPTY=4,0,4,4,0,4: a record written or verified names each reserved box's owner and item"), "{}", text(&o));
    let o = run(&["--reserve", "EMPTY=4,0,4,4,0,4:day-1:R-feed"]);
    assert_eq!(o.status.code(), Some(0), "named, it composes: {}", text(&o));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_stand_in_without_its_support_exits_non_zero() {
    let dir = tmp("support");
    let out = dir.join("out.txt");
    let run = |head: &str, extra: &[&str]| {
        let a = part(&dir, "a", head, "  S stone\n  G glass\n", "SG");
        let mut v: Vec<String> = ["--part", &format!("A={}", a.display()), "--port", "output=1,0,0", "--out", &out.display().to_string()].iter().map(|s| s.to_string()).collect();
        v.extend(extra.iter().map(|s| s.to_string()));
        column(&v)
    };
    let o = run("# stand-in: 0,1,0 support=0,0,0 (a lever on the stone)\n", &[]);
    assert_eq!(o.status.code(), Some(0), "its support held: {}", text(&o));
    // the support taken by a clear
    let o = run("# stand-in: 0,1,0 support=0,0,0 (a lever on the stone)\n", &["--clear=0,0,0,0,0,0"]);
    assert_ne!(o.status.code(), Some(0), "{}", text(&o));
    assert!(text(&o).contains("stand-in 0,1,0: its support 0,0,0 is empty in the record"), "{}", text(&o));
    // no support named
    let o = run("# stand-in: 0,1,0 (a lever on the stone)\n", &[]);
    assert_ne!(o.status.code(), Some(0), "{}", text(&o));
    assert!(text(&o).contains("stand-in 0,1,0: names no support right after its cell"), "{}", text(&o));
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn v_rev_2s_command_fails_when_a_lever_stand_ins_support_is_cleared() {
    // (G1 03:39a B2) V rev 2's own compose (the record commit's command, its record written to a
    // scratch file) with J(-1)'s ceiling glass (-1,27,9) cleared: the lift's stand-in names it
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = tmp("vrev2");
    let r = "designs/farms/iron/carousel-assembly/wip/vcompose";
    let p = format!("{r}/parts");
    let o = "designs/farms/iron/carousel-assembly/wip/onechunk/module";
    let b = format!("{p}/f1q2-module-wayin-bplus.ec561e48d.txt");
    let l = format!("{p}/f1q2-lift-split-path-a.77b7a74af.txt");
    let mut args: Vec<String> = vec![
        "--name=f1q2-module-v-rev2".into(),
        "--part=STATION!=designs/farms/iron/carousel-assembly/wip/station/f1q2-station.txt#-3,-9,-6,9,18,7".into(),
        format!("--clear-part={p}/f1q2-station.main-6e05a4a72.txt@0,0,0"),
        "--part=LANES=designs/farms/iron/carousel-assembly/wip/lanes/f1q2-lanes-sbend.txt".into(),
        format!("--part=LIFT={l}"),
        format!("--part=NAPFOOT!={l}@0,7,0#5,-29,3,7,-27,5"),
        format!("--part=FOLD={p}/bed-tier-fold.bfa0c8098.txt@0,-20,-1"),
        format!("--part=WAYLEG={b}#9,21,-6,9,25,9"),
        format!("--part=WAYFALL={b}#9,-27,-6,9,20,-1"),
        format!("--part=WAYGUARD={b}#8,14,-5,8,22,-5"),
        format!("--part=QCART!={b}@0,4,0#9,-14,-4,9,-14,-4"),
        format!("--part=SKY={o}/f1q2-sky.txt"),
        format!("--part=POSTS={o}/f1q2-scarers-2post.txt"),
        format!("--part=F3={p}/f1q2-scarer-f3.bc8a97b89.txt"),
        format!("--part=R3={p}/f1q2-module-record7f13b.rev3.e8ed1e0fa.txt"),
        "--stale=R3".into(),
        format!("--delta=WAYIN={p}/f1q2-wayin-v.b8949c96.0f86b83e8.delta"),
        format!("--delta=RTRIP={p}/return-tripwire-C.ironspec-2241.txt"),
        format!("--delta=DROPS={p}/v-drops.txt"),
        "--precedence=9,25,9=WAYLEG:the way-in curve north_west of b8949c96s way-in file; the lift file holds a rail east_west there as its own bench stand-in (its stand-in line for 9,25,9)".into(),
        "--clear=-6,-30,-6,9,-24,10".into(),
        "--clear=5,-23,3,7,-20,5".into(),
        "--port=output=-6,1,-6".into(),
        format!("--out={}", dir.join("v.txt").display()),
    ];
    let run = |extra: &[&str], args: &[String]| named(Command::new(env!("CARGO_BIN_EXE_mctas")).current_dir(&repo).arg("mobfarm").arg("column").args(args).args(extra).output().unwrap());
    let ok = run(&[], &args);
    assert_eq!(ok.status.code(), Some(0), "V rev 2's command composes: {}", text(&ok));
    args.push("--clear=-1,27,9,-1,27,9".into());
    let bad = run(&[], &args);
    assert_ne!(bad.status.code(), Some(0), "{}", text(&bad));
    assert!(text(&bad).contains("stand-in -1,26,9: its support -1,27,9 is empty in the record"), "{}", text(&bad));
    std::fs::remove_dir_all(&dir).ok();
}

/// An `Output` with exit code `code` and `stderr` (a control for `named()`, no binary run).
#[cfg(unix)]
fn fake(code: i32, stderr: &str) -> Output {
    use std::os::unix::process::ExitStatusExt;
    Output { status: std::process::ExitStatus::from_raw(code << 8), stdout: Vec::new(), stderr: stderr.as_bytes().to_vec() }
}

#[test]
#[cfg(unix)]
fn named_passes_every_other_exit_through_to_the_cases_own_assertion() {
    // (ironspec Nov 14 06:18 (1), G1 06:22b) the helper can't mask a real failure: an exit 4 without
    // the gate's refusal marker (its reason merely mentioning provenance included), a provenance word
    // under any other code, and a wrong code all come back unchanged, so each case's own assert_eq on
    // the code still decides (a clash case expecting 1 and getting 0 or 4 fails as before)
    assert_eq!(named(fake(4, "Error: a compose that failed some other way")).status.code(), Some(4));
    assert_eq!(named(fake(4, "Error: part A's provenance line names no commit")).status.code(), Some(4));
    assert_eq!(named(fake(1, "CLASH 0,0,0 (the word provenance in a reason)")).status.code(), Some(1));
    assert_eq!(named(fake(0, "")).status.code(), Some(0));
    assert_eq!(named(fake(3, "NOT EVIDENCE (--not-evidence): provenance (ironspec Nov 14 01:58 / 02:35 / 02:43): an UNSTAMPED binary")).status.code(), Some(3));
}

#[test]
#[cfg(unix)]
#[should_panic(expected = "binary unstamped: provenance refused")]
fn named_turns_only_a_provenance_exit_4_into_the_named_panic() {
    named(fake(4, "provenance (ironspec Nov 14 01:58 / 02:35 / 02:43): REFUSED: an UNSTAMPED binary; the worktree's build tree is 0736d554"));
}

#[test]
#[cfg(unix)]
#[should_panic(expected = "provenance refused (a dirty worktree or a stale stamp")]
fn named_names_the_gates_other_refusals_too() {
    named(fake(4, "provenance (ironspec Nov 14 01:58 / 02:35 / 02:43): REFUSED: the worktree at /tmp/w is DIRTY (its tree 81dc, HEAD's 5fb9) and no commit is cited"));
}
