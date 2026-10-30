//! Where the parts of the game menu and the information bar were drawn this frame, in
//! physical pixels: what the mouse, the touch screen and the keyboard test against. The
//! old drawing (`hud::implementation::legacy`) and the new menu pages
//! (`ui::implementation::pages`) both write them.

pub struct Hits {
    /// Where the game menu's lines were drawn this frame (physical pixels), for the mouse.
    pub menu_rects: Vec<[f32; 4]>,
    /// Per line of `menu_rects`, where the arrows round its value are (a list's setting,
    /// `hud::lists::ADJUST`): `[from, to, plus]` - a click from `from` to `to` steps it
    /// down, one right of `plus` up.
    pub menu_arrows: Vec<Option<[f32; 3]>>,
    pub menu_scroll_thumb: Option<[f32; 4]>,
    pub menu_scroll_track: Option<[f32; 4]>,
    /// Where the controls of the settings rows were drawn (a slider's track, a stepper), one
    /// entry per line in `menu_rects`: a click there sets the value.
    pub menu_ctl: Vec<Option<[f32; 4]>>,
    /// The sidebar of a settings window: one box per page, the way back last.
    pub menu_side: Vec<[f32; 4]>,
    /// The rows of the timetable beside a line's tours (the stops to start from), the stop the
    /// first of them is, and the button that starts the trip.
    pub menu_pane: Vec<[f32; 4]>,
    pub menu_pane_start: usize,
    pub menu_pane_go: Option<[f32; 4]>,
    /// The whole timetable pane beside the tours: the wheel over it scrolls its stops.
    pub menu_pane_box: Option<[f32; 4]>,
    /// Its scroll bar when it has more stops than it shows: the track, the thumb (widened
    /// to be hit), how many stops there are and how many it shows - for the mouse to drag.
    pub menu_pane_scroll: Option<([f32; 4], [f32; 4], usize, usize)>,
    /// The two arrows beside the time of a tour: the trip before, the next one.
    pub menu_time: Vec<[f32; 4]>,
    /// The first line of the menu shown (a long menu scrolls: `menu_rects[k]` is line
    /// `menu_start + k`).
    pub menu_start: usize,
    /// How many lines the menu shows at once, and how high one is (physical pixels): a
    /// finger's drag is turned into lines with it.
    pub menu_rows: usize,
    pub menu_row_h: f32,
    /// The entries of the drop-down shown (their rects), the first of them, and how many fit.
    pub dd_rects: Vec<[f32; 4]>,
    pub dd_top: usize,
    pub dd_rows: usize,
    /// The drop-down's scroll bar when it has more entries than it shows: its track and
    /// its thumb (widened to be hit), for the mouse to drag.
    pub dd_scroll: Option<([f32; 4], [f32; 4])>,
    /// Where the information bar was drawn (within the panel, before `Ui::draw_at` moves it), for the
    /// navigator to keep out of its way.
    pub info_rect: Option<[f32; 4]>,
}

impl Default for Hits {
    fn default() -> Self {
        Hits {
            menu_rects: Vec::new(),
            menu_arrows: Vec::new(),
            menu_scroll_thumb: None,
            menu_scroll_track: None,
            menu_ctl: Vec::new(),
            menu_side: Vec::new(),
            menu_pane: Vec::new(),
            menu_pane_start: 0,
            menu_pane_go: None,
            menu_pane_box: None,
            menu_pane_scroll: None,
            menu_time: Vec::new(),
            menu_start: 0,
            menu_rows: 0,
            menu_row_h: 1.0,
            dd_rects: Vec::new(),
            dd_top: 0,
            dd_rows: 8,
            dd_scroll: None,
            info_rect: None,
        }
    }
}

impl Hits {
    /// Move every rectangle `x` pixels right (a panel of a spanning window).
    pub fn shift(&mut self, x: f32) {
        for rect in self
            .menu_rects
            .iter_mut()
            .chain(self.menu_side.iter_mut())
            .chain(self.menu_pane.iter_mut())
            .chain(self.menu_time.iter_mut())
            .chain(self.dd_rects.iter_mut())
            .chain(self.menu_ctl.iter_mut().flatten())
            .chain(self.menu_scroll_thumb.iter_mut())
            .chain(self.menu_scroll_track.iter_mut())
            .chain(self.menu_pane_go.iter_mut())
            .chain(self.menu_pane_box.iter_mut())
        {
            rect[0] += x;
            rect[2] += x;
        }
        for arrows in self.menu_arrows.iter_mut().flatten() {
            for at in arrows {
                *at += x;
            }
        }
        if let Some((track, thumb)) = &mut self.dd_scroll {
            for rect in [track, thumb] {
                rect[0] += x;
                rect[2] += x;
            }
        }
        if let Some((track, thumb, _, _)) = &mut self.menu_pane_scroll {
            for rect in [track, thumb] {
                rect[0] += x;
                rect[2] += x;
            }
        }
    }
}
