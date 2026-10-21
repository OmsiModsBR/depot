//! A picture of the launcher's pages, with no window (the launcher itself is the game's, see
//! `omsi-app::launcher`):
//!
//! `cargo run -p omsi-launcher-ui --example launcher_shot -- --shot FILE.png [--page NAME]
//! [--size WxH] [--sample]`
//!
//! * `--page NAME`: that page (`vehicles`, `maps`, ...);
//! * `--sample`: made-up vehicles and maps instead of the installed ones.

use omsi_launcher_ui::{offscreen, Page, Shell};

fn main() {
    env_logger::init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let page = match value("--page") {
        Some(name) => Page::named(&name).unwrap_or_else(|| {
            eprintln!("unknown page: {name}");
            std::process::exit(2);
        }),
        None => Page::Drive,
    };
    let mut shell = Shell::new(page);
    if args.iter().any(|a| a == "--sample") {
        shell.vehicles = omsi_launcher_ui::vehicles::VehiclesView::with_catalog(
            omsi_launcher_ui::sample::vehicles(),
        );
        shell.maps =
            omsi_launcher_ui::maps::MapsView::with_catalog(omsi_launcher_ui::sample::maps());
    }
    let Some(file) = value("--shot") else {
        eprintln!("usage: launcher_shot --shot FILE.png [--page NAME] [--size WxH] [--sample]");
        std::process::exit(2);
    };
    let size = match value("--size") {
        Some(s) => parse_size(&s).unwrap_or_else(|| {
            eprintln!("invalid --size {s:?}: expected WIDTHxHEIGHT, like 1672x941");
            std::process::exit(2);
        }),
        None => (1672, 941),
    };
    if let Err(e) = offscreen::shot(&mut shell, size, std::path::Path::new(&file)) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

/// `WIDTHxHEIGHT` in pixels; both at least 1 and at most 16384 (the picture is made in memory).
fn parse_size(s: &str) -> Option<(u32, u32)> {
    let (w, h) = s.split_once('x')?;
    let (w, h): (u32, u32) = (w.parse().ok()?, h.parse().ok()?);
    ((1..=16384).contains(&w) && (1..=16384).contains(&h)).then_some((w, h))
}

#[cfg(test)]
mod tests {
    use super::parse_size;

    #[test]
    fn size_is_width_x_height_or_nothing() {
        assert_eq!(parse_size("1672x941"), Some((1672, 941)));
        for bad in [
            "", "1672", "x", "0x10", "10x0", "axb", "1672x", "99999x10", "-1x5",
        ] {
            assert_eq!(parse_size(bad), None, "{bad}");
        }
    }
}
