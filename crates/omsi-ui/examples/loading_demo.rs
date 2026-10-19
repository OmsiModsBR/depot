//! The loading screen drawn offscreen: with no screenshots (the default background), and
//! with three made-up ones in the middle of a fade: `cargo run -p omsi-ui --example
//! loading_demo -- out_dir`
#[path = "common/mod.rs"]
mod common;
use omsi_ui::loading::{self, Card, Slide, Slideshow, View, FADE_SECONDS, HOLD_SECONDS};

/// A made-up screenshot: a vertical blend of two colours, `w` x `h`.
fn picture(w: u32, h: u32, top: [u8; 3], bottom: [u8; 3]) -> Vec<u8> {
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        let t = y as f32 / h as f32;
        for _ in 0..w {
            for k in 0..3 {
                px.push((top[k] as f32 * (1.0 - t) + bottom[k] as f32 * t) as u8);
            }
            px.push(255);
        }
    }
    px
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or(".".into());
    std::fs::create_dir_all(&dir).unwrap();
    let mut o = common::Offscreen::new((1672, 941));
    let colours = [
        ([20, 60, 120], [200, 140, 60]),
        ([30, 100, 60], [220, 220, 120]),
        ([90, 30, 90], [240, 120, 120]),
    ];
    let slides: Vec<Slide> = colours
        .iter()
        .map(|(a, b)| Slide {
            tex: o
                .gpu
                .add_image(&o.device, &o.queue, 300, 200, &picture(300, 200, *a, *b)),
            aspect: 1.5,
        })
        .collect();
    for (name, shown, t) in [
        ("default", 0usize, 0.0f32),
        ("slideshow-mid-fade", 3, HOLD_SECONDS + FADE_SECONDS * 0.5),
    ] {
        let mut show = Slideshow::new(shown);
        show.update(t);
        let v = View {
            version: "example version",
            heading: "Example map \u{b7} Line 50 \u{b7} 08:00",
            stage: "Loading map tiles (first area) \u{b7} 62 of 100",
            progress: 0.62,
            logo: None,
            bus: Some(Card {
                title: "Example bus A",
                sub: "Example Maker \u{b7} v1.2 \u{b7} Default paint",
                description:
                    "Author's description, first lines as written by the author (example text).",
            }),
            map: Some(Card {
                title: "Example map",
                sub: "Example City",
                description: "Map description from the map folder (example text, a few lines).",
            }),
            slides: &slides[..shown],
        };
        o.shot(&format!("{dir}/loading-{name}.png"), |ui| {
            loading::draw(ui, &v, &show)
        });
    }
}
