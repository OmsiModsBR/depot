//! `depot-launcher-next`: the new launcher (a draft; the launcher in the game binary is still
//! the one that ships).
//!
//! * no arguments: the launcher window;
//! * `--page NAME`: start on that page (`vehicles`, `maps`, ...);
//! * `--shot FILE.png [--size WxH]`: draw the page into a picture and exit, with no window.

use omsi_launcher_ui::{offscreen, window, Page, Shell};

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
    let result = match value("--shot") {
        Some(file) => {
            let size = value("--size")
                .and_then(|s| {
                    let (w, h) = s.split_once('x')?;
                    Some((w.parse().ok()?, h.parse().ok()?))
                })
                .unwrap_or((1672, 941));
            offscreen::shot(&mut shell, size, std::path::Path::new(&file))
        }
        None => window::run(shell),
    };
    if let Err(e) = result {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
