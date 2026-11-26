//! Opening the device: the reduced renderers remembered per adapter, the basic pipelines
//! and the thread that keeps the device polled.

use super::caps::gl_backend;
use super::lost::{guarded_poll, lost_why, GpuWaitError, LostFlag};
use std::sync::Arc;

/// The renderer is built without the pipelines that are not needed to draw a picture (the
/// snowfall, the lamps in the fog, the street lamps' shadow maps): a driver whose shader
/// compiler fails on one of them (it answers "out of memory" or an unknown error - phones'
/// drivers do) takes the whole device with it, and every frame after that came out black
/// (see `Renderer::new_on`). Once set, it stays for the rest of the run. OMSI_BASIC_PIPELINES=1
/// asks for it.
pub static BASIC_PIPELINES: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub fn basic_pipelines() -> bool {
    BASIC_PIPELINES.load(std::sync::atomic::Ordering::Relaxed)
        || crate::gpu::debug().basic_pipelines
}

/// The file that remembers, per graphics adapter, the reduced renderer that worked there
/// (`~/.depot/gpu-fallback.cfg`, lines `adapter|msaa|basic`).
pub fn fallback_path() -> Option<std::path::PathBuf> {
    Some(omsi_cfg::paths::data_dir()?.join("gpu-fallback.cfg"))
}

pub fn parse_fallbacks(text: &str) -> Vec<(String, u32, bool)> {
    text.lines()
        .filter_map(|l| {
            let mut f = l.rsplitn(3, '|');
            let basic = f.next()?.trim() == "1";
            let msaa = f.next()?.trim().parse().ok()?;
            Some((f.next()?.to_string(), msaa, basic))
        })
        .collect()
}

/// The reduced renderer (MSAA, basic pipelines) that worked on adapter `name` before.
pub fn fallback_load(name: &str) -> Option<(u32, bool)> {
    let text = std::fs::read_to_string(fallback_path()?).ok()?;
    parse_fallbacks(&text)
        .into_iter()
        .find(|e| e.0 == name)
        .map(|e| (e.1, e.2))
}

/// Remember (`Some`) or forget (`None`) the reduced renderer for adapter `name`.
pub fn fallback_store(name: &str, what: Option<(u32, bool)>) {
    let Some(path) = fallback_path() else { return };
    let mut all = std::fs::read_to_string(&path)
        .map(|t| parse_fallbacks(&t))
        .unwrap_or_default();
    all.retain(|e| e.0 != name);
    if let Some((m, b)) = what {
        all.push((name.to_string(), m, b));
    }
    let text: String = all
        .iter()
        .map(|(n, m, b)| format!("{n}|{m}|{}\n", *b as u8))
        .collect();
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(path, text);
}

pub struct DevicePoller {
    pub stop: Arc<std::sync::atomic::AtomicBool>,
    pub thread: Option<std::thread::JoinHandle<()>>,
}

impl DevicePoller {
    /// The poller of `device`, which stops once the device is lost (`lost`): on Vulkan the
    /// poll of a lost device is a panic in wgpu, and this thread was the first to see the
    /// loss - its panic went to the log as the end of the game (see `lost`).
    pub fn start(device: &wgpu::Device, lost: LostFlag) -> Option<Self> {
        // Not on OpenGL: there every poll takes the one GL context, and whenever the thread
        // drawing held it for more than a second (a big shader linked while the world
        // loads, a slow chip's frame) this thread gave up with wgpu-hal's panic "Could not
        // lock adapter context" (#898, after #843). It is not needed there: every submit
        // of the frame runs the same upkeep (wgpu-core's `maintain` after `queue.submit`).
        if cfg!(target_arch = "wasm32") || gl_backend() || crate::gpu::debug().no_poll_thread {
            return None;
        }
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let (device, flag) = (device.clone(), stop.clone());
        let thread = std::thread::Builder::new()
            .name("omsi-gpu-poll".into())
            .spawn(move || {
                let pause = std::time::Duration::from_millis(1);
                poll_until(&flag, &lost, pause, || {
                    let r = guarded_poll(Some(&lost), false, || device.poll(wgpu::PollType::Poll));
                    !matches!(r, Err(GpuWaitError::Lost))
                });
            })
            .ok()?;
        Some(Self {
            stop,
            thread: Some(thread),
        })
    }
}

/// The poller's loop: `poll()` every `pause` until `stop` is set, the device is lost
/// (`lost`) or `poll` says to stop (it found the device lost). How many polls were made.
pub fn poll_until(
    stop: &std::sync::atomic::AtomicBool,
    lost: &LostFlag,
    pause: std::time::Duration,
    mut poll: impl FnMut() -> bool,
) -> u64 {
    let mut n = 0;
    while !stop.load(std::sync::atomic::Ordering::Relaxed) && lost_why(lost).is_none() {
        n += 1;
        if !poll() {
            break;
        }
        std::thread::sleep(pause);
    }
    n
}

impl Drop for DevicePoller {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
