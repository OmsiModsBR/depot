//! The launcher's pages, in the order of the sidebar.

/// A page of the launcher. The first ten sit in the sidebar's list, `Settings` and `Setup`
/// at its foot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Drive,
    Vehicles,
    Maps,
    Mods,
    Timetable,
    Sessions,
    Profile,
    Controls,
    Tutorials,
    Diagnostics,
    Settings,
    Setup,
}

impl Page {
    /// Every page, the list first, then the foot.
    pub const ALL: [Page; 12] = [
        Page::Drive,
        Page::Vehicles,
        Page::Maps,
        Page::Mods,
        Page::Timetable,
        Page::Sessions,
        Page::Profile,
        Page::Controls,
        Page::Tutorials,
        Page::Diagnostics,
        Page::Settings,
        Page::Setup,
    ];

    /// The pages of the sidebar's list.
    pub fn listed() -> &'static [Page] {
        &Page::ALL[..10]
    }

    /// The pages at the sidebar's foot.
    pub fn foot() -> &'static [Page] {
        &Page::ALL[10..]
    }

    pub fn title(self) -> &'static str {
        match self {
            Page::Drive => "Drive",
            Page::Vehicles => "Vehicles",
            Page::Maps => "Maps",
            Page::Mods => "Mods",
            Page::Timetable => "Timetable",
            Page::Sessions => "Sessions",
            Page::Profile => "Profile",
            Page::Controls => "Controls",
            Page::Tutorials => "Tutorials",
            Page::Diagnostics => "Diagnostics",
            Page::Settings => "Settings",
            Page::Setup => "Setup",
        }
    }

    /// The icon, by its name in `omsi-ui`.
    pub fn icon(self) -> &'static str {
        match self {
            Page::Drive => "home",
            Page::Vehicles => "airport_shuttle",
            Page::Maps => "map",
            Page::Mods => "extension",
            Page::Timetable => "schedule",
            Page::Sessions => "play_arrow",
            Page::Profile => "person",
            Page::Controls => "sports_esports",
            Page::Tutorials => "help",
            Page::Diagnostics => "monitor",
            Page::Settings => "settings",
            Page::Setup => "folder_open",
        }
    }

    /// The page called `name` (as `--page` and the sidebar's ids spell it), any case.
    pub fn named(name: &str) -> Option<Page> {
        Page::ALL
            .into_iter()
            .find(|p| p.title().eq_ignore_ascii_case(name.trim()))
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

    #[test]
    fn the_list_and_the_foot_cover_every_page_once() {
        let mut all: Vec<_> = Page::listed().iter().chain(Page::foot()).copied().collect();
        assert_eq!(all.len(), Page::ALL.len());
        all.dedup();
        assert_eq!(all.len(), Page::ALL.len());
    }
}
