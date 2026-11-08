//! Launch the owner's own Chrome (his default profile, the one he is signed in with) on a URL.
//! A normal launch, exactly what clicking a link does: no DevTools port, no copied profile.

use anyhow::{Context, Result};
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};

const CHROME: &str = r"C:\Program Files\Google\Chrome\Application\chrome.exe";
const DETACHED_PROCESS: u32 = 0x0000_0008;
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// Our own standard handles are pipes back to WSL when started from there: a child that
/// inherited them would keep the caller waiting until Chrome exits. Make them non-inheritable.
fn no_inherit() {
    use windows::Win32::Foundation::{HANDLE_FLAG_INHERIT, HANDLE_FLAGS, SetHandleInformation};
    use windows::Win32::System::Console::{GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE};
    for which in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        // SAFETY: flag change on this process's own standard handles.
        unsafe {
            if let Ok(h) = GetStdHandle(which) {
                let _ = SetHandleInformation(h, HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0));
            }
        }
    }
}

/// Open `url` in a new window of the owner's Chrome (`Default` profile). Accessibility is
/// switched on for the pages so UI Automation sees them (a no-op when Chrome already runs: the
/// URL then opens in the running browser, which turns it on by itself for an automation client).
pub fn open(url: &str) -> Result<()> {
    no_inherit();
    Command::new(CHROME)
        .args(["--profile-directory=Default", "--force-renderer-accessibility", "--new-window", url])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
        .spawn()
        .with_context(|| format!("starting {CHROME}"))?;
    Ok(())
}
