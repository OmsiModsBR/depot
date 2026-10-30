//! The launcher's link to a running helper: its process (or thread, on a phone), the
//! messages to and from it, and its end with the launcher.

use super::super::domain::message::{Cmd, Msg};
use super::{helper::*, preview::*, wire::*};
use std::io::Write;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

/// The way to a running helper.
pub struct Link {
    pub cmds: Sender<Cmd>,
    pub msgs: Receiver<Msg>,
    pub child: Option<std::process::Child>,
    /// The file name of the helper's own mark of the bus it reads (None: the common one).
    pub mark: Option<&'static str>,
}

/// The mark of a bus a helper was reading taken away: `mark`'s, or the common one.
pub(super) fn clear_own_mark(mark: Option<&str>) {
    match mark {
        Some(n) => super::scene::clear_mark(n),
        None => super::scene::clear_placing_mark(),
    }
}

impl Link {
    /// The helper started: the game binary again on a computer, a thread on a phone.
    pub(super) fn start(
        root: &std::path::Path,
        backend: Option<&str>,
        mark: Option<&'static str>,
    ) -> Result<Link, String> {
        let root = root.to_path_buf();
        let (cmds, cmd_rx) = channel::<Cmd>();
        let (msg_tx, msgs) = channel::<Msg>();
        if cfg!(target_os = "android") {
            std::thread::Builder::new()
                .name("preview-helper".into())
                .stack_size(HELPER_STACK)
                .spawn(move || {
                    if let Err(e) = serve(cmd_rx, |m| msg_tx.send(m).is_ok()) {
                        log::warn!("preview helper: {e:#}");
                    }
                })
                .map_err(|e| e.to_string())?;
            return Ok(Link {
                cmds,
                msgs,
                child: None,
                mark: None,
            });
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let log_file = helper_log();
        let _ = std::fs::create_dir_all(log_file.parent().unwrap_or(&log_file));
        // (the last helper's lines stay beside it: the one that stopped is often the one before)
        let _ = std::fs::rename(&log_file, log_file.with_extension("previous.log"));
        let stderr = match std::fs::File::create(&log_file) {
            Ok(f) => std::process::Stdio::from(f),
            Err(_) => std::process::Stdio::null(),
        };
        let mut cmd = std::process::Command::new(&exe);
        cmd.arg("--preview-helper")
            .arg("--root")
            .arg(&root)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(stderr);
        if let Some(b) = backend {
            cmd.env(omsi_cfg::flags::BACKEND.name, b);
        }
        if let Some(m) = mark {
            cmd.env("DEPOT_PLACING_MARK", m);
        }
        if mark == Some(GAME_MARK) {
            cmd.env(NO_KEEP, "1");
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // (no console window of its own for the helper)
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = cmd.spawn().map_err(|e| format!("{}: {e}", exe.display()))?;
        #[cfg(windows)]
        end_with_launcher(&child);
        let mut stdin = child.stdin.take().ok_or("no input to the helper")?;
        let mut stdout = child.stdout.take().ok_or("no output from the helper")?;
        std::thread::spawn(move || {
            for c in cmd_rx {
                let Ok(line) = serde_json::to_string(&c) else {
                    continue;
                };
                if writeln!(stdin, "{line}")
                    .and_then(|_| stdin.flush())
                    .is_err()
                {
                    break;
                }
            }
        });
        std::thread::spawn(move || {
            let mut r = std::io::BufReader::with_capacity(1 << 20, &mut stdout);
            loop {
                match read_msg(&mut r) {
                    Ok(m) => {
                        if msg_tx.send(m).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        if e.kind() != std::io::ErrorKind::UnexpectedEof {
                            log::warn!("preview helper: {e}");
                        }
                        break;
                    }
                }
            }
        });
        log::info!(
            "launcher: preview helper started ({}{}), its log is {}",
            child.id(),
            backend.map(|b| format!(", on {b}")).unwrap_or_default(),
            log_file.display()
        );
        Ok(Link {
            cmds,
            msgs,
            child: Some(child),
            mark,
        })
    }
}

/// The helper ends with the launcher, however the launcher ends: it is put in a job of the
/// launcher's that the system closes, and its processes with it, when the launcher's process
/// is gone (a helper that crashed and is held open by the error reporting included, which
/// would not see its input close).
#[cfg(windows)]
fn end_with_launcher(child: &std::process::Child) {
    use std::os::windows::io::AsRawHandle;
    use std::sync::OnceLock;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
        SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    // (one job for every helper, its handle kept open until the launcher ends: 0 if none)
    static JOB: OnceLock<isize> = OnceLock::new();
    let job = *JOB.get_or_init(|| unsafe {
        let Ok(job) = CreateJobObjectW(None, windows::core::PCWSTR::null()) else {
            return 0;
        };
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let size = std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;
        match SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const _,
            size,
        ) {
            Ok(()) => job.0 as isize,
            Err(e) => {
                log::warn!("launcher: no job for the preview helper: {e}");
                0
            }
        }
    });
    if job == 0 {
        return;
    }
    if let Err(e) =
        unsafe { AssignProcessToJobObject(HANDLE(job as *mut _), HANDLE(child.as_raw_handle())) }
    {
        log::warn!("launcher: the preview helper is not in the launcher's job: {e}");
    }
}

/// How long a helper asked to end may take before it is stopped (reading a big map or bus is
/// not broken off in the middle: it ends after that).
const END_GRACE: Duration = Duration::from_secs(20);

impl Drop for Link {
    fn drop(&mut self) {
        // its input closes (with the last sender of `cmds`) and it ends by itself, in order
        drop(std::mem::replace(&mut self.cmds, channel().0));
        let mark = self.mark;
        if let Some(mut c) = self.child.take() {
            // (waited for off the launcher's thread)
            std::thread::spawn(move || {
                let until = Instant::now() + END_GRACE;
                while Instant::now() < until {
                    if !matches!(c.try_wait(), Ok(None)) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                log::warn!("launcher: the preview helper did not end, it is stopped");
                let _ = c.kill();
                let _ = c.wait();
                // A helper stopped here was ended by the launcher, not by the bus it was reading:
                // the showroom's mark of a bus that hung is not left for the next helper to take
                // as one (the bus would have been left out of the preview for good).
                clear_own_mark(mark);
            });
        }
    }
}
