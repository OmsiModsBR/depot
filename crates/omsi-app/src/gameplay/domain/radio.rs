//! The bus radio's station lists and the rules of tuning: the default list, the
//! `radio.cfg` text, the frequencies of a map's stations, and the display of a station's name
//! and status.

pub const DEFAULT_STATIONS: &[(&str, &str)] = &[
    (
        "radioeins",
        "https://dispatcher.rndfnk.com/rbb/radioeins/live/mp3/mid",
    ),
    (
        "Berlin 88.8",
        "https://dispatcher.rndfnk.com/rbb/rbb888/live/mp3/mid",
    ),
    (
        "Deutschlandfunk",
        "https://st01.sslstream.dlf.de/dlf/01/128/mp3/stream.mp3",
    ),
    ("SWR3", "https://liveradio.swr.de/sw282p3/swr3/play.mp3"),
    (
        "Европа Плюс",
        "https://ep256.hostingradio.ru:8052/europaplus256.mp3",
    ),
    (
        "Русское Радио",
        "https://rusradio.hostingradio.ru/rusradio128.mp3",
    ),
    (
        "Ретро FM",
        "https://retro.hostingradio.ru:8043/retro256.mp3",
    ),
    (
        "Наше Радио",
        "https://nashe1.hostingradio.ru:80/nashe-128.mp3",
    ),
    ("Radio Paradise", "https://stream.radioparadise.com/mp3-128"),
];
/// `radio.cfg` as it is written the first time: what it is for, the volume, the default list.
pub fn default_text() -> String {
    let mut text = String::from(
        "# Internet radio for the buses' radios: one station per line, `name = address`\n\
         # (an MP3, AAC or Ogg stream, or an .m3u/.pls playlist). A radio's station\n\
         # button n plays the n-th station, a cassette player the first; Shift+R in the\n\
         # game steps through the list. volume = 0..1.\n\
         volume = 0.7\n",
    );
    for (n, u) in DEFAULT_STATIONS {
        text.push_str(&format!("{n} = {u}\n"));
    }
    text
}
/// The station lines of a radio.cfg as they stand: (name, all behind the `=` - the address
/// and any frequencies after it).
pub fn station_lines(text: &str) -> Vec<(String, String)> {
    text.lines()
        .map(|l| l.trim().trim_start_matches('\u{feff}'))
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .map(|(n, v)| (n.trim().to_string(), v.trim().to_string()))
        .filter(|(n, v)| {
            !n.eq_ignore_ascii_case("volume")
                && !v.split('|').next().unwrap_or("").trim().is_empty()
        })
        .collect()
}
/// `text` (a radio.cfg) with `list` for its stations: its comments and volume stay as they
/// are, the stations stand where its first one stood (after the rest when it had none). A
/// station without an address is left out, and a name loses what would end it or make the
/// line a comment (`=`, a line break, a leading `#`).
pub fn with_stations(text: &str, list: &[(String, String)]) -> String {
    let mut out = String::new();
    let mut placed = false;
    let put = |out: &mut String| {
        for (k, (name, value)) in list.iter().enumerate() {
            let value = value.replace(['\r', '\n'], " ");
            if value.split('|').next().unwrap_or("").trim().is_empty() {
                continue;
            }
            let name = name.replace(['=', '\r', '\n'], " ");
            let name = name.trim().trim_start_matches('#').trim();
            let name = if name.is_empty() {
                format!("Station {}", k + 1)
            } else {
                name.to_string()
            };
            out.push_str(&format!("{name} = {}\n", value.trim()));
        }
    };
    for line in text.lines() {
        if !station_lines(line).is_empty() {
            if !placed {
                put(&mut out);
                placed = true;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if !placed {
        put(&mut out);
    }
    out
}
/// A frequency a station is on (as a radio's display writes it: `94.6`), and where: a
/// place on the map in the game's own metres - x east, y north, as the game's log gives a
/// bus's position - near which that frequency is the one on air. Without a place it is
/// the station's frequency everywhere.
#[derive(Clone, Debug, PartialEq)]
pub struct Frequency {
    pub mhz: String,
    pub at: Option<(f64, f64)>,
}
/// `94.6`, `94.6 MHz` or `94.6 @ 25400, 990`.
fn parse_frequency(text: &str) -> Option<Frequency> {
    let (mhz, at) = match text.split_once('@') {
        Some((m, p)) => (m, Some(p)),
        None => (text, None),
    };
    let mhz = mhz.trim().to_ascii_lowercase();
    let mhz: f32 = mhz
        .trim_end_matches("mhz")
        .trim()
        .replace(',', ".")
        .parse()
        .ok()
        .filter(|v: &f32| *v > 0.0 && v.is_finite())?;
    let at = match at {
        Some(p) => {
            let (x, y) = p.split_once(',')?;
            Some((x.trim().parse::<f64>().ok()?, y.trim().parse::<f64>().ok()?))
        }
        None => None,
    };
    Some(Frequency {
        mhz: format!("{mhz:.1}"),
        at,
    })
}
/// The frequencies of the stations that name any, by address (lower case).
pub(crate) type Frequencies = std::collections::HashMap<String, Vec<Frequency>>;
/// Which of a station's frequencies is on air at (x, y): that of the nearest place, and
/// the one without a place where no place is given.
pub fn frequency_at(on: &[Frequency], x: f64, y: f64) -> Option<&str> {
    let near = on
        .iter()
        .filter_map(|f| {
            f.at.map(|(fx, fy)| ((fx - x).powi(2) + (fy - y).powi(2), f))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, f)| f);
    near.or_else(|| on.iter().find(|f| f.at.is_none()))
        .map(|f| f.mhz.as_str())
}
/// What a radio.cfg says: its stations, its volume where one is set, and the frequencies
/// of the stations that name any (by address, lower case). A line is `name = address`,
/// and behind the address `| frequency` or `| frequency @ x, y` as often as the station
/// has frequencies along the map.
pub fn parse_stations(text: &str) -> (Vec<(String, String)>, Option<f32>, Frequencies) {
    let (mut stations, mut volume, mut frequencies) = (Vec::new(), None, Frequencies::new());
    for line in text.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        let (name, value) = (name.trim(), value.trim());
        if name.eq_ignore_ascii_case("volume") {
            volume = value
                .parse::<f32>()
                .ok()
                .map(|v| v.clamp(0.0, 1.0))
                .or(volume);
            continue;
        }
        let mut parts = value.split('|');
        let url = parts.next().unwrap_or("").trim();
        if url.is_empty() {
            continue;
        }
        let on: Vec<Frequency> = parts.filter_map(parse_frequency).collect();
        if !on.is_empty() {
            frequencies.insert(url.to_ascii_lowercase(), on);
        }
        stations.push((name.to_string(), url.to_string()));
    }
    (stations, volume, frequencies)
}
/// Characters in a line of a radio's text display (the "Magnitola" radio of P3ta's SOR
/// buses and its kin: two lines of ten).
pub(crate) const DISPLAY_WIDTH: usize = 10;
/// A station's name and its song as one line a simple text display can show: plain Latin
/// letters (its fonts have little else), no `@` (the display's line break).
pub fn display_line(name: &str, status: &str) -> String {
    let waiting = status.is_empty() || status.ends_with('…') || status.eq_ignore_ascii_case(name);
    let text = if waiting {
        name.to_string()
    } else {
        format!("{name} - {status}")
    };
    let mut out = String::new();
    for c in text.chars() {
        let plain: &str = match c {
            '@' => " ",
            c if c.is_ascii() && !c.is_ascii_control() => {
                out.push(c);
                continue;
            }
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'ą' => "a",
            'Á' | 'À' | 'Â' | 'Ä' | 'Ã' | 'Å' | 'Ą' => "A",
            'č' | 'ç' | 'ć' => "c",
            'Č' | 'Ç' | 'Ć' => "C",
            'ď' => "d",
            'Ď' => "D",
            'é' | 'ě' | 'è' | 'ê' | 'ë' | 'ę' => "e",
            'É' | 'Ě' | 'È' | 'Ê' | 'Ë' | 'Ę' => "E",
            'í' | 'ì' | 'î' | 'ï' => "i",
            'Í' | 'Ì' | 'Î' | 'Ï' => "I",
            'ľ' | 'ĺ' | 'ł' => "l",
            'Ľ' | 'Ĺ' | 'Ł' => "L",
            'ň' | 'ñ' | 'ń' => "n",
            'Ň' | 'Ñ' | 'Ń' => "N",
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ő' | 'ø' => "o",
            'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ' | 'Ő' | 'Ø' => "O",
            'ř' | 'ŕ' => "r",
            'Ř' | 'Ŕ' => "R",
            'š' | 'ś' => "s",
            'Š' | 'Ś' => "S",
            'ß' => "ss",
            'ť' => "t",
            'Ť' => "T",
            'ú' | 'ù' | 'û' | 'ü' | 'ů' | 'ű' => "u",
            'Ú' | 'Ù' | 'Û' | 'Ü' | 'Ů' | 'Ű' => "U",
            'ý' | 'ÿ' => "y",
            'Ý' => "Y",
            'ž' | 'ź' | 'ż' => "z",
            'Ž' | 'Ź' | 'Ż' => "Z",
            '–' | '—' => "-",
            '’' | '‘' => "'",
            '…' => "...",
            _ => "",
        };
        out.push_str(plain);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
/// `width` characters of `text` at `seconds` after it came up: all of a text that fits,
/// else the text running through from right to left, four characters a second after a
/// moment at its beginning, with a gap before it comes round again.
pub fn marquee(text: &str, width: usize, seconds: f32) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= width {
        return format!("{text:<width$}");
    }
    let round = chars.len() + 3;
    let at = ((seconds - 1.5).max(0.0) * 4.0) as usize % round;
    (0..width)
        .map(|i| chars.get((at + i) % round).copied().unwrap_or(' '))
        .collect()
}
