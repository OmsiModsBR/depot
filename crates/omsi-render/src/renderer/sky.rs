//! The sky on the renderer: installing a new sky and the enhanced path's per-frame sky uniform.

use crate::lighting::{debug_view, tone_contrast, Lighting, CABIN_E, LAMP_E, WINDOW_RADIANCE};
use crate::renderer::Renderer;
use crate::sky::{
    atmosphere, cloud_sun_transmittance, enhanced_sky_input, enhanced_weather_fog,
    sky_input_differs, EnhancedUniform, CLOUD_SHADE_ADAPT, FOG_FALLOFF, MOON_RADIUS, PROBE_MIPS,
    SUN_RADIUS,
};
use glam::{DVec3, Vec3};

impl Renderer {
    /// Take in a newly computed sky: its table goes to the GPU, the probe is redrawn.
    pub(crate) fn install_sky(&mut self, st: atmosphere::SkyState) {
        let mut bytes: Vec<u8> = Vec::with_capacity(st.lut.len() * 8);
        for texel in &st.lut {
            for v in texel {
                bytes.extend_from_slice(&atmosphere::f16_bits(*v).to_le_bytes());
            }
        }
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.sky.sky_lut,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(atmosphere::SKY_LUT_W * 8),
                rows_per_image: Some(atmosphere::SKY_LUT_H),
            },
            wgpu::Extent3d {
                width: atmosphere::SKY_LUT_W,
                height: atmosphere::SKY_LUT_H,
                depth_or_array_layers: 1,
            },
        );
        if omsi_cfg::flags::DEBUG_SKY.is_set() {
            log::info!("sky: sun {:?} (altitude {:.1}°) sky {:?} ground {:?} exposure {:.3} table scale {:.4} haze {:.2} overcast {:.2} rain {:.2} sun visibility {:.2} moon {:?} (altitude {:.1}°, lit {:.2}) city glow {:.2}", st.sun, st.input.sun_dir.z.asin().to_degrees(), st.sky_horizontal, st.ground, st.exposure, st.lut_scale, st.input.haze, st.input.overcast, st.input.rain, st.input.sun_visibility, st.moon_light, st.input.moon_dir.normalize_or_zero().z.asin().to_degrees(), st.input.moon_illum, st.input.city_glow);
        }
        if let Some(p) = self.sky.probe.as_mut() {
            p.age = u32::MAX;
        }
        self.sky.sky_state = Some(st);
    }

    /// The enhanced path's light for this frame: the sky (recomputed when the sun or the
    /// weather has moved on), the exposure following it, the sky table and the uniform.
    /// Returns whether the reflection probe is to be drawn this frame.
    pub(crate) fn prepare_enhanced(
        &mut self,
        lighting: &Lighting,
        cam_rel: Vec3,
        ro: DVec3,
        dt: f32,
    ) -> bool {
        let (input, sun_visibility) =
            enhanced_sky_input(lighting, self.sky.city_glow.unwrap_or(1.0));
        // A new sky takes a few milliseconds: it is computed on a helper thread and taken in
        // when it is ready. A picture on its own, and the first frame, wait for it.
        if let Some((_, rx)) = &self.sky.sky_job {
            match rx.try_recv() {
                Ok(st) => {
                    self.sky.sky_job = None;
                    self.install_sky(st);
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.sky.sky_job = None,
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        let pending = self.sky.sky_job.as_ref().map(|j| j.0).or(self
            .sky
            .sky_state
            .as_ref()
            .map(|st| st.input));
        if pending
            .map(|i| sky_input_differs(&i, &input))
            .unwrap_or(true)
        {
            if self.instant_exposure || self.sky.sky_state.is_none() {
                self.sky.sky_job = None;
                self.install_sky(atmosphere::SkyState::compute(&input));
            } else {
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let _ = tx.send(atmosphere::SkyState::compute(&input));
                });
                self.sky.sky_job = Some((input, rx));
            }
        }
        let input_overcast = input.overcast;
        // the cumulus over the street: how much of the sun gets through them along the sun's
        // way from here, a little blurred (a cloud's shadow has a soft edge tens of metres
        // wide) and followed over a moment as the clouds drift
        let cloud_t = {
            let now = if lighting.enhanced && lighting.cloud_density > 0.0 {
                cloud_sun_transmittance(&self.sky.cloud_shape_cpu, lighting, cam_rel, ro)
            } else {
                1.0
            };
            let k = if self.instant_exposure || dt <= 0.0 {
                1.0
            } else {
                1.0 - (-dt / 0.7).exp()
            };
            if omsi_cfg::flags::DEBUG_SKY.is_set() {
                // (what share of the sky's directions are clear, against the cover asked for)
                let mut clear = 0;
                let mut n = 0;
                for i in 0..64 {
                    let az = i as f32 * 0.7;
                    let el = 0.25 + (i % 8) as f32 * 0.09;
                    let mut l = lighting.clone();
                    l.sun_dir = Vec3::new(el.cos() * az.cos(), el.cos() * az.sin(), el.sin());
                    if cloud_sun_transmittance(&self.sky.cloud_shape_cpu, &l, cam_rel, ro) > 0.5 {
                        clear += 1;
                    }
                    n += 1;
                }
                log::info!(
                    "cloud sun: {now:.3} here, {clear} of {n} directions clear (cover {:.2})",
                    lighting.cloud_density
                );
            }
            let t = self.sky.cloud_sun.map(|c| c + (now - c) * k).unwrap_or(now);
            self.sky.cloud_sun = Some(t);
            t
        };
        let st = self.sky.sky_state.as_ref().expect("sky state");
        // the exposure follows the light over a second or two (in log space); an offscreen
        // picture and the first frame take it at once
        // (the eye takes to a cloud's shadow as to any other light: the exposure is for the
        // sun the street actually gets)
        // (half of the way in log terms: a camera, and the eye, take a cloud's shadow as
        // darker - it is darker - only not as much as the light meter says)
        // (by night the eye adapts to the lamps' and headlights' light it sees, measured,
        // instead of a lit city's average: on a dark country road under a full moon it
        // takes to the moonlight, and the moon's shadows show)
        // ... but never below the lit streets of the district the camera is in: the eye's
        // adaptation to the dark takes minutes (the rods' some twenty), and among lamp-lit
        // streets it stays with them while the view passes over a dark yard or a field -
        // taken to the frame's darkness at once, every lamp's light in a lit village went
        // white. The district's level is a lit street's (`ARTIFICIAL`) by how lit the
        // place round the camera is (`city_glow`, its lamps within a few hundred metres);
        // a country road with no lamps round it has none, and the moonlight still decides.
        let district = atmosphere::ARTIFICIAL * st.input.city_glow.clamp(0.0, 1.0);
        let e_rest = match self.adaptation.view_lamps {
            Some(v) => (st.e_rest - st.e_artificial + v.max(district)).max(1e-6),
            None => st.e_rest,
        };
        let full = atmosphere::exposure_for(st.e_sun + e_rest).max(1e-6).ln();
        let shaded = atmosphere::exposure_for(st.e_sun * cloud_t + e_rest)
            .max(1e-6)
            .ln();
        let target = full + (shaded - full) * CLOUD_SHADE_ADAPT;
        let log_exposure = match self.adaptation.exposure {
            // (the eye takes to brighter light within a second, to the dark over several:
            // the cones' light adaptation is fast, their dark adaptation slow)
            Some(e) if !self.instant_exposure && dt > 0.0 => {
                let tau = if target < e { 0.6 } else { 3.0 };
                e + (target - e) * (1.0 - (-dt / tau).exp())
            }
            _ => target,
        };
        self.adaptation.exposure = Some(log_exposure);
        let pre = log_exposure.exp();
        // the fog's in-scattered light: a white sphere's average in this light
        let axes = [Vec3::X, -Vec3::X, Vec3::Y, -Vec3::Y, Vec3::Z, -Vec3::Z];
        let avg = axes
            .iter()
            .map(|a| atmosphere::sh_irradiance(&st.sh, *a))
            .fold(Vec3::ZERO, |a, b| a + b)
            / 6.0;
        let fog_rgb = avg * 0.9 / std::f32::consts::PI;
        let weather_fog = enhanced_weather_fog(lighting);
        // (the air near the ground: the Rayleigh part and more than the sky model's
        // average aerosol - a city's air near the ground holds its dust and exhaust, which is what gives a street its depth - the houses a few
        // hundred metres off a shade paler and bluer than the ones beside the road)
        let clear_air = 1.8e-5 + 3.6e-5 * input.haze;
        // the fog lies on the ground under the player's vehicle, or just under the camera
        let base = match lighting.fog_base.or(lighting.inside.map(|v| v.0.z)) {
            Some(z) => (z - ro.z) as f32,
            None => cam_rel.z - 2.0,
        };
        let mut sh = [[0.0f32; 4]; 9];
        for (k, c) in st.sh.iter().enumerate() {
            sh[k] = c.extend(0.0).to_array();
        }
        // the probe is drawn every half second or so, and at once for a new sky
        let instant = self.instant_exposure;
        let (redraw, probe_scale) = match self.sky.probe.as_mut() {
            Some(p) => {
                let redraw = instant || p.age >= 30;
                if redraw {
                    p.age = 0;
                    p.scale = st.lut_scale;
                } else {
                    p.age += 1;
                }
                (redraw, p.scale)
            }
            None => (false, 1.0),
        };
        // the sky cube's eye (see Probe::cube_eye): the correction for a camera that moved
        // away from it is exact for the base of the clouds, and wrong by up to the layer's
        // depth over their distance; a climb shows far sooner than driving along
        let cam_w = ro + cam_rel.as_dvec3();
        let eye_off = match self.sky.probe.as_mut() {
            Some(p) => {
                let to_clouds = (1400.0 - cam_rel.z as f64).max(120.0);
                let far = match p.cube_eye {
                    Some(e) if p.cube_filled => {
                        let m = cam_w - e;
                        // (a third of what it was: flying the free camera fast, the clouds
                        // drifted with the old cube for 400 m and then jumped back into place)
                        (m.truncate().length() * 0.1 + m.z.abs()) / to_clouds > 0.01
                    }
                    _ => true,
                };
                if far {
                    p.cube_eye = Some(cam_w);
                    p.cube_recapture = p.cube_filled;
                }
                (p.cube_eye.unwrap_or(cam_w) - cam_w).as_vec3()
            }
            None => Vec3::ZERO,
        };
        let u = EnhancedUniform {
            // (self-lit surfaces at their own brightness after the metering, see ExposureLog)
            exposure: [
                pre,
                2f32.powf(
                    -self
                        .diagnostics
                        .exposure_log
                        .as_ref()
                        .map(|l| l.ev)
                        .unwrap_or(0.0),
                )
                .clamp(0.7, 1.6),
                pre * WINDOW_RADIANCE,
                1.6,
            ],
            sun: (st.sun * cloud_t).extend(SUN_RADIUS).to_array(),
            sh,
            ground: st.ground.extend(st.lut_scale).to_array(),
            fog: [weather_fog, FOG_FALLOFF, base, clear_air],
            fog_color: fog_rgb.extend(probe_scale).to_array(),
            weather: [
                lighting.wetness.clamp(0.0, 1.0),
                lighting.snow.clamp(0.0, 1.0),
                lighting.rain.clamp(0.0, 1.0),
                input_overcast,
            ],
            lights: [PROBE_MIPS as f32, LAMP_E, CABIN_E, sun_visibility],
            sun_disc: st.sun_disc.extend(dt).to_array(),
            debug: [
                debug_view(),
                omsi_cfg::flags::PUDDLE_F0
                    .number::<f32>()
                    .filter(|v| v.is_finite())
                    .unwrap_or(0.08)
                    .clamp(0.02, 0.2),
                omsi_cfg::flags::ENV_PHOTO.number().unwrap_or(1.0),
                // the tone curve's contrast, which self-lit pictures undo (`display_level`)
                tone_contrast(log_exposure),
            ],
            eye: eye_off.extend(0.0).to_array(),
            // x how bright an LED panel's dots burn (see `MaterialExtra::led`; the settings'
            // 16 levels give 0 = off .. 3.75), y whether the LED panels' `\S:n` masks keep
            // their mip chain (0: at full resolution, the dots stay visible when small)
            led: [lighting.led_glow, lighting.led_mips, 0.0, 0.0],
            moon: lighting
                .moon_dir
                .normalize_or_zero()
                .extend(MOON_RADIUS)
                .to_array(),
            // (w of the first: the veil's optical depth, which the sky draws as the high
            // layer; of the second: how far the veil spreads the sun, which softens shadows)
            cloud_sun: {
                let mut c = st.sun_at.map(|c| c.extend(0.0).to_array());
                c[0][3] = st.input.veil;
                c[1][3] = 1.0 - (-st.input.veil / st.input.sun_dir.z.max(0.03)).exp();
                c
            },
            condensation: lighting.condensation,
            moon_light: st
                .moon_light
                .extend(if lighting.casts_moon_shadows() {
                    1.0
                } else {
                    0.0
                })
                .to_array(),
            moon_disc: st
                .moon_disc
                .extend((1.0 - 0.18 * (st.input.haze - 1.0).max(0.0)).clamp(0.2, 1.0) * 0.55)
                .to_array(),
        };
        self.queue
            .write_buffer(&self.enh_buf, 0, bytemuck::bytes_of(&u));
        redraw
    }
}
