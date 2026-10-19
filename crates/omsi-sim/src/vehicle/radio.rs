//! The radio's text on the dashboard (the magnitola's line and the station's frequency).

use super::VehicleInstance;

impl VehicleInstance {
    /// The station and the song on a radio whose display is a text of its script. OMSI has
    /// no radio of its own: these radios show names from a list in the script, and a radio
    /// plugin writes what it really plays into a string of theirs. Two kinds are known:
    ///
    /// - a script that reads `Snd_Radio_Text` (the plugin's variable) and puts it behind
    ///   its frequency: the text goes there;
    /// - Dmitrij's "Magnitola" (the radio of P3ta's SOR buses and others): the playlist
    ///   writes `frequency@station` into `mp3_display_track_name` every frame and the
    ///   display `magnitola_1` shows it - `@` is the line break, ten characters a line.
    ///   While the display shows that, its second line is replaced.
    ///
    /// The frequency in front is the script's too, one of its list. Where the station's
    /// own is known (`radio_frequency`: a map says which frequency its stations are on,
    /// and where) that one stands there instead.
    pub(super) fn show_radio_text(&mut self) {
        let Some(text) = self.radio_text.as_ref() else {
            return;
        };
        let frequency = self.radio_frequency.as_deref();
        let p = &self.ty.program;
        if let Some(i) = p.str_var("Snd_Radio_Text") {
            if self.state.str_vars[i as usize] != *text {
                self.state.str_vars[i as usize] = text.clone();
            }
            // (this kind keeps its frequency apart, `90.9 MHz@` in `mp3_freq`, and the
            // display begins with it)
            if let (Some(frequency), Some(display), Some(own)) =
                (frequency, p.str_var("magnitola_1"), p.str_var("mp3_freq"))
            {
                let own = &self.state.str_vars[own as usize];
                if let Some(shown) =
                    own_frequency(own, &self.state.str_vars[display as usize], frequency)
                {
                    self.state.str_vars[display as usize] = shown;
                }
            }
            return;
        }
        let (Some(display), Some(track)) = (
            p.str_var("magnitola_1"),
            p.str_var("mp3_display_track_name"),
        ) else {
            return;
        };
        if let Some(shown) = magnitola_line(
            &self.state.str_vars[track as usize],
            &self.state.str_vars[display as usize],
            text,
            frequency,
        ) {
            self.state.str_vars[display as usize] = shown;
        }
    }
}

/// The "Magnitola" radio's display with `text` as its second line: `track` is what the
/// playlist wrote (`90.9 MHz@R-ZURNAL`), `shown` what the display holds. None while the
/// display shows something else (its welcome, the volume), while the radio is stopped (no
/// station behind the `@`) or off (`text` empty). `own` is the frequency the station is
/// really on, where that is known: it stands for the script's.
pub fn magnitola_line(track: &str, shown: &str, text: &str, own: Option<&str>) -> Option<String> {
    if text.is_empty() || shown != track {
        return None;
    }
    let (frequency, station) = track.split_once('@')?;
    (!station.trim().is_empty()).then(|| format!("{}@{text}", own.unwrap_or(frequency)))
}

/// A display that begins with the script's frequency (`script`: `90.9 MHz@`), with the
/// station's own in its place. None while it shows something else, and for the script's
/// `STOPPED@`, which is no frequency.
pub fn own_frequency(script: &str, shown: &str, own: &str) -> Option<String> {
    if !script.ends_with('@') || !script.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let rest = shown.strip_prefix(script)?;
    Some(format!("{own}@{rest}"))
}
