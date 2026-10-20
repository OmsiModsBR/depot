//! The launcher's bus preview, drawn by a helper process.
//!
//! The launcher's window has no 3D renderer (see `gfx`): the chosen bus is drawn by the game
//! binary started again as `depot --preview-helper`. The helper opens the game's renderer
//! without a window, reads the bus with the showroom (`showroom`), and sends each picture
//! back over its standard output, while the launcher tells it what to show (the bus and its
//! light, the size, the mouse turning it) over its standard input. It runs only while a page
//! shows the preview, ends a moment after the page is left, and ends with the launcher (its
//! input closes then). The last settled picture of each bus and paint is kept in
//! `<data>/previews` and shown at once the next time that bus is chosen, until the helper
//! has drawn the live one. On a phone the helper runs on a thread of the launcher instead
//! (an app does not start processes of its own).

use super::showroom::{Look, Showroom};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, Read, Write};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::time::{Duration, Instant};

/// What the launcher tells the helper.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Cmd {
    /// Show this bus in this light.
    Look(Look),
    /// The picture's size in pixels.
    Size(u32, u32),
    /// The mouse dragged over the picture (see `Showroom::orbit`).
    Orbit(f32, f32),
    Zoom(f32),
    /// Where the bus stands across the picture (see `Showroom::focus_x`).
    Focus(f32),
}

/// What the helper sends back.
#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    /// A picture: width, height, RGBA (sRGB).
    Frame(u32, u32, Vec<u8>),
    /// Whether a bus is being read, and why the last one could not be shown.
    Status { busy: bool, error: Option<String> },
}

#[derive(Serialize, Deserialize)]
struct Status {
    busy: bool,
    error: Option<String>,
}

/// Every message starts with these bytes, then its kind and length: a line written to the
/// output by anything else is passed over, not taken for a picture.
const MAGIC: &[u8; 4] = b"DPV1";

pub fn write_msg(w: &mut impl Write, m: &Msg) -> std::io::Result<()> {
    let (kind, body) = match m {
        Msg::Frame(fw, fh, px) => {
            let mut b = Vec::with_capacity(8 + px.len());
            b.extend_from_slice(&fw.to_le_bytes());
            b.extend_from_slice(&fh.to_le_bytes());
            b.extend_from_slice(px);
            (b'F', b)
        }
        Msg::Status { busy, error } => (b'S', serde_json::to_vec(&Status { busy: *busy, error: error.clone() }).unwrap_or_default()),
    };
    w.write_all(MAGIC)?;
    w.write_all(&[kind])?;
    w.write_all(&(body.len() as u32).to_le_bytes())?;
    w.write_all(&body)?;
    w.flush()
}

/// The largest picture taken (a 4K window's preview), against a stream gone wrong.
const MAX_BODY: usize = 8 + 4096 * 2304 * 4;

pub fn read_msg(r: &mut impl Read) -> std::io::Result<Msg> {
    let bad = |what: &str| std::io::Error::new(std::io::ErrorKind::InvalidData, what.to_string());
    // (what is not a message - a line some library printed - is passed over up to the next)
    let mut head = [0u8; 9];
    r.read_exact(&mut head[..4])?;
    let mut skipped = 0usize;
    while &head[..4] != MAGIC {
        head.copy_within(1..4, 0);
        r.read_exact(&mut head[3..4])?;
        skipped += 1;
    }
    if skipped > 0 {
        log::debug!("preview helper: {skipped} bytes that were not a message passed over");
    }
    r.read_exact(&mut head[4..])?;
    let len = u32::from_le_bytes([head[5], head[6], head[7], head[8]]) as usize;
    if len > MAX_BODY {
        return Err(bad("preview message too long"));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body)?;
    match head[4] {
        b'F' if len >= 8 => {
            let w = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
            let h = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
            if (w as usize) * (h as usize) * 4 != len - 8 {
                return Err(bad("preview picture of the wrong size"));
            }
            body.drain(..8);
            Ok(Msg::Frame(w, h, body))
        }
        b'S' => {
            let s: Status = serde_json::from_slice(&body).map_err(|_| bad("preview status"))?;
            Ok(Msg::Status { busy: s.busy, error: s.error })
        }
        _ => Err(bad("unknown preview message")),
    }
}

/// The file the settled picture of `look`'s bus and paint is kept in. Its name changes with
/// the bus file and its folder (a bus changed or a paint added is drawn anew, not shown as it
/// was) and with the build (which draws it).
fn cache_file(look: &Look) -> PathBuf {
    let bus = look.root.join(&look.bus);
    let stamp = |p: &std::path::Path| {
        std::fs::metadata(p)
            .map(|m| {
                let t = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0);
                format!("{t}:{}", m.len())
            })
            .unwrap_or_default()
    };
    let key = [look.root.to_string_lossy().to_string(), look.bus.clone(), look.paint.clone(), stamp(&bus), bus.parent().map(stamp).unwrap_or_default(), crate::BUILD.to_string()].join("\0");
    // (FNV-1a: the same name from one run to the next)
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in key.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    omsi_launcher_lib::data_dir().join("previews").join(format!("{h:016x}.png"))
}

/// How many kept pictures there are at most (the oldest go).
const KEEP: usize = 300;

/// Only the `keep` newest pictures in `dir`.
fn prune(dir: &std::path::Path, keep: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = rd
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "png"))
        .map(|e| (e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH), e.path()))
        .collect();
    if files.len() <= keep {
        return;
    }
    files.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, f) in files.drain(keep..) {
        let _ = std::fs::remove_file(f);
    }
}

/// The helper: shows what `cmds` asks for and hands each picture to `send` (false when
/// the launcher is gone), until `cmds` ends.
pub fn serve(cmds: Receiver<Cmd>, mut send: impl FnMut(Msg) -> bool) -> anyhow::Result<()> {
    crate::launcher_statics();
    let settings = crate::settings::Settings::load();
    let instance = crate::startup::graphics_instance();
    let mut renderer = pollster::block_on(omsi_render::Renderer::new_with(&instance, None, Some(wgpu::TextureFormat::Rgba8UnormSrgb), super::showroom_options(&settings)))?;
    log::info!("preview helper: drawing on {}", renderer.adapter_name);
    let mut room = Showroom::new();
    let mut size: Option<(u32, u32)> = None;
    let mut look: Option<Look> = None;
    let mut frames = 0;
    let mut last_px: Option<(u32, u32, Vec<u8>)> = None;
    let mut cached: Option<PathBuf> = None;
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
            if let Some((w, h, px)) = room.pixels(&renderer) {
                if !send(Msg::Frame(w, h, px.clone())) {
                    return Ok(());
                }
                last_px = Some((w, h, px));
            }
        }
        // the settled picture is kept for the next time this bus is chosen
        if let (true, Some(l), Some((w, h, px))) = (room.settled(), look.as_ref(), last_px.as_ref()) {
            let file = cache_file(l);
            if cached.as_ref() != Some(&file) {
                cached = Some(file.clone());
                let (w, h, px) = (*w, *h, px.clone());
                std::thread::spawn(move || {
                    let _ = std::fs::create_dir_all(file.parent().unwrap_or(&file));
                    if let Some(img) = image::RgbaImage::from_raw(w, h, px) {
                        if let Err(e) = img.save(&file) {
                            log::warn!("preview helper: {}: {e}", file.display());
                        }
                    }
                    if let Some(dir) = file.parent() {
                        prune(dir, KEEP);
                    }
                });
            }
        }
        let now_status = (room.busy, room.error.clone());
        if now_status != status {
            status = now_status;
            if !send(Msg::Status { busy: status.0, error: status.1.clone() }) {
                return Ok(());
            }
        }
    }
}

/// `depot --preview-helper`: the helper on its standard input and output.
pub fn helper_main() -> anyhow::Result<()> {
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
        // (the launcher closed the helper's input, or ended: so does the helper)
    });
    let mut out = std::io::BufWriter::new(std::io::stdout().lock());
    serve(rx, |m| write_msg(&mut out, &m).is_ok())
}

/// The way to a running helper.
struct Link {
    cmds: Sender<Cmd>,
    msgs: Receiver<Msg>,
    child: Option<std::process::Child>,
}

impl Link {
    /// The helper started: the game binary again on a computer, a thread on a phone.
    fn start(root: &std::path::Path) -> Result<Link, String> {
        let root = root.to_path_buf();
        let (cmds, cmd_rx) = channel::<Cmd>();
        let (msg_tx, msgs) = channel::<Msg>();
        if cfg!(target_os = "android") {
            std::thread::Builder::new()
                .name("preview-helper".into())
                .spawn(move || {
                    if let Err(e) = serve(cmd_rx, |m| msg_tx.send(m).is_ok()) {
                        log::warn!("preview helper: {e:#}");
                    }
                })
                .map_err(|e| e.to_string())?;
            return Ok(Link { cmds, msgs, child: None });
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let mut child = std::process::Command::new(&exe)
            .arg("--preview-helper")
            .arg("--root")
            .arg(&root)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .map_err(|e| format!("{}: {e}", exe.display()))?;
        let mut stdin = child.stdin.take().ok_or("no input to the helper")?;
        let mut stdout = child.stdout.take().ok_or("no output from the helper")?;
        std::thread::spawn(move || {
            for c in cmd_rx {
                let Ok(line) = serde_json::to_string(&c) else { continue };
                if writeln!(stdin, "{line}").and_then(|_| stdin.flush()).is_err() {
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
        log::info!("launcher: preview helper started ({})", child.id());
        Ok(Link { cmds, msgs, child: Some(child) })
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        // its input closes (with the last sender of `cmds`) and it ends by itself, in order:
        // ended at once, it left the showroom's mark behind and the bus it was reading was
        // taken for one that had crashed the next time (see `Showroom::start_loading`)
        drop(std::mem::replace(&mut self.cmds, channel().0));
        if let Some(mut c) = self.child.take() {
            // (waited for off the launcher's thread; one that does not end is ended)
            std::thread::spawn(move || {
                let until = Instant::now() + Duration::from_secs(3);
                while Instant::now() < until {
                    if !matches!(c.try_wait(), Ok(None)) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                log::warn!("launcher: the preview helper did not end, it is stopped");
                let _ = c.kill();
                let _ = c.wait();
            });
        }
    }
}

/// How long the preview stays drawn by the helper after no page shows it.
const LINGER: f32 = 2.0;

/// The launcher's side of the preview.
pub struct Preview {
    wanted: Option<Look>,
    link: Option<Link>,
    sent: (Option<Look>, Option<(u32, u32)>, f32),
    size: Option<(u32, u32)>,
    pub focus_x: f32,
    /// A picture not yet on the GPU (`take_frame`), and how many have come.
    frame: Option<(u32, u32, Vec<u8>)>,
    pub generation: u64,
    picture: bool,
    /// The picture shown is the one kept from an earlier time, not yet the helper's.
    pub from_cache: bool,
    cache: Option<(PathBuf, Receiver<Option<(u32, u32, Vec<u8>)>>)>,
    cache_for: Option<PathBuf>,
    pub busy: bool,
    pub error: Option<String>,
    /// The helper is running and has drawn since it started.
    pub live: bool,
    unused: f32,
    /// The helper could not be started, or stopped on its own (not tried again for the same
    /// bus).
    broken: Option<Look>,
}

impl Default for Preview {
    fn default() -> Self {
        Preview::new()
    }
}

impl Preview {
    pub fn new() -> Preview {
        Preview {
            wanted: None,
            link: None,
            sent: (None, None, -1.0),
            size: None,
            focus_x: 0.5,
            frame: None,
            generation: 0,
            picture: false,
            from_cache: false,
            cache: None,
            cache_for: None,
            busy: false,
            error: None,
            live: false,
            unused: 0.0,
            broken: None,
        }
    }

    /// Show this (a bus, its paint, the time and weather to light it with).
    pub fn want(&mut self, look: Look) {
        if self.wanted.as_ref() == Some(&look) {
            return;
        }
        let file = cache_file(&look);
        if self.cache_for.as_ref() != Some(&file) {
            self.cache_for = Some(file.clone());
            let (tx, rx) = channel();
            let f = file.clone();
            std::thread::spawn(move || {
                let img = image::open(&f).ok().map(|i| i.to_rgba8()).map(|i| (i.width(), i.height(), i.into_raw()));
                let _ = tx.send(img);
            });
            self.cache = Some((file, rx));
        }
        self.wanted = Some(look);
    }

    pub fn orbit(&mut self, dx: f32, dy: f32) {
        if let Some(l) = self.link.as_ref() {
            let _ = l.cmds.send(Cmd::Orbit(dx, dy));
        }
    }

    pub fn zoom_by(&mut self, k: f32) {
        if let Some(l) = self.link.as_ref() {
            let _ = l.cmds.send(Cmd::Zoom(k));
        }
    }

    /// Per frame: `shown` the size the preview is drawn at this frame (in pixels), None when
    /// no page shows it.
    pub fn update(&mut self, dt: f32, shown: Option<(u32, u32)>) {
        // the kept picture, read from its file
        if let Some((file, rx)) = self.cache.as_ref() {
            match rx.try_recv() {
                Ok(img) => {
                    if let (Some(img), false) = (img, self.live && self.cache_for.as_ref() == Some(file)) {
                        self.frame = Some(img);
                        self.generation += 1;
                        self.picture = true;
                        self.from_cache = true;
                    }
                    self.cache = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => self.cache = None,
            }
        }
        if let Some(s) = shown {
            self.size = Some(s);
            self.unused = 0.0;
        } else {
            self.unused += dt;
        }
        // the helper while a page shows the preview, and a moment after
        if self.unused > LINGER {
            if self.link.take().is_some() {
                log::info!("launcher: the preview is not shown any more, its helper ends");
            }
            self.live = false;
            self.sent = (None, None, -1.0);
            return;
        }
        let Some(look) = self.wanted.clone() else { return };
        if self.link.is_none() && self.broken.as_ref() != Some(&look) && shown.is_some() {
            match Link::start(&look.root) {
                Ok(l) => {
                    self.link = Some(l);
                    self.sent = (None, None, -1.0);
                    // (what an earlier helper said holds no more; this one says it anew)
                    self.busy = true;
                    self.error = None;
                }
                Err(e) => {
                    log::warn!("launcher: the preview helper could not be started: {e}");
                    self.error = Some("The 3D preview could not be started".into());
                    self.broken = Some(look.clone());
                }
            }
        }
        let Some(link) = self.link.as_ref() else { return };
        if self.sent.0.as_ref() != Some(&look) {
            let _ = link.cmds.send(Cmd::Look(look.clone()));
            self.sent.0 = Some(look.clone());
        }
        if self.size.is_some() && self.sent.1 != self.size {
            let (w, h) = self.size.unwrap_or((16, 16));
            let _ = link.cmds.send(Cmd::Size(w, h));
            self.sent.1 = self.size;
        }
        if (self.sent.2 - self.focus_x).abs() > 1e-4 {
            let _ = link.cmds.send(Cmd::Focus(self.focus_x));
            self.sent.2 = self.focus_x;
        }
        let mut ended = false;
        loop {
            match link.msgs.try_recv() {
                Ok(Msg::Frame(w, h, px)) => {
                    self.frame = Some((w, h, px));
                    self.generation += 1;
                    self.picture = true;
                    self.from_cache = false;
                    self.live = true;
                }
                Ok(Msg::Status { busy, error }) => {
                    self.busy = busy;
                    if error.is_some() {
                        self.picture = false;
                    }
                    self.error = error;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    ended = true;
                    break;
                }
            }
        }
        if ended {
            log::warn!("launcher: the preview helper stopped (see its lines in the log)");
            self.link = None;
            self.live = false;
            self.busy = false;
            self.error.get_or_insert_with(|| "The 3D preview stopped".into());
            self.broken = Some(look);
        }
    }

    /// The picture that came since the last call, to go to the GPU.
    pub fn take_frame(&mut self) -> Option<(u32, u32, Vec<u8>)> {
        self.frame.take()
    }

    /// A bus is there to show (live, or kept from an earlier time).
    pub fn has_picture(&self) -> bool {
        self.picture
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_go_through_the_pipe_as_they_were_sent() {
        let msgs = [
            Msg::Frame(2, 1, vec![1, 2, 3, 255, 4, 5, 6, 255]),
            Msg::Status { busy: true, error: None },
            Msg::Status { busy: false, error: Some("no bus".into()) },
        ];
        let mut buf = Vec::new();
        for m in &msgs {
            write_msg(&mut buf, m).unwrap();
        }
        let mut r = std::io::Cursor::new(buf);
        for m in &msgs {
            assert_eq!(&read_msg(&mut r).unwrap(), m);
        }
        assert!(read_msg(&mut r).is_err(), "nothing more");
    }

    #[test]
    fn a_stray_line_is_passed_over_and_a_wrong_size_refused() {
        let mut buf = b"hello world, a log line\n".to_vec();
        write_msg(&mut buf, &Msg::Status { busy: true, error: None }).unwrap();
        let mut r = std::io::Cursor::new(buf);
        assert_eq!(read_msg(&mut r).unwrap(), Msg::Status { busy: true, error: None });
        assert!(read_msg(&mut std::io::Cursor::new(b"only a line\n".to_vec())).is_err());
        let mut buf = Vec::new();
        write_msg(&mut buf, &Msg::Frame(2, 2, vec![0; 4])).unwrap();
        assert!(read_msg(&mut std::io::Cursor::new(buf)).is_err(), "a 2x2 picture is 16 bytes");
    }

    #[test]
    fn commands_are_one_line_each() {
        let cmds = [Cmd::Look(Look { bus: "Vehicles/A/a.bus".into(), time: 480, ..Default::default() }), Cmd::Size(800, 450), Cmd::Orbit(1.5, -2.0), Cmd::Zoom(0.9), Cmd::Focus(0.6)];
        for c in cmds {
            let line = serde_json::to_string(&c).unwrap();
            assert!(!line.contains('\n'));
            assert_eq!(serde_json::from_str::<Cmd>(&line).unwrap(), c);
        }
    }

    #[test]
    fn only_the_newest_pictures_are_kept() {
        let dir = std::env::temp_dir().join(format!("depot-previews-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for i in 0..5 {
            let f = dir.join(format!("{i}.png"));
            std::fs::write(&f, b"x").unwrap();
            let t = std::time::UNIX_EPOCH + Duration::from_secs(1_000_000 + i * 60);
            std::fs::File::options().write(true).open(&f).unwrap().set_modified(t).unwrap();
        }
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        prune(&dir, 2);
        let mut left: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
        left.sort();
        assert_eq!(left, ["3.png", "4.png", "notes.txt"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_kept_picture_is_per_bus_and_paint_not_per_light() {
        let a = Look { root: "/omsi".into(), bus: "Vehicles/A/a.bus".into(), paint: "Red".into(), time: 480, ..Default::default() };
        let b = Look { time: 1200, weather: "rain".into(), ..a.clone() };
        let c = Look { paint: "Blue".into(), ..a.clone() };
        assert_eq!(cache_file(&a), cache_file(&b));
        assert_ne!(cache_file(&a), cache_file(&c));
    }
}
