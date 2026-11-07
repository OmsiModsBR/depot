//! `helper\stars.dat` - the night sky's stars: one star a line, its right ascension and
//! declination (degrees), its brightness (relative to a star of magnitude 0) and its
//! spectral class, separated by tabs.

mod map;

pub use map::{face_of, star_size, StarMap, STAR_MAP_CELLS};

use std::path::Path;

/// A star's spectral class: the colour it is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StarClass {
    O,
    W,
    B,
    A,
    F,
    G,
    K,
    M,
    /// Any other letter (the file's few C, N and S stars): drawn as an M star, the red
    /// giants they are.
    Other,
}

impl StarClass {
    fn of(letter: char) -> StarClass {
        match letter.to_ascii_uppercase() {
            'O' => StarClass::O,
            'W' => StarClass::W,
            'B' => StarClass::B,
            'A' => StarClass::A,
            'F' => StarClass::F,
            'G' => StarClass::G,
            'K' => StarClass::K,
            'M' => StarClass::M,
            _ => StarClass::Other,
        }
    }

    /// The class's colour at full brightness (RGB, 0..255), from blue-white O to orange M.
    pub fn rgb(self) -> [u8; 3] {
        match self {
            StarClass::O => [209, 198, 255],
            StarClass::W => [215, 205, 255],
            StarClass::B => [224, 217, 255],
            StarClass::A => [240, 244, 255],
            StarClass::F => [254, 255, 225],
            StarClass::G => [255, 252, 185],
            StarClass::K => [255, 228, 147],
            StarClass::M | StarClass::Other => [255, 198, 147],
        }
    }

    /// The class's number in a [`StarMap`] (1..8; 0 marks an empty cell there).
    pub fn code(self) -> u8 {
        match self {
            StarClass::O => 1,
            StarClass::W => 2,
            StarClass::B => 3,
            StarClass::A => 4,
            StarClass::F => 5,
            StarClass::G => 6,
            StarClass::K => 7,
            StarClass::M | StarClass::Other => 8,
        }
    }
}

/// One star of the catalogue.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Star {
    /// Right ascension and declination, degrees.
    pub ra: f64,
    pub dec: f64,
    /// Brightness relative to a star of magnitude 0 (`10^(-0.4 m)`: Sirius about 3.8,
    /// the faintest of the file 0.004).
    pub flux: f32,
    pub class: StarClass,
}

impl Star {
    /// The unit vector towards the star in the equatorial frame: x towards the vernal
    /// equinox, z towards the celestial north pole.
    pub fn direction(&self) -> [f64; 3] {
        let (ra, dec) = (self.ra.to_radians(), self.dec.to_radians());
        [dec.cos() * ra.cos(), dec.cos() * ra.sin(), dec.sin()]
    }
}

/// The stars of `helper\stars.dat`, and the lines that could not be read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StarCatalog {
    pub stars: Vec<Star>,
    /// The (1-based) numbers of the lines left out, each with what was wrong with it.
    pub skipped: Vec<(usize, String)>,
}

impl StarCatalog {
    pub fn load(path: &Path) -> std::io::Result<StarCatalog> {
        let bytes = std::fs::read(path)?;
        Ok(Self::parse(&String::from_utf8_lossy(&bytes)))
    }

    /// Reads the file's lines; a blank line is passed over, a line that does not hold a
    /// star (too few fields, a number that is not one, a place off the sphere, a negative
    /// brightness) is left out and listed in [`StarCatalog::skipped`].
    pub fn parse(text: &str) -> StarCatalog {
        let mut out = StarCatalog::default();
        for (i, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match parse_star(line) {
                Ok(s) => out.stars.push(s),
                Err(e) => out.skipped.push((i + 1, e)),
            }
        }
        out
    }
}

fn parse_star(line: &str) -> Result<Star, String> {
    let mut f = line.split('\t').map(str::trim);
    let mut num = |name: &str| -> Result<f64, String> {
        let v = f.next().ok_or_else(|| format!("no {name}"))?;
        v.parse::<f64>()
            .ok()
            .filter(|x| x.is_finite())
            .ok_or_else(|| format!("{name} {v:?} is not a number"))
    };
    let ra = num("right ascension")?;
    let dec = num("declination")?;
    let flux = num("brightness")?;
    if !(-90.0..=90.0).contains(&dec) {
        return Err(format!("declination {dec} is off the sphere"));
    }
    if flux < 0.0 {
        return Err(format!("brightness {flux} is negative"));
    }
    let letter = f
        .next()
        .and_then(|c| c.chars().next())
        .ok_or("no spectral class")?;
    Ok(Star {
        ra: ra.rem_euclid(360.0),
        dec,
        flux: flux as f32,
        class: StarClass::of(letter),
    })
}
