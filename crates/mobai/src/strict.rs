//! (tickexact 2026-11-13, G1 on 6ec22ffd8 #8) Strict readings of a command's reply, so an error
//! reply never reads as a measurement: a test reads "Test passed" / "Test failed" and nothing else;
//! a data read reads "has the following entity data" / "Found no elements matching"; any other
//! reply (an error, an entity that isn't there, an unparsable line) fails the run.

use anyhow::{bail, Result};
use mctas::vanilla::VanillaServer;

/// The reply of an `execute if …` test: true on "Test passed", false on "Test failed", an error
/// otherwise.
pub fn classify_test(cmd: &str, out: &str) -> Result<bool> {
    let o = out.trim_start();
    if o.starts_with("Test passed") {
        Ok(true)
    } else if o.starts_with("Test failed") {
        Ok(false)
    } else {
        bail!("`{cmd}`: neither \"Test passed\" nor \"Test failed\": {}", out.trim())
    }
}

/// The reply of an `execute as <one entity> on <relation> if …` test: an EMPTY reply (the relation
/// forked to no context) is false; otherwise as `classify_test`.
pub fn classify_test_on(cmd: &str, out: &str) -> Result<bool> {
    if out.trim().is_empty() {
        return Ok(false);
    }
    classify_test(cmd, out).map_err(|_| anyhow::anyhow!("`{cmd}`: neither \"Test passed\", \"Test failed\" nor an empty reply: {}", out.trim()))
}

/// The reply of a `data get entity <one entity> <path>`: whether the path holds a value; an error
/// when the entity isn't found or the reply is anything else.
pub fn classify_data(cmd: &str, out: &str) -> Result<bool> {
    if out.contains("has the following entity data") {
        Ok(true)
    } else if out.contains("Found no elements matching") {
        Ok(false)
    } else {
        bail!("`{cmd}`: neither entity data nor \"Found no elements matching\": {}", out.trim())
    }
}

/// `execute if …` (any test), read strictly.
pub fn test(srv: &mut VanillaServer, cmd: &str) -> Result<bool> {
    let out = srv.cmd_long(cmd)?;
    classify_test(cmd, &out)
}

/// `execute as <one entity> on <relation> if …`, read strictly (an empty reply = no context). The
/// caller checks the entity itself exists first (bed-trap reads its Pos).
pub fn test_on(srv: &mut VanillaServer, cmd: &str) -> Result<bool> {
    let out = srv.cmd_long(cmd)?;
    classify_test_on(cmd, &out)
}

/// `data get entity <one entity> <path>`, read strictly.
pub fn has_data(srv: &mut VanillaServer, cmd: &str) -> Result<bool> {
    let out = srv.cmd_long(cmd)?;
    classify_data(cmd, &out)
}

/// `data get entity <one entity> <path>`, read strictly: the value, None on "Found no elements
/// matching", an error otherwise.
pub fn data_value(srv: &mut VanillaServer, cmd: &str) -> Result<Option<String>> {
    let out = srv.cmd_long(cmd)?;
    Ok(if classify_data(cmd, &out)? { out.split("entity data: ").nth(1).map(|s| s.trim().to_string()) } else { None })
}

#[cfg(test)]
mod tests {
    use super::{classify_data, classify_test, classify_test_on};

    // (G1 on G #10) the readers' own classifiers, against the reply shapes 26.3 prints (rcon)
    #[test]
    fn reply_shapes() {
        for (reply, want) in [("Test passed", Some(true)), ("Test passed, count: 1", Some(true)), ("Test failed", Some(false)), ("No entity was found", None), ("Unknown or incomplete command", None), ("", None)] {
            assert_eq!(classify_test("t", reply).ok(), want, "test: {reply:?}");
        }
        for (reply, want) in [("Test passed", Some(true)), ("Test failed", Some(false)), ("", Some(false)), ("  ", Some(false)), ("Unknown or incomplete command", None)] {
            assert_eq!(classify_test_on("t", reply).ok(), want, "test_on: {reply:?}");
        }
        for (reply, want) in [("Villager has the following entity data: 300s", Some(true)), ("Found no elements matching Brain.memories", Some(false)), ("No entity was found", None), ("", None)] {
            assert_eq!(classify_data("d", reply).ok(), want, "data: {reply:?}");
        }
    }
}
