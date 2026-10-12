//! GPU time per render pass from timestamp queries (OMSI_GPU_TIMERS).

use std::sync::Arc;

impl GpuTimers {
    /// Take in the pass times of the last timed frame once its readback has arrived.
    pub(crate) fn collect(&mut self, period: f64) {
        let t = self;
        if !t.waiting || !t.ready.swap(false, std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        let n = t.pending.len() * 2;
        {
            let view = t.read.slice(0..n as u64 * 8).get_mapped_range();
            let stamps: &[u64] = bytemuck::cast_slice(&view[..n * 8]);
            // The passes in the order the GPU finished them, each counted from where the one
            // before it ended (the untimed passes in between go to the next timed one). A
            // tile-based GPU takes a pass's first stamp when its vertex stage starts, well
            // before the pass in front of it has finished its fragments: measured from their
            // own first stamps the post passes of the enhanced path overlapped and added up
            // to 25 ms of a 10 ms frame.
            if omsi_cfg::env::var_os("OMSI_GPU_TIMERS_RAW").is_some() {
                log::info!(
                    "gpu stamps: {:?}",
                    t.pending
                        .iter()
                        .enumerate()
                        .map(|(k, label)| (*label, stamps[k * 2], stamps[k * 2 + 1]))
                        .collect::<Vec<_>>()
                );
            }
            let mut order: Vec<(u64, u64, &'static str)> = t
                .pending
                .iter()
                .enumerate()
                .map(|(k, label)| (stamps[k * 2], stamps[k * 2 + 1], *label))
                .filter(|(a, b, _)| *b >= *a && *b > 0)
                .collect();
            order.sort_by_key(|(_, b, _)| *b);
            let mut prev: Option<u64> = None;
            for (a, b, label) in &order {
                let from = prev.unwrap_or(*a);
                let e = t.totals.entry(label).or_default();
                e.0 += b.saturating_sub(from) as f64 * period * 1e-9;
                e.1 += 1;
                prev = Some(*b);
            }
            if let (Some(first), Some(last)) = (order.iter().map(|o| o.0).min(), order.last()) {
                let e = t.totals.entry("(all passes)").or_default();
                e.0 += last.1.saturating_sub(first) as f64 * period * 1e-9;
                e.1 += 1;
            }
        }
        t.read.unmap();
        t.waiting = false;
    }
}

/// GPU time per render pass from timestamp queries (OMSI_GPU_TIMERS). One frame is timed
/// at a time: its readback has to arrive before the next one is measured. The passes
/// partition the frame: each counts from the end of the one the GPU finished before it, so
/// untimed passes are in the next timed one's figure and the figures add up to
/// "(all passes)".
pub(crate) struct GpuTimers {
    pub(crate) set: wgpu::QuerySet,
    pub(crate) resolve: wgpu::Buffer,
    pub(crate) read: wgpu::Buffer,
    /// The passes of the frame being read back, in query order.
    pub(crate) pending: Vec<&'static str>,
    /// The frame's passes are timed, their stamps not yet resolved (see `collect_gpu_timers`).
    pub(crate) unresolved: bool,
    pub(crate) waiting: bool,
    pub(crate) ready: Arc<std::sync::atomic::AtomicBool>,
    /// pass → (seconds, frames)
    pub(crate) totals: std::collections::BTreeMap<&'static str, (f64, u32)>,
}

pub(crate) const GPU_TIMER_PASSES: u32 = 16;

impl GpuTimers {
    pub(crate) fn new(device: &wgpu::Device) -> Option<GpuTimers> {
        if omsi_cfg::env::var_os("OMSI_GPU_TIMERS").is_none()
            || !device.features().contains(wgpu::Features::TIMESTAMP_QUERY)
        {
            return None;
        }
        let set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("pass timers"),
            ty: wgpu::QueryType::Timestamp,
            count: GPU_TIMER_PASSES * 2,
        });
        let size = (GPU_TIMER_PASSES as u64 * 16).div_ceil(wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT)
            * wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
        let resolve = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pass timers"),
            size,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pass timers read"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Some(GpuTimers {
            set,
            resolve,
            read,
            pending: Vec::new(),
            unresolved: false,
            waiting: false,
            ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            totals: Default::default(),
        })
    }
}

/// The timestamp writes of the next timed pass (none when this frame is not timed).
pub(crate) fn pass_timer<'a>(
    set: Option<&'a wgpu::QuerySet>,
    timed: &mut Vec<&'static str>,
    label: &'static str,
) -> Option<wgpu::RenderPassTimestampWrites<'a>> {
    let set = set?;
    if timed.len() as u32 >= GPU_TIMER_PASSES {
        return None;
    }
    let i = timed.len() as u32 * 2;
    timed.push(label);
    Some(wgpu::RenderPassTimestampWrites {
        query_set: set,
        beginning_of_pass_write_index: Some(i),
        end_of_pass_write_index: Some(i + 1),
    })
}
