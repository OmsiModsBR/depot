//! The Depot | OMSI 2 switch in the sidebar and the top bar: it is drawn only when asked for,
//! a click on the other half asks for that engine, and it does nothing while a game runs.

use glam::Vec2;
use omsi_launcher_ui::shell::{EngineSwitch, Nav, Sidebar, TopBar};
use omsi_launcher_ui::Page;
use omsi_ui::widgets::{id_of, Ui};

fn click(ui: &mut Ui, at: Vec2, draw: &mut dyn FnMut(&mut Ui) -> Option<Nav>) -> Option<Nav> {
    ui.input.mouse = at;
    ui.input.pressed = true;
    ui.input.down = true;
    ui.begin(Vec2::new(1700.0, 900.0), 1.0, 0.016);
    draw(ui);
    ui.finish();
    ui.input.pressed = false;
    ui.input.released = true;
    ui.input.down = false;
    ui.begin(Vec2::new(1700.0, 900.0), 1.0, 0.016);
    let n = draw(ui);
    ui.finish();
    ui.input.released = false;
    n
}

fn sidebar(ui: &mut Ui, engine: Option<EngineSwitch>) -> Option<Nav> {
    Sidebar {
        page: Page::Maps,
        logo: None,
        badges: &[],
        footer: ("Driver", "Level 1"),
        engine,
    }
    .draw(ui, 900.0)
}

fn frame(ui: &mut Ui, f: &mut dyn FnMut(&mut Ui)) {
    ui.begin(Vec2::new(1700.0, 900.0), 1.0, 0.016);
    f(ui);
    ui.finish();
}

#[test]
fn the_switch_is_drawn_only_when_asked_for_and_a_click_picks_the_other_engine() {
    let mut ui = Ui::new();
    frame(&mut ui, &mut |u| {
        sidebar(u, None);
    });
    assert!(!ui.drawn.contains_key(&id_of("engine:omsi2")));
    let sw = Some(EngineSwitch {
        omsi2: false,
        locked: false,
    });
    frame(&mut ui, &mut |u| {
        sidebar(u, sw);
    });
    let r = ui.drawn[&id_of("engine:omsi2")];
    assert_eq!(
        click(&mut ui, r.center(), &mut |u| sidebar(u, sw)),
        Some(Nav::Engine(true))
    );
    // the half already in use does nothing
    let depot = ui.drawn[&id_of("engine:depot")];
    assert_eq!(
        click(&mut ui, depot.center(), &mut |u| sidebar(u, sw)),
        None
    );
    // locked while a game runs
    let locked = Some(EngineSwitch {
        omsi2: false,
        locked: true,
    });
    assert_eq!(
        click(&mut ui, r.center(), &mut |u| sidebar(u, locked)),
        None
    );
}

#[test]
fn the_top_bar_has_the_halves_when_wide_and_one_swap_button_when_narrow() {
    let sw = Some(EngineSwitch {
        omsi2: true,
        locked: false,
    });
    let bar = TopBar {
        page: Page::Maps,
        logo: None,
        badges: &[],
        engine: sw,
    };
    let mut ui = Ui::new();
    ui.begin(Vec2::new(1800.0, 900.0), 1.0, 0.016);
    bar.draw(&mut ui, 1800.0);
    ui.finish();
    assert!(ui.drawn.contains_key(&id_of("engine:depot")));
    assert!(!ui.drawn.contains_key(&id_of("engine:swap")));
    let mut ui = Ui::new();
    ui.begin(Vec2::new(1100.0, 900.0), 1.0, 0.016);
    bar.draw(&mut ui, 1100.0);
    ui.finish();
    assert!(ui.drawn.contains_key(&id_of("engine:swap")));
    // a click on the swap button asks for the other engine
    let r = ui.drawn[&id_of("engine:swap")];
    let n = click(&mut ui, r.center(), &mut |u| bar.draw(u, 1100.0));
    assert_eq!(n, Some(Nav::Engine(false)));
}
