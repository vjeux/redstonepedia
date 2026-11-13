//! (tickexact 2026-11-13, G1 on 6ec22ffd8 #8) Strict readings of a command's reply, so an error
//! reply never reads as a measurement: a test reads "Test passed" / "Test failed" and nothing else;
//! a data read reads "has the following entity data" / "Found no elements matching"; any other
//! reply (an error, an entity that isn't there, an unparsable line) fails the run.

use anyhow::{bail, Result};
use mctas::vanilla::VanillaServer;

/// `execute if …` (any test): true on "Test passed", false on "Test failed", an error otherwise.
pub fn test(srv: &mut VanillaServer, cmd: &str) -> Result<bool> {
    let out = srv.cmd_long(cmd)?;
    let o = out.trim_start();
    if o.starts_with("Test passed") {
        Ok(true)
    } else if o.starts_with("Test failed") {
        Ok(false)
    } else {
        bail!("`{cmd}`: neither \"Test passed\" nor \"Test failed\": {}", out.trim())
    }
}

/// `execute as <one entity> on <relation> if …`: true on "Test passed", false on "Test failed" or
/// on an EMPTY reply (the relation forked to no context: no vehicle, no passenger), an error on
/// anything else. The caller checks the entity itself exists first (bed-trap reads its Pos).
pub fn test_on(srv: &mut VanillaServer, cmd: &str) -> Result<bool> {
    let out = srv.cmd_long(cmd)?;
    if out.trim().is_empty() {
        return Ok(false);
    }
    let o = out.trim_start();
    if o.starts_with("Test passed") {
        Ok(true)
    } else if o.starts_with("Test failed") {
        Ok(false)
    } else {
        bail!("`{cmd}`: neither \"Test passed\", \"Test failed\" nor an empty reply: {}", out.trim())
    }
}

/// `data get entity <one entity> <path>`: whether the path holds a value; an error when the entity
/// isn't found or the reply is anything else.
pub fn has_data(srv: &mut VanillaServer, cmd: &str) -> Result<bool> {
    let out = srv.cmd_long(cmd)?;
    if out.contains("has the following entity data") {
        Ok(true)
    } else if out.contains("Found no elements matching") {
        Ok(false)
    } else {
        bail!("`{cmd}`: neither entity data nor \"Found no elements matching\": {}", out.trim())
    }
}

#[cfg(test)]
mod tests {
    // The reply shapes the two readers accept, as 26.3 prints them (rcon). The readers themselves
    // need a server; these pin the strings they match against.
    #[test]
    fn reply_shapes() {
        for (reply, want) in [("Test passed", Some(true)), ("Test passed, count: 1", Some(true)), ("Test failed", Some(false)), ("No entity was found", None), ("Unknown or incomplete command", None)] {
            let o = reply.trim_start();
            let got = if o.starts_with("Test passed") {
                Some(true)
            } else if o.starts_with("Test failed") {
                Some(false)
            } else {
                None
            };
            assert_eq!(got, want, "{reply}");
        }
    }
}
