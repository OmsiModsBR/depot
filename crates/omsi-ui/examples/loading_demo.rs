//! The loading screen drawn offscreen at the sizes it is checked at (1920x1080, 1672x941 and
//! 2560x1080, each at the game's scale and half as large again): with no screenshots (the
//! default background), and with made-up ones in the middle of a fade, one of them bright:
//! `cargo run -p omsi-ui --example loading_demo -- out_dir`
#[path = "common/mod.rs"]
mod common;
use omsi_ui::loading::{self, Card, Slide, Slideshow, View, FADE_SECONDS, HOLD_SECONDS};

const LOGO: &[u8] = include_bytes!("../../../assets/logos/wordmark-on-dark.png");

/// A made-up screenshot: a sky over a road, `w` x `h`, the sky blending `top` into `horizon`.
fn picture(w: u32, h: u32, top: [u8; 3], horizon: [u8; 3], ground: [u8; 3]) -> Vec<u8> {
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    let line = h * 55 / 100;
    for y in 0..h {
        for x in 0..w {
            let c = if y < line {
                let t = y as f32 / line as f32;
                [0, 1, 2].map(|k| (top[k] as f32 * (1.0 - t) + horizon[k] as f32 * t) as u8)
            } else {
                // a road narrowing to the horizon
                let t = (y - line) as f32 / (h - line) as f32;
                let half = w as f32 * (0.06 + 0.4 * t);
                let on_road = (x as f32 - w as f32 * 0.5).abs() < half;
                let g = if on_road { [70, 72, 76] } else { ground };
                [0, 1, 2].map(|k| (g[k] as f32 * (0.8 + 0.3 * t)).min(255.0) as u8)
            };
            px.extend_from_slice(&c);
            px.push(255);
        }
    }
    px
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or(".".into());
    std::fs::create_dir_all(&dir).unwrap();
    let skies = [
        // a bright midday sky: the text at the foot must still read over it
        ([150, 190, 235], [235, 240, 245], [150, 160, 120]),
        ([30, 50, 90], [220, 140, 80], [60, 70, 50]),
        ([70, 110, 160], [190, 210, 225], [90, 110, 70]),
    ];
    for (w, h) in [(1920u32, 1080u32), (1672, 941), (2560, 1080)] {
        let mut o = common::Offscreen::new((w, h));
        // (as the game loads it: shrunk, and its black field made see-through)
        let logo = image::load_from_memory(LOGO).unwrap();
        let lh = logo.height() * 640 / logo.width();
        let mut logo = logo
            .resize_exact(640, lh, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        loading::unmatte_black(&mut logo);
        let logo = (
            o.gpu.add_image(
                &o.device,
                &o.queue,
                logo.width(),
                logo.height(),
                logo.as_raw(),
            ),
            logo.height() as f32 / logo.width() as f32,
        );
        let slides: Vec<Slide> = skies
            .iter()
            .map(|(a, b, c)| Slide {
                tex: o.gpu.add_image(
                    &o.device,
                    &o.queue,
                    480,
                    270,
                    &picture(480, 270, *a, *b, *c),
                ),
                aspect: 480.0 / 270.0,
            })
            .collect();
        // the game's scale: the 1672x941 design fitted to the window
        let natural = (w as f32 / 1672.0).min(h as f32 / 941.0).max(0.75);
        for (name, shown, t) in [
            ("default", 0usize, 0.0f32),
            ("bright", 1, 0.0),
            ("mid-fade", 3, HOLD_SECONDS + FADE_SECONDS * 0.5),
        ] {
            for (tag, scale) in [("", natural), ("-150", natural * 1.5)] {
                let mut show = Slideshow::new(shown);
                show.update(t);
                let v = View {
                    version: "example version",
                    heading: "Example map \u{b7} Line 50 \u{b7} 08:00",
                    stage: "Loading map tiles (first area) \u{b7} 62 of 100",
                    activity: "Preparing the world",
                    progress: 0.62,
                    logo: Some(logo),
                    bus: Some(Card {
                        title: "Example bus A",
                        sub: "Example Maker",
                        detail: "Livery: Default paint",
                        icon: "directions_bus",
                        thumb: None,
                    }),
                    map: Some(Card {
                        title: "Example map",
                        sub: "Example City",
                        detail: "",
                        icon: "map",
                        thumb: None,
                    }),
                    slides: &slides[..shown],
                };
                o.shot_at(
                    &format!("{dir}/loading-{w}x{h}-{name}{tag}.png"),
                    scale,
                    |ui| loading::draw(ui, &v, &show),
                );
            }
        }
    }
}
