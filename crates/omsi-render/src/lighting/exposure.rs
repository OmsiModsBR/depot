//! Exposure: the metering's tuning, the tone curve's contrast and the log of the measured
//! exposure.

use super::smoothstep;
use std::sync::Arc;

/// The metering (see `meter_tuning`).
pub(crate) const METER_GAIN: f32 = 0.4;

pub(crate) const METER_TARGET: f32 = -2.84;

// (how far the eye adapts to what it looks at beyond what the light model knows: the sun
// in view, a dark cab or an underpass by day - two stops and a half and more for the
// eye; at the metering's gain a snow field still comes out darkened by well under a stop)
pub(crate) const METER_DARKEN: f32 = 2.0;

pub(crate) const METER_BRIGHTEN: f32 = 1.6;

/// The tone curve's contrast about mid grey by day and at night (see `tone_contrast`).
pub(crate) const TONE_CONTRAST_DAY: f32 = 1.22;

pub(crate) const TONE_CONTRAST_NIGHT: f32 = 0.94;

/// How far night vision takes the colour out of a dark scene (post.wgsl `night_vision`).
pub(crate) const NIGHT_VISION: f32 = 0.3;

/// The metering: the share of the metered difference that is corrected, its target (log2 of
/// the picture's mean luminance), how far it may darken and brighten (EV), the exposure
/// bias (EV) and the night vision strength. `OMSI_METER=gain,target,dark,bright,bias,night`
/// overrides them for tuning.
pub(crate) fn meter_tuning() -> [f32; 6] {
    static METER: std::sync::OnceLock<[f32; 6]> = std::sync::OnceLock::new();
    *METER.get_or_init(|| {
        let mut m = [
            METER_GAIN,
            METER_TARGET,
            METER_DARKEN,
            METER_BRIGHTEN,
            0.0,
            NIGHT_VISION,
        ];
        if let Ok(v) = omsi_cfg::env::var("OMSI_METER") {
            for (k, x) in v.split(',').take(6).enumerate() {
                if let Ok(x) = x.trim().parse() {
                    m[k] = x;
                }
            }
        }
        m
    })
}

/// The tone curve's contrast about mid grey for a pre-exposure (its natural log): a
/// camera's by day, falling off through the dusk to a little under none at night, where
/// the eye's own adaptation to the dark between the street lamps lifts it out of black
/// (a street a few hundred times darker than the pool under a lamp is still seen). `OMSI_TONE_CONTRAST=day,night`
/// overrides it.
pub(crate) fn tone_contrast(log_pre: f32) -> f32 {
    static OVERRIDE: std::sync::OnceLock<Option<(f32, f32)>> = std::sync::OnceLock::new();
    let (day, night) = OVERRIDE
        .get_or_init(|| {
            let v = omsi_cfg::env::var("OMSI_TONE_CONTRAST").ok()?;
            let mut it = v.split(',').map(|x| x.trim().parse::<f32>().ok());
            Some((it.next()??, it.next().flatten().unwrap_or(1.0)))
        })
        .unwrap_or((TONE_CONTRAST_DAY, TONE_CONTRAST_NIGHT));
    // (the pre-exposure is about 1 by day, 70 in the blue hour and 400-700 at night)
    let t = smoothstep(2.0, 8.0, log_pre / std::f32::consts::LN_2);
    day + (night - day) * t
}

/// `OMSI_DEBUG_EXPOSURE=1`: the enhanced path's exposure as it adapts - the light model's
/// pre-exposure, the metered picture and the correction the tone mapping applies, logged
/// about four times a second (the metered value lives on the GPU and is read back).
pub(crate) struct ExposureLog {
    pub(crate) buf: wgpu::Buffer,
    pub(crate) ready: Arc<std::sync::atomic::AtomicBool>,
    pub(crate) waiting: bool,
    pub(crate) frame: u64,
    pub(crate) started: std::time::Instant,
    /// the pre-exposure (log2) and the metering settings of the frame being read back
    pub(crate) pending: (f32, [f32; 6]),
    /// The tone mapping's metering correction last read back (EV). Self-lit surfaces (a
    /// display's text, a script texture) are drawn this much brighter or darker in advance,
    /// so that they come out at their own brightness whatever the metering does to the
    /// rest of the picture: a destination display was darkened with a sunlit street and
    /// hardly readable by day.
    pub(crate) ev: f32,
    /// `OMSI_DEBUG_EXPOSURE`: log what is read.
    pub(crate) log: bool,
}

impl ExposureLog {
    pub(crate) fn new(device: &wgpu::Device) -> Option<ExposureLog> {
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("exposure readback"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Some(ExposureLog {
            buf,
            ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            waiting: false,
            frame: 0,
            started: std::time::Instant::now(),
            pending: (0.0, [0.0; 6]),
            ev: 0.0,
            log: omsi_cfg::env::var_os("OMSI_DEBUG_EXPOSURE").is_some(),
        })
    }

    pub(crate) fn sample(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        adapted: &wgpu::TextureView,
        pre_log2: f32,
        meter: [f32; 6],
    ) {
        self.frame += 1;
        if self.waiting && self.ready.swap(false, std::sync::atomic::Ordering::Relaxed) {
            {
                let view = self.buf.slice(0..8).get_mapped_range();
                let bits = u16::from_le_bytes([view[0], view[1]]);
                let metered = half_to_f32(bits);
                let (pre, m) = self.pending;
                let ev = ((m[1] - metered) * m[0]).clamp(-m[2], m[3]) + m[4];
                if ev.is_finite() {
                    self.ev = ev;
                }
                if self.log {
                    log::info!("exposure t={:.2}s: light model {:+.2} EV, metered picture log2 {:+.2}, correction {:+.2} EV, total {:+.2} EV", self.started.elapsed().as_secs_f32(), pre, metered, ev, pre + ev);
                }
            }
            self.buf.unmap();
            self.waiting = false;
        }
        if self.waiting || self.frame % 8 != 0 {
            return;
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: adapted.texture(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        self.pending = (pre_log2, meter);
        let ready = self.ready.clone();
        // mapped once the frame's commands are submitted (see `render_inner`)
        self.waiting = true;
        encoder.map_buffer_on_submit(&self.buf, wgpu::MapMode::Read, .., move |r| {
            ready.store(r.is_ok(), std::sync::atomic::Ordering::Relaxed)
        });
    }
}

pub(crate) fn half_to_f32(b: u16) -> f32 {
    let sign = if b & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = ((b >> 10) & 0x1f) as i32;
    let m = (b & 0x3ff) as f32;
    match e {
        0 => sign * m / 1024.0 * 2f32.powi(-14),
        31 => sign * f32::INFINITY,
        _ => sign * (1.0 + m / 1024.0) * 2f32.powi(e - 15),
    }
}
