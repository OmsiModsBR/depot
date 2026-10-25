//! The launcher's pages, in the order of the sidebar.

/// A page of the launcher. The first eight sit in the sidebar's list, `Settings` and `Setup`
/// at its foot. `Drive` is no longer listed: a session starts from Maps (the bus and the
/// line are chosen inside the game, as in OMSI 2); the phone's launcher still has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Drive,
    Vehicles,
    Maps,
    Mods,
    Timetable,
    Profile,
    Controls,
    Tutorials,
    Diagnostics,
    Settings,
    Setup,
}

impl Page {
    /// Every page, the list first, then the foot.
    pub const ALL: [Page; 11] = [
        Page::Maps,
        Page::Vehicles,
        Page::Mods,
        Page::Timetable,
        Page::Profile,
        Page::Controls,
        Page::Tutorials,
        Page::Diagnostics,
        Page::Settings,
        Page::Setup,
        Page::Drive,
    ];

    /// The pages of the sidebar's list.
    pub fn listed() -> &'static [Page] {
        &Page::ALL[..8]
    }

    /// The pages at the sidebar's foot.
    pub fn foot() -> &'static [Page] {
        &Page::ALL[8..10]
    }

    pub fn title(self) -> &'static str {
        match self {
            Page::Drive => "Drive",
            Page::Vehicles => "Vehicles",
            Page::Maps => "Maps",
            Page::Mods => "Mods",
            Page::Timetable => "Timetable",
            Page::Profile => "Profile",
            Page::Controls => "Controls",
            Page::Tutorials => "Tutorials",
            Page::Diagnostics => "Diagnostics",
            Page::Settings => "Settings",
            Page::Setup => "Setup",
        }
    }

    /// The heading the concept gives the page: where it is (after the page's name), what it
    /// asks of the player, and a line on it (every page but Drive, which has its steps).
    pub fn heading(self) -> Option<(&'static str, &'static str, &'static str)> {
        match self {
            Page::Drive => None,
            Page::Vehicles => Some((
                "Library",
                "Choose and check your bus",
                "Look at a bus with its paint and the author's description. Pick one to have it ready when you start from Maps.",
            )),
            Page::Maps => Some((
                "Start",
                "Choose your map and start",
                "Pick the map, the date and the kind of session, then where to start. The line and the bus are chosen inside the game.",
            )),
            Page::Mods => Some((
                "Install & content",
                "Add buses, maps and scenery",
                "Add a .zip, used in place from Archives. The original OMSI 2 folder is never written to.",
            )),
            Page::Timetable => Some((
                "Plan",
                "Plan a map's lines and tours",
                "See the lines on the map, see which lines serve a stop, draw them as a diagram, and edit when each trip leaves. Saved in the content folder; OMSI 2's own files stay as they are.",
            )),
            Page::Profile => Some((
                "Personnel file",
                "Your driver and their record",
                "The driver whose personnel file the game keeps: level, hours, kilometres, punctuality and the drives already finished.",
            )),
            Page::Controls => Some((
                "Keyboard",
                "Set your keys and controllers",
                "Click a key and press the new one (hold Shift, Ctrl or Alt for a combination); Escape leaves it as it is.",
            )),
            Page::Tutorials => Some((
                "Lessons",
                "Learn to drive the bus",
                "OMSI 2's own lessons: each opens its situation, with its pages beside the picture (Enter turns the page).",
            )),
            Page::Diagnostics => Some((
                "Build & log",
                "Check what goes wrong",
                "This build, the OMSI 2 folder, the game's log and the games running: to look at, or send along.",
            )),
            Page::Settings => Some((
                "Graphics, sound & driving",
                "Set up the game",
                "Pick a section on the left or search every option. A dot marks what you changed; the line under each option says when a change takes effect.",
            )),
            Page::Setup => Some((
                "Folders",
                "Find the original game",
                "Where the original OMSI 2 is, where this game's program is, and where the Depot keeps its own files.",
            )),
        }
    }

    /// The icon, by its name in `omsi-ui` (thin outlines, as Windows 11 draws them).
    pub fn icon(self) -> &'static str {
        match self {
            Page::Drive => "o-home",
            Page::Vehicles => "o-bus",
            Page::Maps => "o-map",
            Page::Mods => "o-package",
            Page::Timetable => "o-clock",
            Page::Profile => "o-user",
            Page::Controls => "o-gamepad",
            Page::Tutorials => "o-help",
            Page::Diagnostics => "o-pulse",
            Page::Settings => "o-gear",
            Page::Setup => "o-folder",
        }
    }

    /// The page called `name` (as `--page` and the sidebar's ids spell it), any case; a page
    /// of two names ("A / B") by its first too.
    pub fn named(name: &str) -> Option<Page> {
        let name = name.trim();
        Page::ALL.into_iter().find(|p| {
            p.title().eq_ignore_ascii_case(name)
                || p.title()
                    .split(" / ")
                    .next()
                    .is_some_and(|first| first.eq_ignore_ascii_case(name))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_page_is_found_by_its_name_and_has_an_icon() {
        for p in Page::ALL {
            assert_eq!(Page::named(p.title()), Some(p));
            assert_eq!(Page::named(&p.title().to_uppercase()), Some(p));
            assert!(omsi_ui::icons::svg(p.icon()).is_some(), "{p:?}");
        }
        assert_eq!(Page::named("nope"), None);
    }

    /// Every page but Drive has the concept's heading, and where it is never just repeats the
    /// page's name or its title.
    #[test]
    fn every_page_but_drive_has_a_heading() {
        for p in Page::ALL {
            match p.heading() {
                None => assert_eq!(p, Page::Drive),
                Some((place, title, line)) => {
                    assert!(!place.eq_ignore_ascii_case(p.title()), "{p:?}");
                    assert!(!title.eq_ignore_ascii_case(p.title()), "{p:?}");
                    assert!(!place.is_empty() && !title.is_empty() && !line.is_empty());
                }
            }
        }
    }

    /// Every page is in the list or at the foot once, Drive (sessions start from Maps) aside.
    #[test]
    fn the_list_and_the_foot_cover_every_page_once() {
        let mut all: Vec<_> = Page::listed()
            .iter()
            .chain(Page::foot())
            .chain([&Page::Drive])
            .copied()
            .collect();
        assert_eq!(all.len(), Page::ALL.len());
        all.dedup();
        assert_eq!(all.len(), Page::ALL.len());
        assert_eq!(Page::listed()[0], Page::Maps);
        assert!(!Page::listed().contains(&Page::Drive));
        assert_eq!(Page::named("profile"), Some(Page::Profile));
    }
}
