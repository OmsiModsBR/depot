//! The helper process: the game binary started again as `depot --preview-helper`, drawing
//! what the launcher asks for without a window.

use super::super::domain::{
    look::Look,
    message::{Cmd, Msg},
    shape::Shape,
};
use super::{kept::*, preview::*, scene::*, wire::*};
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, TryRecvError};
use std::time::{Duration, Instant};

/// The helper: shows what `cmds` asks for and hands each picture to `send` (false when
/// the launcher is gone), until `cmds` ends.
pub fn serve(cmds: Receiver<Cmd>, mut send: impl FnMut(Msg) -> bool) -> anyhow::Result<()> {
    crate::core::launcher_statics();
    let settings = crate::settings::Settings::load();
    let instance = crate::core::startup::graphics_instance();
    // (the renderer's setup waited for on the heap, not on this thread's stack)
    let mut renderer = pollster::block_on(Box::pin(omsi_render::Renderer::new_with(
        &instance,
        None,
        Some(wgpu::TextureFormat::Rgba8UnormSrgb),
        showroom_options(&settings),
    )))?;
    log::info!("preview helper: drawing on {}", renderer.adapter_name);
    let mut room = Showroom::new();
    let mut size: Option<(u32, u32)> = None;
    let mut look: Option<Look> = None;
    let mut frames = 0;
    let mut last_px: Option<(u32, u32, Vec<u8>)> = None;
    let mut cached: Option<(u64, Shape)> = None;
    let (kept_tx, kept_rx) = channel::<(u64, Shape)>();
    let keep = std::env::var_os(NO_KEEP).is_none();
    // (none said yet: the first is always sent)
    let mut status = (true, Some(String::new()));
    let mut last = Instant::now();
    loop {
        let mut apply = |c: Cmd, room: &mut Showroom| match c {
            Cmd::Look(l) => {
                look = Some(l.clone());
                room.want(l);
            }
            Cmd::Size(w, h) => size = Some((w.clamp(16, 4096), h.clamp(16, 2304))),
            Cmd::Orbit(dx, dy) => room.orbit(dx, dy),
            Cmd::Zoom(k) => room.zoom_by(k),
            Cmd::Focus(f) => room.focus_x = f,
        };
        match cmds.recv_timeout(Duration::from_millis(16)) {
            Ok(c) => apply(c, &mut room),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        }
        loop {
            match cmds.try_recv() {
                Ok(c) => apply(c, &mut room),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f32().min(0.1);
        last = now;
        room.update(&renderer, dt);
        if let Some((w, h)) = size {
            room.preview(&mut renderer, w, h);
        }
        if room.frames != frames {
            frames = room.frames;
            if let (Some((w, h, px)), Some(t)) =
                (room.pixels(&renderer), room.shown_look().map(tag))
            {
                if !send(Msg::Frame(w, h, t, px.clone())) {
                    return Ok(());
                }
                last_px = Some((w, h, px));
            }
        }
        // the settled picture is kept for the next time this bus is chosen (not by the game's
        // helper: its pictures are of the game's window, and turned by the player)
        if let (true, Some(l), Some((w, h, px))) =
            (room.settled() && keep, look.as_ref(), last_px.as_ref())
        {
            let key = (tag(l), Shape::of(*w, *h, room.focus_x));
            if cached != Some(key) {
                cached = Some(key);
                let (w, h, px, l) = (*w, *h, px.clone(), l.clone());
                let kept_tx = kept_tx.clone();
                std::thread::spawn(move || {
                    let file = cache_file(&l, key.1);
                    let _ = std::fs::create_dir_all(file.parent().unwrap_or(&file));
                    if let Some(mut img) = image::RgbaImage::from_raw(w, h, px) {
                        if w > KEEP_W {
                            img = image::imageops::resize(
                                &img,
                                KEEP_W,
                                (h as f32 * KEEP_W as f32 / w as f32).round() as u32,
                                image::imageops::FilterType::Triangle,
                            );
                        }
                        // (written beside it and then put in its place: a helper ended in the
                        // middle of writing leaves no half picture to be shown next time)
                        let part = file.with_extension("part");
                        match img
                            .save_with_format(&part, image::ImageFormat::Png)
                            .and_then(|_| {
                                std::fs::rename(&part, &file).map_err(image::ImageError::IoError)
                            }) {
                            Ok(()) => drop(kept_tx.send(key)),
                            Err(e) => {
                                log::warn!("preview helper: {}: {e}", file.display());
                                let _ = std::fs::remove_file(&part);
                            }
                        }
                    }
                    if let Some(dir) = file.parent() {
                        prune(
                            dir,
                            if key.1 == thumb_shape() {
                                KEEP_SMALL
                            } else {
                                KEEP
                            },
                        );
                    }
                });
            }
        }
        while let Ok((t, shape)) = kept_rx.try_recv() {
            if !send(Msg::Kept(t, shape)) {
                return Ok(());
            }
        }
        let now_status = (room.busy, room.error.clone());
        if now_status != status {
            status = now_status;
            if !send(Msg::Status {
                busy: status.0,
                error: status.1.clone(),
            }) {
                return Ok(());
            }
        }
    }
}

/// `depot --preview-helper`: the helper on its standard input and output.
pub fn helper_main() -> anyhow::Result<()> {
    // A helper that crashes ends at once, with no "stopped working" window for the player
    // and not held open by the error reporting meanwhile: the launcher sees it end, and
    // starts the next one on another graphics interface.
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Diagnostics::Debug::{
            SetErrorMode, SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX,
        };
        SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
    }
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            match serde_json::from_str::<Cmd>(&line) {
                Ok(c) => {
                    if tx.send(c).is_err() {
                        break;
                    }
                }
                Err(e) => log::warn!("preview helper: {e}: {line}"),
            }
        }
        // The launcher closed the helper's input, or ended: so does the helper - in order
        // when it is drawing (`serve` sees the input end), and a moment later in any case: in
        // the middle of opening its renderer or reading a bus it does not look, and stayed on
        // alone until that was done. (Not a bus that hung: the mark of one is not left.)
        std::thread::sleep(Duration::from_secs(1));
        super::scene::clear_placing_mark();
        std::process::exit(0);
    });
    // (each message in one write, the output locked only for it: a stray print of another
    // thread waits its turn, and `read_msg` passes over it)
    // The drawing on a thread with a stack of its own: on the process's main thread (1 MB on
    // Windows when the program does not say otherwise) opening the renderer overflowed it, and
    // the helper ended before it kept a single picture.
    std::thread::Builder::new()
        .name("preview-helper".into())
        .stack_size(HELPER_STACK)
        .spawn(move || {
            let out = std::io::stdout();
            let mut buf = Vec::new();
            serve(rx, |m| {
                buf.clear();
                if write_msg(&mut buf, &m).is_err() {
                    return false;
                }
                let mut o = out.lock();
                o.write_all(&buf).and_then(|_| o.flush()).is_ok()
            })
        })?
        .join()
        .map_err(|_| anyhow::anyhow!("the preview helper stopped on an error"))?
}

/// The stack of the thread that draws the preview (as much as the game's main thread has).
pub(super) const HELPER_STACK: usize = 64 << 20;

/// The helper's log: its lines, written again for each helper (the launcher's own window has
/// no console on Windows, and a helper's lines on its error output went nowhere).
pub fn helper_log() -> PathBuf {
    omsi_launcher_lib::data_dir().join("preview-helper.log")
}

/// The graphics interfaces the helper is tried on, in turn, as `OMSI_BACKEND` names them
/// (None: the settings' own), for the settings' graphics interface `chosen`. On Windows
/// Vulkan first, whatever the player chose for the game, then the one chosen: on an Intel UHD
/// 630 the helper's DirectX 12 device overflowed its stack compiling the scene's shaders (the
/// game's own did not), and on Vulkan it drew. The helper only draws a picture of the bus, it
/// need not draw it as the game does. A helper that ends, or goes `FIRST_PICTURE_LIMIT`
/// without a word before its first picture, is started again on the next one. `OMSI_BACKEND` set for the
/// launcher holds for the helper too.
pub fn helper_backends(chosen: &str) -> Vec<Option<&'static str>> {
    if !cfg!(windows) || omsi_cfg::flags::BACKEND.is_set() {
        return vec![None];
    }
    // (the names the game's own start reads, see `startup::backend_order`)
    let chosen = match chosen.trim().to_ascii_lowercase().as_str() {
        "dx12" | "directx" | "d3d12" => Some("dx12"),
        "gl" | "opengl" | "gles" => Some("gl"),
        _ => None,
    };
    let rest = ["dx12", "gl"].into_iter().filter(|b| Some(*b) != chosen);
    std::iter::once("vulkan")
        .chain(chosen)
        .chain(rest)
        .map(Some)
        .collect()
}

/// How long a helper that has not sent its first picture may go without a word before it is
/// taken for one that hung (stopped, and started on the next graphics interface): a backstop
/// for a helper held open after a crash, its output never closing. Generous: opening the
/// renderer alone took 70-80 s on a software driver.
pub const FIRST_PICTURE_LIMIT: Duration = Duration::from_secs(240);
