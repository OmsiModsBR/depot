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
pub(crate) static BASIC_PIPELINES: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub(crate) fn basic_pipelines() -> bool {
    BASIC_PIPELINES.load(std::sync::atomic::Ordering::Relaxed)
        || omsi_cfg::flags::BASIC_PIPELINES.is_set()
}

/// The file that remembers, per graphics adapter, the reduced renderer that worked there
/// (`~/.openomsi/gpu-fallback.cfg`, lines `adapter|msaa|basic`).
pub(crate) fn fallback_path() -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    Some(
        std::path::PathBuf::from(home)
            .join(".depot")
            .join("gpu-fallback.cfg"),
    )
}

pub(crate) fn parse_fallbacks(text: &str) -> Vec<(String, u32, bool)> {
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
pub(crate) fn fallback_load(name: &str) -> Option<(u32, bool)> {
    let text = std::fs::read_to_string(fallback_path()?).ok()?;
    parse_fallbacks(&text)
        .into_iter()
        .find(|e| e.0 == name)
        .map(|e| (e.1, e.2))
}

/// Remember (`Some`) or forget (`None`) the reduced renderer for adapter `name`.
pub(crate) fn fallback_store(name: &str, what: Option<(u32, bool)>) {
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

#[cfg(test)]
pub(crate) mod fallback_tests {
    #[test]
    fn a_remembered_fallback_is_read_back() {
        let e =
            super::parse_fallbacks("Adreno (TM) 830 (Gl)|1|1\nNVIDIA | odd (Vulkan)|4|0\nbroken\n");
        assert_eq!(
            e,
            vec![
                ("Adreno (TM) 830 (Gl)".to_string(), 1, true),
                ("NVIDIA | odd (Vulkan)".to_string(), 4, false)
            ]
        );
    }
}

pub(crate) struct DevicePoller {
    pub(crate) stop: Arc<std::sync::atomic::AtomicBool>,
    pub(crate) thread: Option<std::thread::JoinHandle<()>>,
}

impl DevicePoller {
    /// The poller of `device`, which stops once the device is lost (`lost`): on Vulkan the
    /// poll of a lost device is a panic in wgpu, and this thread was the first to see the
    /// loss - its panic went to the log as the end of the game (see `lost`).
    pub(crate) fn start(device: &wgpu::Device, lost: LostFlag) -> Option<Self> {
        // Not on OpenGL: there every poll takes the one GL context, and whenever the thread
        // drawing held it for more than a second (a big shader linked while the world
        // loads, a slow chip's frame) this thread gave up with wgpu-hal's panic "Could not
        // lock adapter context" (#898, after #843). It is not needed there: every submit
        // of the frame runs the same upkeep (wgpu-core's `maintain` after `queue.submit`).
        if cfg!(target_arch = "wasm32") || gl_backend() || omsi_cfg::flags::NO_POLL_THREAD.is_set()
        {
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
pub(crate) fn poll_until(
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

#[cfg(test)]
mod poller_tests {
    use super::*;
    use crate::gpu::lost::mark_lost;
    use std::sync::atomic::{AtomicBool, Ordering};

    const NO_PAUSE: std::time::Duration = std::time::Duration::ZERO;

    #[test]
    fn a_stopped_poller_polls_no_more() {
        let stop = AtomicBool::new(true);
        let n = poll_until(&stop, &LostFlag::default(), NO_PAUSE, || true);
        assert_eq!(n, 0);
    }

    #[test]
    fn a_lost_device_is_not_polled() {
        let lost = LostFlag::default();
        mark_lost(&lost, "before");
        let n = poll_until(&AtomicBool::new(false), &lost, NO_PAUSE, || {
            unreachable!("a lost device was polled")
        });
        assert_eq!(n, 0);
    }

    #[test]
    fn the_poller_stops_at_the_poll_that_finds_the_device_lost() {
        let lost = LostFlag::default();
        let mut k = 0;
        let n = poll_until(&AtomicBool::new(false), &lost, NO_PAUSE, || {
            k += 1;
            k < 3
        });
        assert_eq!(n, 3);
    }

    #[test]
    fn the_poller_stops_once_the_loss_handler_marked_the_device() {
        let lost = LostFlag::default();
        let mut k = 0;
        let n = poll_until(&AtomicBool::new(false), &lost, NO_PAUSE, || {
            k += 1;
            if k == 2 {
                mark_lost(&lost, "the handler, between two polls");
            }
            true
        });
        assert_eq!(n, 2);
    }

    #[test]
    fn the_poller_stops_when_asked() {
        let stop = AtomicBool::new(false);
        let n = poll_until(&stop, &LostFlag::default(), NO_PAUSE, || {
            stop.store(true, Ordering::Relaxed);
            true
        });
        assert_eq!(n, 1);
    }

    #[test]
    fn the_poller_thread_runs_and_stops_on_a_real_device() {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::NOOP;
        descriptor.backend_options.noop = wgpu::NoopBackendOptions { enable: true };
        let instance = wgpu::Instance::new(descriptor);
        let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
        let (device, _queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let lost = LostFlag::default();
        let poller = DevicePoller::start(&device, lost.clone()).expect("a poller off OpenGL");
        std::thread::sleep(std::time::Duration::from_millis(5));
        // the device lost: the thread ends by itself, before it is asked to
        mark_lost(&lost, "test");
        let mut poller = poller;
        let t = poller.thread.take().unwrap();
        t.join().expect("the poller thread ended without a panic");
    }
}
