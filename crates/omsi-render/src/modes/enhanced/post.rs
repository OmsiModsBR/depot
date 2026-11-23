//! Enhanced's post passes: glow, exposure metering, adaptation, tone curve and FXAA, over the
//! high-range picture the mode drew.

use crate::gpu::pass_timer;
use crate::interface::PostParts;
use crate::lighting::{meter_tuning, tone_contrast};
use crate::passes::{FrameFeatures, PostPass, PostUniform};
use crate::sky::atmosphere;

/// Enhanced's post passes: glow, metering, adaptation, tone curve and FXAA.
pub(crate) fn encode_post(parts: PostParts<'_>) {
    let PostParts {
        queue,
        post,
        targets,
        adaptation,
        sky,
        msaa,
        fxaa: fxaa_opt,
        instant_exposure,
        exposure_log,
        overlay_pipeline_1x,
        scene,
        encoder,
        lighting,
        scene_view,
        overlays,
        width,
        height,
        dt,
        features,
        puddles_on,
        tset,
        timed,
    } = parts;
    let FrameFeatures {
        enhanced,
        lead_view,
        scaled,
        with_overlays,
        ..
    } = *features;
    if enhanced {
        // --- the post passes: glow, metering and adaptation, tone curve, FXAA
        let secs = |tau: f32| {
            if dt > 0.0 {
                1.0 - (-dt / tau).exp()
            } else {
                1.0
            }
        };
        // (the brightening is for what the light model cannot know by day - a dark cab,
        // an underpass; by night the picture is dark because the night is, and the eye's
        // adaptation to it is the light model's already: lifted by the meter on top, a
        // lamp-lit street came out most of a stop brighter than any eye sees it)
        let mut m = meter_tuning();
        let after_sunset = sky.sky_state.as_ref().map_or(0.0, |st| {
            ((0.052 - st.input.sun_dir.normalize_or_zero().z) / 0.157).clamp(0.0, 1.0)
        });
        m[3] *= 1.0
            - atmosphere::smoothstep(
                3.0,
                7.0,
                adaptation.exposure.unwrap_or(0.0) / std::f32::consts::LN_2,
            );
        let pu = PostUniform {
            // the metering may take a little off a bright picture and add a little to a
            // dark one: a night stays a night, snow stays white
            a: [
                // (the share of the light beyond the screen's white the eye scatters
                // further than a third of a degree, CIE 146 - see post.wgsl `fs_up`)
                0.2,
                m[2],
                m[3],
                if instant_exposure || dt <= 0.0 {
                    1.0
                } else {
                    0.0
                },
            ],
            // darker: the eye takes a few seconds; brighter: under one
            b: [
                secs(2.5),
                secs(0.6),
                m[1],
                m[4] + lighting.night_brightness * after_sunset,
            ],
            // (w: an LED panel's dots count for this much in the glow's source. The mix
            // the glow lands with is a few per cent - a lamp a hundred times brighter
            // than white spreads, a white wall does not - so the dots are multiplied up
            // there instead of being drawn burning: their halo shows, they don't bleach.
            // `Led glow`, 0 = not at all.)
            c: [
                m[0],
                m[5],
                adaptation.exposure.map(f32::exp).unwrap_or(1.0),
                lighting.led_glow * 10.0,
            ],
            // (w: the tone curve's contrast, post.wgsl `natural_tone`)
            d: [
                0.0,
                0.0,
                0.0,
                tone_contrast(adaptation.exposure.unwrap_or(0.0)),
            ],
        };
        queue.write_buffer(&post.post_buf, 0, bytemuck::bytes_of(&pu));
        // (a mirror's small picture goes without FXAA)
        let fxaa = with_overlays && fxaa_opt && msaa <= 1;
        if let Some(h) = targets.hdr_targets.get(&(width, height)) {
            let puddles = h.puddles.as_ref().filter(|_| puddles_on);
            let levels = h.down.len();
            for i in 0..levels {
                PostPass::new("post", &h.down[i]).draw(
                    encoder,
                    if i == 0 { &post.down_first } else { &post.down },
                    &[if i == 0 {
                        puddles.map(|p| &p.down_bg).unwrap_or(&h.down_bg[i])
                    } else {
                        &h.down_bg[i]
                    }],
                );
            }
            // the exposure: meter the smallest level, move the adapted value towards it
            // (the window's picture only: a mirror is graded with the window's exposure,
            // as the eye that looks into it is adapted to the street)
            if lead_view {
                PostPass::new("post", &adaptation.meter_view).draw(
                    encoder,
                    &post.meter,
                    &[&h.meter_bg],
                );
                let front = adaptation.adapt_front;
                PostPass::new("post", &adaptation.adapt_views[1 - front]).draw(
                    encoder,
                    &post.adapt,
                    &[&adaptation.adapt_bg[front]],
                );
                adaptation.adapt_front = 1 - front;
            }
            if let Some(log) = exposure_log {
                let pre = adaptation.exposure.unwrap_or(0.0) / std::f32::consts::LN_2;
                log.sample(
                    encoder,
                    &adaptation.adapt_views[adaptation.adapt_front],
                    pre,
                    m,
                );
            }
            // (timed on its last pass: the glow chain with the metering, see GpuTimers)
            for i in (0..levels).rev() {
                let timer = if i == 0 {
                    pass_timer(tset.as_ref(), timed, "glow+meter")
                } else {
                    None
                };
                PostPass::new("post", &h.up[i]).timer(timer).draw(
                    encoder,
                    &post.up,
                    &[&h.up_bg[i]],
                );
            }
            let final_view = if fxaa { &h.ldr } else { scene_view };
            let tonemap_bg =
                &puddles.map(|p| &p.tonemap_bg).unwrap_or(&h.tonemap_bg)[adaptation.adapt_front];
            let mut pass = PostPass::new("tone map", final_view)
                .timer(pass_timer(tset.as_ref(), timed, "tone map"))
                .begin(encoder);
            pass.set_pipeline(if fxaa {
                &post.tonemap_encoded
            } else {
                &post.tonemap
            });
            pass.set_bind_group(0, tonemap_bg, &[]);
            pass.draw(0..3, 0..1);
            if fxaa {
                drop(pass);
                pass = PostPass::new("fxaa", scene_view)
                    .timer(pass_timer(tset.as_ref(), timed, "fxaa"))
                    .begin(encoder);
                pass.set_pipeline(&post.fxaa);
                pass.set_bind_group(0, &h.fxaa_bg, &[]);
                pass.draw(0..3, 0..1);
            }
            if !overlays.is_empty() && !scaled {
                pass.set_pipeline(overlay_pipeline_1x);
                for (k, _) in overlays.iter().enumerate() {
                    if let Some((_, _, bg, _)) = scene.overlay_res.get(k) {
                        pass.set_bind_group(0, bg, &[]);
                        pass.draw(0..6, 0..1);
                    }
                }
            }
        }
    }
}
