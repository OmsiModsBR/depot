//! Running OmsiLaunch as a separate process. Everything that starts a program goes through
//! `OriginalEngineRunner`, so tests use the fake program of `tools/fake-omsilaunch`.

use std::io;
use std::path::Path;
use std::process::{Child, Command, Stdio};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunOutput {
    /// `None` when the program was ended by a signal.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub trait OriginalEngineRunner {
    /// Runs the program to its end and collects its output.
    fn run(&self, exe: &Path, args: &[String]) -> io::Result<RunOutput>;
    /// Starts the program and leaves it running (a game session).
    fn spawn(&self, exe: &Path, args: &[String]) -> io::Result<Child>;
}

/// The real thing: no console window, nothing inherited from the launcher's streams.
pub struct ProcessRunner;

fn command(exe: &Path, args: &[String]) -> Command {
    let mut c = Command::new(exe);
    c.args(args);
    // OmsiLaunch works relative to its own folder (the OMSI 2 folder)
    if let Some(dir) = exe.parent().filter(|d| d.is_dir()) {
        c.current_dir(dir);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

impl OriginalEngineRunner for ProcessRunner {
    fn run(&self, exe: &Path, args: &[String]) -> io::Result<RunOutput> {
        let o = command(exe, args).stdin(Stdio::null()).output()?;
        Ok(RunOutput {
            code: o.status.code(),
            stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
        })
    }

    fn spawn(&self, exe: &Path, args: &[String]) -> io::Result<Child> {
        command(exe, args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
    }
}
