//! Starting helper programs (Demucs, Python, add-ons) the same way on
//! every platform.

use std::process::Command;

/// `cmd` set up for a program DJ Sugar runs in the background: on Windows it
/// gets no console window of its own, which would otherwise flash up from
/// an app that has none.
pub fn background(mut cmd: Command) -> Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = &mut cmd;
    cmd
}

/// A program's file name here: `name.exe` on Windows, `name` elsewhere.
pub fn exe_name(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}
