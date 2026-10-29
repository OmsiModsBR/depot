//! The "Add vehicle" window's state: what it offers (makers, models, liveries, depot files,
//! fleet numbers, the registration), what the player chose in it, what the overlay draws of
//! it and which picture it shows.

use omsi_ui::ingame::Preview;

/// A picture of a vehicle drawn ahead of time, read for the window: RGBA, and where the
/// vehicle stands in it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Picture {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
    pub part: [f32; 4],
}

/// What names the picture of `bus` in `paint` under `root`.
pub fn picture_key(root: &std::path::Path, bus: &str, paint: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (root, bus, paint).hash(&mut h);
    h.finish()
}

/// What the overlay draws of the window, owned so that it can cross to the renderer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AddFrame {
    pub makers: Vec<String>,
    pub(in crate::ui) maker: usize,
    pub vehicles: Vec<String>,
    pub vehicle: usize,
    pub(in crate::ui) liveries: Vec<String>,
    pub(in crate::ui) livery: usize,
    pub(in crate::ui) depots: Vec<String>,
    pub(in crate::ui) depot: usize,
    pub(in crate::ui) numbers: Vec<String>,
    pub(in crate::ui) number: usize,
    pub(in crate::ui) plate: String,
    pub plate_focused: bool,
    pub description: String,
    /// Which picture the window shows (see [`picture_key`]).
    pub picture: Option<u64>,
    /// The live 3D view, while it is on: the vehicle and paint it should show
    /// (`preview::tag`), and whether it could not be shown.
    pub live: Option<(u64, bool)>,
}

impl AddFrame {
    /// The player has the live 3D view on.
    pub fn live_on(&self) -> bool {
        self.live.is_some()
    }

    /// What the picture box shows, given the picture the overlay has: which one it is, and
    /// its texture, width over height and the vehicle's part (none: there is none of it).
    /// With the live view on, its frame the overlay has (which vehicle and paint it shows,
    /// and its texture) instead.
    pub fn preview(
        &self,
        have: Option<(u64, Option<(usize, f32, [f32; 4])>)>,
        live: Option<(u64, usize)>,
    ) -> Preview {
        if let Some((want, failed)) = self.live {
            return match live {
                _ if failed => Preview::Failed,
                Some((tag, tex)) if tag == want => Preview::Live { tex },
                _ => Preview::Starting,
            };
        }
        match (self.picture, have) {
            (Some(want), Some((key, Some((tex, aspect, part))))) if want == key => {
                Preview::Picture { tex, aspect, part }
            }
            (Some(want), Some((key, None))) if want == key => Preview::Missing,
            (None, _) => Preview::Missing,
            _ => Preview::Loading,
        }
    }
}

/// The window's state on the game's side.
#[derive(Debug, Default)]
pub struct AddWindow {
    /// The manufacturers with their models as (label, vehicle file), as the place list has them.
    makers: Vec<(String, Vec<(String, String)>)>,
    maker: usize,
    /// The chosen model, among the chosen manufacturer's.
    vehicle: usize,
    /// The chosen vehicle's liveries by name; the first line is "Random livery".
    liveries: Vec<String>,
    livery: usize,
    /// The chosen vehicle's depot files as (label, file name); the first line is the map's.
    depots: Vec<(String, String)>,
    depot: usize,
    /// The chosen vehicle's fleet numbers (its `[number]` list) and the one picked.
    numbers: Vec<String>,
    number: usize,
    /// The registration typed (empty: the vehicle's own) and whether its field has the keyboard.
    pub(in crate::ui) plate: String,
    pub(in crate::ui) plate_focused: bool,
    /// The chosen vehicle's description, as its author wrote it.
    description: String,
    /// The picture asked for (see [`picture_key`]).
    picture: Option<u64>,
    /// A press over the window was taken: its release goes to the window too.
    pub(in crate::ui) pressed: bool,
}

/// A line of the paint or garage list as the window shows it: the first ("Random livery",
/// "The map's depot file") translated, the vehicle's own names as they are.
fn own_line(i: usize, line: &str) -> String {
    if i == 0 {
        omsi_ui::tr(line).into_owned()
    } else {
        line.to_string()
    }
}

/// The longest registration the field takes.
const PLATE_MAX: usize = 16;

const RANDOM_LIVERY: &str = "Random livery";

/// Why no vehicle is added now.
pub(crate) const PLACING: &str = "Put down the vehicle on the mouse first";

const MAP_DEPOT: &str = "The map's depot file";

impl AddWindow {
    pub fn new(makers: Vec<(String, Vec<(String, String)>)>) -> AddWindow {
        AddWindow {
            makers,
            liveries: vec![RANDOM_LIVERY.to_string()],
            depots: vec![(MAP_DEPOT.to_string(), String::new())],
            ..Default::default()
        }
    }

    pub fn frame(&self) -> AddFrame {
        AddFrame {
            makers: self.makers.iter().map(|m| m.0.clone()).collect(),
            maker: self.maker,
            vehicles: self.models().iter().map(|v| v.0.clone()).collect(),
            vehicle: self.vehicle,
            // (the first lines are the interface's own, in the player's language; the rest are
            // the game's names, drawn as they are)
            liveries: self
                .liveries
                .iter()
                .enumerate()
                .map(|(i, l)| own_line(i, l))
                .collect(),
            livery: self.livery,
            depots: self
                .depots
                .iter()
                .enumerate()
                .map(|(i, d)| own_line(i, &d.0))
                .collect(),
            depot: self.depot,
            numbers: self.numbers.clone(),
            number: self.number,
            plate: self.plate.clone(),
            plate_focused: self.plate_focused,
            description: self.description.clone(),
            picture: self.picture,
            live: None,
        }
    }

    /// The chosen manufacturer's models.
    fn models(&self) -> &[(String, String)] {
        self.makers
            .get(self.maker)
            .map(|m| m.1.as_slice())
            .unwrap_or(&[])
    }

    /// Where the manufacturer `maker` and its vehicle file `file` are in the lists (the
    /// first model of the manufacturer when its file is gone; none when the manufacturer is).
    pub fn find(&self, maker: &str, file: &str) -> Option<(usize, usize)> {
        let m = self.makers.iter().position(|m| m.0 == maker)?;
        let v = self.makers[m]
            .1
            .iter()
            .position(|v| v.1 == file)
            .unwrap_or(0);
        Some((m, v))
    }

    /// The manufacturer and the vehicle file chosen now.
    pub fn chosen(&self) -> Option<(String, String)> {
        let maker = self.makers.get(self.maker)?.0.clone();
        let file = self.models().get(self.vehicle)?.1.clone();
        Some((maker, file))
    }

    /// Choose the manufacturer at `i`: its first model is chosen with it (see [`choose`]).
    /// Returns that model's file, for the caller to read its lists.
    pub fn choose_maker(&mut self, i: usize) -> Option<String> {
        if i >= self.makers.len() {
            return None;
        }
        self.maker = i;
        self.vehicle = 0;
        self.vehicle_file(0).map(String::from)
    }

    /// Choose the model at `i` of the chosen manufacturer with its `liveries`, `depots` (as
    /// the place list reads them), fleet numbers and description; the livery, the depot file
    /// and the number go back to the first line and the typed registration goes.
    pub fn choose(
        &mut self,
        i: usize,
        liveries: Vec<String>,
        depots: Vec<(String, String)>,
        numbers: Vec<String>,
        description: String,
    ) {
        if i >= self.models().len() {
            return;
        }
        self.vehicle = i;
        self.liveries = std::iter::once(RANDOM_LIVERY.to_string())
            .chain(liveries)
            .collect();
        self.depots = std::iter::once((MAP_DEPOT.to_string(), String::new()))
            .chain(depots)
            .collect();
        self.livery = 0;
        self.depot = 0;
        self.numbers = numbers;
        self.number = 0;
        self.plate.clear();
        self.plate_focused = false;
        self.description = description;
    }

    /// Pick the fleet number at line `i`.
    pub fn set_number(&mut self, i: usize) {
        if i < self.numbers.len() {
            self.number = i;
        }
    }

    pub fn focus_plate(&mut self) {
        self.plate_focused = true;
    }

    /// Text typed into the registration's field. True when the field took it.
    pub fn type_plate(&mut self, text: &str) -> bool {
        if !self.plate_focused {
            return false;
        }
        for c in text.chars().filter(|c| !c.is_control()) {
            if self.plate.chars().count() < PLATE_MAX {
                self.plate.push(c);
            }
        }
        true
    }

    /// The fleet number and the registration to put down (`None`: the vehicle's own).
    pub fn identity(&self) -> (Option<String>, Option<String>) {
        let number = self.numbers.get(self.number).cloned();
        let plate = Some(self.plate.trim().to_string()).filter(|p| !p.is_empty());
        (number, plate)
    }

    pub(crate) fn vehicle_file(&self, i: usize) -> Option<&str> {
        self.models().get(i).map(|v| v.1.as_str())
    }

    pub fn set_livery(&mut self, i: usize) {
        if i < self.liveries.len() {
            self.livery = i;
        }
    }

    pub fn set_depot(&mut self, i: usize) {
        if i < self.depots.len() {
            self.depot = i;
        }
    }

    /// The vehicle file and the paint whose picture the window shows: the paint chosen, the
    /// bus's own (the empty name) for "Random livery".
    pub fn picture_look(&self) -> Option<(String, String)> {
        let file = self.models().get(self.vehicle)?.1.clone();
        let paint = if self.livery > 0 {
            self.liveries[self.livery].trim().to_string()
        } else {
            String::new()
        };
        Some((file, paint))
    }

    pub(crate) fn set_picture(&mut self, key: u64) {
        self.picture = Some(key);
    }

    /// What "Place" puts down: the vehicle's file, its livery (`None`: random) and its depot
    /// file (`None`: the map's).
    pub fn placement(&self) -> Option<(String, Option<String>, Option<String>)> {
        let (_, file) = self.models().get(self.vehicle)?;
        let livery = (self.livery > 0).then(|| self.liveries[self.livery].clone());
        let depot = (self.depot > 0).then(|| self.depots[self.depot].1.clone());
        Some((file.clone(), livery, depot))
    }
}
