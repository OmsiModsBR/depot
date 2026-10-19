//! `Inputs/gamectrler.cfg`: the game controllers OMSI drives - steering wheels, pedals,
//! joysticks, gamepads. Each `[ctrl]` block names a device and says what its eight
//! DirectInput axes (X, Y, Z, Rx, Ry, Rz and the two sliders) do: a pair per axis of the
//! function (-1 none, 0 steering, 1 throttle, 2 brake, 3 clutch, 4 throttle and brake on one
//! axis) and flags (bit 0: the axis runs the other way, as the G25's pedals do).
//! `[buttons]` lists per button the key action it presses, `[FFScale]` the force feedback
//! strengths. The Depot's own blocks (`[openOMSI.Latching]`, `[openOMSI.FFInvert]`) follow
//! a device's. This is the only reader and writer of the file; the game maps the devices.

use std::path::Path;

/// What one axis of a device does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Func {
    Steering,
    Throttle,
    Brake,
    Clutch,
    ThrottleBrake,
    /// The driver's head turned left and right, up and down (#454; openOMSI's own: OMSI's
    /// file has the five above, numbered 0 to 4).
    LookX,
    LookY,
}

impl Func {
    /// The file's number of the function (-1 none).
    pub fn code(f: Option<Func>) -> i32 {
        match f {
            None => -1,
            Some(Func::Steering) => 0,
            Some(Func::Throttle) => 1,
            Some(Func::Brake) => 2,
            Some(Func::Clutch) => 3,
            Some(Func::ThrottleBrake) => 4,
            Some(Func::LookX) => 5,
            Some(Func::LookY) => 6,
        }
    }

    pub fn from_code(c: i32) -> Option<Func> {
        match c {
            0 => Some(Func::Steering),
            1 => Some(Func::Throttle),
            2 => Some(Func::Brake),
            3 => Some(Func::Clutch),
            4 => Some(Func::ThrottleBrake),
            5 => Some(Func::LookX),
            6 => Some(Func::LookY),
            _ => None,
        }
    }

    /// As the options dialog lists them.
    pub const LABELS: [&'static str; 8] = [
        "<none>",
        "Steering",
        "Throttle",
        "Brake",
        "Clutch",
        "Throttle/Brake",
        "Look left / right",
        "Look up / down",
    ];
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DeviceCfg {
    pub name: String,
    /// The line after the name (kept as the file has it).
    pub second: String,
    /// Per DirectInput axis: the function and whether it runs the other way.
    pub axes: [Option<(Func, bool)>; 8],
    /// Per axis the file's flags beyond bit 0 (kept as they are).
    pub axis_flags: [i32; 8],
    /// Per button: the key action (empty: none) and the number after it.
    pub buttons: Vec<(String, String)>,
    /// `[FFScale]`: steering forces (centering and drag), then vibration strength.
    pub ff_scale: Option<(f32, f32)>,
    /// Motor polarity for this device; None uses the existing global setting.
    pub ff_invert: Option<bool>,
    /// `[openOMSI.Latching]`: the buttons (from 0) that are latching switches - a turn signal
    /// lever, a lit hazard button - and switch back when they come out.
    pub latching: Vec<usize>,
}

/// Read and decode the file at `path`.
pub fn read(path: &Path) -> std::io::Result<Vec<DeviceCfg>> {
    Ok(parse(&omsi_cfg::decode_text(&std::fs::read(path)?)))
}

pub fn parse(text: &str) -> Vec<DeviceCfg> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let mut out: Vec<DeviceCfg> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        match lines[i] {
            "[ctrl]" => {
                out.push(DeviceCfg {
                    name: lines.get(i + 1).unwrap_or(&"").to_string(),
                    second: lines.get(i + 2).unwrap_or(&"0").to_string(),
                    ..Default::default()
                });
                i += 3;
            }
            "[axis]" => {
                if let Some(d) = out.last_mut() {
                    for a in 0..8 {
                        let f: i32 = lines
                            .get(i + 1 + a * 2)
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(-1);
                        let flags: i32 = lines
                            .get(i + 2 + a * 2)
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(0);
                        d.axes[a] = Func::from_code(f).map(|f| (f, flags & 1 != 0));
                        d.axis_flags[a] = flags & !1;
                    }
                }
                i += 17;
            }
            "[buttons]" => {
                let n: usize = lines
                    .get(i + 1)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0)
                    .min(512);
                if let Some(d) = out.last_mut() {
                    for b in 0..n {
                        d.buttons.push((
                            lines.get(i + 2 + b * 2).unwrap_or(&"").to_string(),
                            lines.get(i + 3 + b * 2).unwrap_or(&"0").to_string(),
                        ));
                    }
                }
                i += 2 + n * 2;
            }
            "[ffscale]" | "[FFScale]" => {
                if let Some(d) = out.last_mut() {
                    let f = |k: usize| {
                        lines
                            .get(i + k)
                            .map(|v| omsi_cfg::parse_f64(v))
                            .unwrap_or(1.0) as f32
                    };
                    d.ff_scale = Some((f(1), f(2)));
                }
                i += 3;
            }
            "[openOMSI.Latching]" => {
                if let Some(d) = out.last_mut() {
                    // (button numbers as the launcher shows them, from 1)
                    d.latching = lines
                        .get(i + 1)
                        .unwrap_or(&"")
                        .split_whitespace()
                        .filter_map(|v| v.parse::<usize>().ok()?.checked_sub(1))
                        .collect();
                }
                i += 2;
            }
            "[openOMSI.FFInvert]" => {
                if let Some(d) = out.last_mut() {
                    d.ff_invert = lines.get(i + 1).and_then(|v| match *v {
                        "0" => Some(false),
                        "1" => Some(true),
                        _ => None,
                    });
                }
                i += 2;
            }
            _ => i += 1,
        }
    }
    out
}

/// The file's text for `devices`, as OMSI writes it (CR LF).
pub fn to_text(devices: &[DeviceCfg]) -> String {
    let mut t = String::new();
    for d in devices {
        t.push_str(&format!(
            "\r\n[ctrl]\r\n{}\r\n{}\r\n\r\n[axis]\r\n",
            d.name,
            if d.second.is_empty() { "0" } else { &d.second }
        ));
        for a in 0..8 {
            let (f, inv) = match d.axes[a] {
                Some((f, inv)) => (Func::code(Some(f)), inv),
                None => (-1, false),
            };
            t.push_str(&format!("{f}\r\n{}\r\n", d.axis_flags[a] | inv as i32));
        }
        t.push_str(&format!("\r\n[buttons]\r\n{}\r\n", d.buttons.len()));
        for (action, n) in &d.buttons {
            t.push_str(&format!(
                "{action}\r\n{}\r\n",
                if n.is_empty() { "0" } else { n }
            ));
        }
        let (a, b) = d.ff_scale.unwrap_or((1.0, 1.0));
        t.push_str(&format!("\r\n[FFScale]\r\n{a:.3}\r\n{b:.3}\r\n\r\n"));
        if let Some(invert) = d.ff_invert {
            t.push_str(&format!("[openOMSI.FFInvert]\r\n{}\r\n\r\n", invert as u8));
        }
        if !d.latching.is_empty() {
            let numbers: Vec<String> = d.latching.iter().map(|b| (b + 1).to_string()).collect();
            t.push_str(&format!(
                "[openOMSI.Latching]\r\n{}\r\n\r\n",
                numbers.join(" ")
            ));
        }
    }
    t
}

/// Write `devices` to `path` if they read back the same, through a temporary file so a
/// failed write never leaves half a file behind.
pub fn write(path: &Path, devices: &[DeviceCfg]) -> Result<(), String> {
    let text = to_text(devices);
    if to_text(&parse(&text)) != text {
        return Err("Controller configuration did not pass its round-trip check".into());
    }
    let dir = path
        .parent()
        .ok_or("Controller configuration has no parent folder")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    omsi_cfg::write_atomic(path, text).map_err(|e| e.to_string())
}

/// An axis' characteristic from its gamectrler.cfg flags, on 0..1 (after the inversion),
/// as OMSI applies it: 2 extends the range by a quarter, 4 is
/// degressive, 8 progressive, 0x10 makes either of them symmetric round the centre.
pub fn axis_shape(v: f32, flags: i32) -> f32 {
    use std::f32::consts::PI;
    let mut v = v;
    if flags & 2 != 0 {
        v = v * 1.25 - 0.125;
    }
    let bi = flags & 0x10 != 0;
    if flags & 4 != 0 {
        v = if bi {
            ((v - 0.5) * PI).sin() * 0.5 + 0.5
        } else {
            (v * PI * 0.5).sin()
        };
    } else if flags & 8 != 0 {
        v = if bi {
            let d = (v - 0.5) * 2.0;
            0.5 + d.signum() * d * d * 0.5
        } else {
            v * v
        };
    }
    v.clamp(0.0, 1.0)
}

/// The characteristics the set-up offers, with their flag bits (as OMSI's combo box offers them).
pub const AXIS_SHAPES: [(&str, i32); 5] = [
    ("Linear", 0),
    ("Progressive", 8),
    ("Degressive", 4),
    ("Bi-progressive", 8 | 0x10),
    ("Bi-degressive", 4 | 0x10),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devices_round_trip() {
        let mut wheel = DeviceCfg {
            name: "G25 Racing Wheel".into(),
            second: "1".into(),
            buttons: vec![("bus_horn".into(), "0".into()), (String::new(), "0".into())],
            ff_scale: Some((1.5, 0.25)),
            ff_invert: Some(true),
            latching: vec![0, 7],
            ..Default::default()
        };
        wheel.axes[0] = Some((Func::Steering, false));
        wheel.axes[2] = Some((Func::Throttle, true));
        wheel.axes[5] = Some((Func::LookX, false));
        wheel.axis_flags[2] = 8 | 0x10;
        let text = to_text(&[wheel.clone()]);
        assert!(text.starts_with("\r\n[ctrl]\r\nG25 Racing Wheel\r\n1\r\n"));
        assert_eq!(parse(&text), vec![wheel]);
    }

    #[test]
    fn write_checks_the_round_trip_and_reads_back() {
        let dir = std::env::temp_dir().join(format!("depot-gamectrler-{}", std::process::id()));
        let path = dir.join("Inputs").join("gamectrler.cfg");
        let pad = DeviceCfg {
            name: "Pad".into(),
            ..Default::default()
        };
        write(&path, std::slice::from_ref(&pad)).unwrap();
        let read_back = read(&path).unwrap();
        assert_eq!(
            read_back,
            vec![DeviceCfg {
                second: "0".into(),
                ff_scale: Some((1.0, 1.0)),
                ..pad
            }]
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn characteristics_match_omsi() {
        assert!((axis_shape(0.75, 24) - 0.625).abs() < 1e-6);
        assert!((axis_shape(0.5, 24) - 0.5).abs() < 1e-6);
        assert!((axis_shape(0.5, 8) - 0.25).abs() < 1e-6);
        assert!((axis_shape(0.5, 4) - (std::f32::consts::PI / 4.0).sin()).abs() < 1e-6);
        assert!((axis_shape(0.3, 0) - 0.3).abs() < 1e-6);
        assert_eq!(axis_shape(1.0, 2), 1.0);
    }

    #[test]
    fn the_stock_file_round_trips() {
        let Ok(bytes) = std::fs::read("../../../OMSI 2 Original/Inputs/gamectrler.cfg") else {
            return;
        };
        let text = omsi_cfg::decode_text(&bytes);
        let devs = parse(&text);
        assert!(
            devs.iter().any(|d| d.name.contains("G25")),
            "{:?}",
            devs.iter().map(|d| &d.name).collect::<Vec<_>>()
        );
        let again = parse(&to_text(&devs));
        assert_eq!(again, devs);
    }
}
