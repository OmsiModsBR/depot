//! `ailists.cfg` and the parking lists changed line by line. A change touches only the lines
//! it is about: every other byte (comments, blank lines, the order of the blocks, the code
//! page, the line breaks, a last line without a break) is written back as it was read.
//!
//! The groups are read the way [`AiLists::parse`](crate::AiLists::parse) reads them, so a
//! vehicle here is the vehicle the game takes.

use omsi_cfg::codepage::CodePage;
use omsi_cfg::{CfgFile, KeywordRule, TextEncoding};

/// A text file as its lines, each with the line break it ends with.
#[derive(Debug, Clone, PartialEq)]
pub struct TextLines {
    lines: Vec<(String, &'static str)>,
    encoding: TextEncoding,
    /// The break a new line gets: the file's first one (CR LF in a file without any).
    eol: &'static str,
}

impl Default for TextLines {
    fn default() -> Self {
        Self::new()
    }
}

impl TextLines {
    /// An empty new file, written as OMSI's own: Windows-1252 with CR LF.
    pub fn new() -> TextLines {
        TextLines {
            lines: Vec::new(),
            encoding: TextEncoding::CodePage(CodePage::Windows1252),
            eol: "\r\n",
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> TextLines {
        let (text, mut encoding) = omsi_cfg::decode_with_encoding(bytes);
        // (plain ASCII reads as UTF-8; a name with an accent added to it is written in the
        // code page the game reads such a file in)
        if encoding == (TextEncoding::Utf8 { bom: false }) && bytes.is_ascii() {
            encoding = TextEncoding::CodePage(CodePage::Windows1252);
        }
        let mut lines = Vec::new();
        let mut cur = String::new();
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\r' => {
                    let eol = if chars.peek() == Some(&'\n') {
                        chars.next();
                        "\r\n"
                    } else {
                        "\r"
                    };
                    lines.push((std::mem::take(&mut cur), eol));
                }
                '\n' => lines.push((std::mem::take(&mut cur), "\n")),
                _ => cur.push(c),
            }
        }
        if !cur.is_empty() {
            lines.push((cur, ""));
        }
        let eol = lines
            .iter()
            .map(|l| l.1)
            .find(|e| !e.is_empty())
            .unwrap_or("\r\n");
        TextLines {
            lines,
            encoding,
            eol,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let text: String = self
            .lines
            .iter()
            .flat_map(|(l, e)| [l.as_str(), e])
            .collect();
        omsi_cfg::encode_like(&text, self.encoding)
    }

    pub fn len(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn line(&self, i: usize) -> &str {
        self.lines.get(i).map(|l| l.0.as_str()).unwrap_or("")
    }

    /// The lines as the content loaders see them.
    pub fn cfg(&self) -> CfgFile {
        let mut text = String::new();
        for (l, _) in &self.lines {
            text.push_str(l);
            text.push('\n');
        }
        CfgFile::from_str("", &text)
    }

    /// Line `i` becomes `text`; its line break stays.
    pub fn set(&mut self, i: usize, text: &str) {
        if let Some(l) = self.lines.get_mut(i) {
            l.0 = text.to_string();
        }
    }

    /// `text` as line `i` (at the end for `i` past it), with the file's line break.
    pub fn insert(&mut self, i: usize, text: &str) {
        let i = i.min(self.lines.len());
        if i == self.lines.len() {
            // a file that ends without a break goes on doing so
            if let Some(last) = self.lines.last_mut().filter(|l| l.1.is_empty()) {
                last.1 = self.eol;
                self.lines.push((text.to_string(), ""));
                return;
            }
        }
        self.lines.insert(i, (text.to_string(), self.eol));
    }

    pub fn push(&mut self, text: &str) {
        self.insert(self.lines.len(), text);
    }

    pub fn remove(&mut self, i: usize) {
        if i >= self.lines.len() {
            return;
        }
        let (_, eol) = self.lines.remove(i);
        if eol.is_empty() && i == self.lines.len() {
            if let Some(last) = self.lines.last_mut() {
                last.1 = "";
            }
        }
    }
}

/// What a change of an AI list cannot do.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    #[error("there is no such group")]
    NoGroup,
    #[error("there is no such vehicle in the group")]
    NoVehicle,
    #[error("a depot's group is not changed here")]
    Depot,
    #[error("the vehicles of an [aigroup] have no weights")]
    NoWeights,
    #[error("a weight is a number from 0 up")]
    BadWeight,
    #[error("the group ends before its depot line")]
    Truncated,
    #[error("a group needs a name")]
    NoName,
    #[error("there is a group of that name already")]
    Exists,
    #[error("the [ailist] header names this group or one after it as its default")]
    Default,
    #[error("the lists of a depot's vehicles that follow belong to this group")]
    HoldsFleet,
}

/// The kind of block a group is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupKind {
    /// `[aigroup_2]`: each vehicle with its weight.
    Weighted,
    /// `[aigroup]`: the vehicles alone.
    Plain,
    /// `[aigroup_depot]`, whose vehicles are in the fleet lists after it: not changed here.
    Depot,
}

impl GroupKind {
    pub fn has_weights(self) -> bool {
        self == GroupKind::Weighted
    }
}

/// A vehicle of a group and the line it is on.
#[derive(Debug, Clone, PartialEq)]
pub struct DocVehicle {
    pub file: String,
    pub weight: f32,
    line: usize,
    /// On the group's depot line (a map that leaves that line out).
    on_depot_line: bool,
}

/// A group of `ailists.cfg` and where it is in the file.
#[derive(Debug, Clone, PartialEq)]
pub struct DocGroup {
    pub name: String,
    pub kind: GroupKind,
    pub vehicles: Vec<DocVehicle>,
    header: usize,
    /// The depot line (the second after the keyword), when the file has it.
    depot_line: Option<usize>,
    /// The `[end]` line; none when the group runs to the end of the file.
    end: Option<usize>,
}

/// `ailists.cfg`, to change and write back.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AiListsDoc {
    text: TextLines,
    groups: Vec<DocGroup>,
    /// The `[ailist]` header's default group, when it names one.
    default_group: Option<usize>,
}

fn has_vehicle_ending(s: &str) -> bool {
    let l = s.to_ascii_lowercase();
    l.contains(".bus") || l.contains(".ovh") || l.contains(".sco")
}

/// A vehicle line: the file and its weight (1 without one).
fn vehicle_of(l: &str) -> (String, f32) {
    match l.split_once('\t') {
        Some((f, w)) => (f.trim().to_string(), omsi_cfg::parse_f32(w)),
        None => (l.trim().to_string(), 1.0),
    }
}

/// A weight as a list writes it: whole numbers without a point.
pub fn weight_text(w: f32) -> String {
    if w.fract() == 0.0 && w.abs() < 1e7 {
        format!("{}", w as i64)
    } else {
        format!("{w}")
    }
}

impl AiListsDoc {
    pub fn from_bytes(bytes: &[u8]) -> AiListsDoc {
        let mut d = AiListsDoc {
            text: TextLines::from_bytes(bytes),
            ..Default::default()
        };
        d.index();
        d
    }

    /// A map without the file: an empty one.
    pub fn new() -> AiListsDoc {
        AiListsDoc::default()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.text.to_bytes()
    }

    pub fn groups(&self) -> &[DocGroup] {
        &self.groups
    }

    /// The group of that name (any case), as an index into [`groups`](Self::groups).
    pub fn find(&self, name: &str) -> Option<usize> {
        self.groups
            .iter()
            .position(|g| g.name.trim().eq_ignore_ascii_case(name.trim()))
    }

    /// Where the groups and their vehicles are: the blocks read as `AiLists::parse` reads
    /// them, line by line.
    fn index(&mut self) {
        let f = self.text.cfg();
        let n = f.lines.len();
        let mut r = f.reader().with_rule(KeywordRule::AnyCase);
        let mut groups = Vec::new();
        let mut default_group = None;
        while let Some(k) = r.next_keyword() {
            let header = r.pos() - 1;
            match k.as_str() {
                "ailist" => {
                    let _ = r.str();
                    let default = r.str().trim().parse::<i32>().unwrap_or(-1);
                    let count = r.str().trim().parse::<usize>().unwrap_or(0).min(10_000);
                    for _ in 0..count {
                        let _ = r.str();
                    }
                    default_group = usize::try_from(default).ok();
                }
                "aigroup" | "aigroup_2" => {
                    let name = r.str().to_string();
                    let depot_at = r.pos();
                    let depot = r.str().trim().to_string();
                    let mut g = DocGroup {
                        name,
                        kind: if k == "aigroup_2" {
                            GroupKind::Weighted
                        } else {
                            GroupKind::Plain
                        },
                        vehicles: Vec::new(),
                        header,
                        depot_line: (depot_at < n).then_some(depot_at),
                        end: None,
                    };
                    if has_vehicle_ending(&depot) {
                        let (file, weight) = vehicle_of(&depot);
                        g.vehicles.push(DocVehicle {
                            file,
                            weight,
                            line: depot_at,
                            on_depot_line: true,
                        });
                    }
                    while r.pos() < n {
                        let at = r.pos();
                        let l = r.line();
                        if l.eq_ignore_ascii_case("[end]") {
                            g.end = Some(at);
                            break;
                        }
                        let l = l.trim_end();
                        if l.trim().is_empty() {
                            continue;
                        }
                        let (file, weight) = vehicle_of(l);
                        g.vehicles.push(DocVehicle {
                            file,
                            weight,
                            line: at,
                            on_depot_line: false,
                        });
                    }
                    groups.push(g);
                }
                "aigroup_depot" => {
                    let name = r.str().to_string();
                    let _ = r.str();
                    groups.push(DocGroup {
                        name,
                        kind: GroupKind::Depot,
                        vehicles: Vec::new(),
                        header,
                        depot_line: Some(header + 2).filter(|l| *l < n),
                        end: None,
                    });
                }
                "aigroup_depot_typgroup" | "aigroup_depot_typgroup_2" => {
                    let _ = r.str();
                    let _ = r.until("[end]");
                }
                _ => {}
            }
        }
        self.groups = groups;
        self.default_group = default_group;
    }

    fn group(&self, g: usize) -> Result<&DocGroup, EditError> {
        self.groups.get(g).ok_or(EditError::NoGroup)
    }

    fn editable(&self, g: usize) -> Result<&DocGroup, EditError> {
        let grp = self.group(g)?;
        if grp.kind == GroupKind::Depot {
            return Err(EditError::Depot);
        }
        if grp.depot_line.is_none() {
            return Err(EditError::Truncated);
        }
        Ok(grp)
    }

    /// The weight of vehicle `v` of group `g` becomes `w`.
    pub fn set_weight(&mut self, g: usize, v: usize, w: f32) -> Result<(), EditError> {
        if !w.is_finite() || w < 0.0 {
            return Err(EditError::BadWeight);
        }
        let grp = self.editable(g)?;
        if !grp.kind.has_weights() {
            return Err(EditError::NoWeights);
        }
        let line = grp.vehicles.get(v).ok_or(EditError::NoVehicle)?.line;
        let old = self.text.line(line).to_string();
        let file = match old.split_once('\t') {
            Some((f, _)) => f,
            None => old.trim_end(),
        };
        self.text.set(line, &format!("{file}\t{}", weight_text(w)));
        self.index();
        Ok(())
    }

    /// `file` added to group `g` after its last vehicle, with weight `w` (an `[aigroup]`
    /// writes none).
    pub fn add_vehicle(&mut self, g: usize, file: &str, w: f32) -> Result<(), EditError> {
        if !w.is_finite() || w < 0.0 {
            return Err(EditError::BadWeight);
        }
        let grp = self.editable(g)?;
        let at = match (grp.vehicles.last(), grp.end) {
            (Some(last), _) => last.line + 1,
            (None, Some(end)) => end,
            (None, None) => self.text.len(),
        };
        let text = if grp.kind.has_weights() {
            format!("{}\t{}", file.trim(), weight_text(w))
        } else {
            file.trim().to_string()
        };
        self.text.insert(at, &text);
        self.index();
        Ok(())
    }

    /// Vehicle `v` of group `g` taken out.
    pub fn remove_vehicle(&mut self, g: usize, v: usize) -> Result<(), EditError> {
        let grp = self.editable(g)?;
        let veh = grp.vehicles.get(v).ok_or(EditError::NoVehicle)?;
        if veh.on_depot_line {
            // (the line stays, empty: the next line would become the depot line)
            self.text.set(veh.line, "");
        } else {
            self.text.remove(veh.line);
        }
        self.index();
        Ok(())
    }

    /// Vehicle `v` of group `g` moved to the end of group `to`, with its weight.
    pub fn move_vehicle(&mut self, g: usize, v: usize, to: usize) -> Result<(), EditError> {
        if g == to {
            return Ok(());
        }
        self.editable(to)?;
        let veh = self
            .editable(g)?
            .vehicles
            .get(v)
            .ok_or(EditError::NoVehicle)?
            .clone();
        let before = self.text.clone();
        self.remove_vehicle(g, v)?;
        if let Err(e) = self.add_vehicle(to, &veh.file, veh.weight) {
            self.text = before;
            self.index();
            return Err(e);
        }
        Ok(())
    }

    /// A new, empty `[aigroup_2]` at the end of the file (where it moves no other group's
    /// number). Its index.
    pub fn add_group(&mut self, name: &str) -> Result<usize, EditError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(EditError::NoName);
        }
        if self.find(name).is_some() {
            return Err(EditError::Exists);
        }
        if !self.text.is_empty() && !self.text.line(self.text.len() - 1).trim().is_empty() {
            self.text.push("");
        }
        for l in ["[aigroup_2]", name, "", "[end]"] {
            self.text.push(l);
        }
        self.index();
        Ok(self.groups.len() - 1)
    }

    /// Group `g` taken out with its vehicles. Not when the `[ailist]` header's default
    /// group would become another one, nor when depot fleet lists follow it (they belong to
    /// the group before them).
    pub fn remove_group(&mut self, g: usize) -> Result<(), EditError> {
        let grp = self.editable(g)?;
        if self
            .default_group
            .is_some_and(|d| d >= g && d < self.groups.len())
        {
            return Err(EditError::Default);
        }
        let last = grp.end.unwrap_or(self.text.len().saturating_sub(1));
        // the next keyword after the group
        let next = (last + 1..self.text.len())
            .map(|i| self.text.line(i))
            .find_map(|l| omsi_cfg::keyword_with(l, KeywordRule::AnyCase));
        if next.is_some_and(|k| k.to_ascii_lowercase().starts_with("aigroup_depot_typgroup")) {
            return Err(EditError::HoldsFleet);
        }
        for i in (grp.header..=last).rev() {
            self.text.remove(i);
        }
        self.index();
        Ok(())
    }
}

/// A parking list (`parklist_p.txt`, `parklist_p_<n>.txt`): one `.sco` per line, a car
/// listed twice drawn twice as often. Other lines are left as they are.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParkListDoc {
    text: TextLines,
}

fn is_car(l: &str) -> bool {
    l.trim().to_ascii_lowercase().ends_with(".sco")
}

impl ParkListDoc {
    pub fn from_bytes(bytes: &[u8]) -> ParkListDoc {
        ParkListDoc {
            text: TextLines::from_bytes(bytes),
        }
    }

    pub fn new() -> ParkListDoc {
        ParkListDoc::default()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.text.to_bytes()
    }

    /// Each car once, in the list's order, with how many times it is listed.
    pub fn cars(&self) -> Vec<(String, usize)> {
        let mut out: Vec<(String, usize)> = Vec::new();
        for i in 0..self.text.len() {
            let l = self.text.line(i).trim();
            if !is_car(l) {
                continue;
            }
            match out.iter_mut().find(|(f, _)| f.eq_ignore_ascii_case(l)) {
                Some(c) => c.1 += 1,
                None => out.push((l.to_string(), 1)),
            }
        }
        out
    }

    fn lines_of(&self, file: &str) -> Vec<usize> {
        (0..self.text.len())
            .filter(|i| {
                let l = self.text.line(*i).trim();
                is_car(l) && l.eq_ignore_ascii_case(file.trim())
            })
            .collect()
    }

    /// `file` listed `n` times: the extra lines go after its last one (or after the last
    /// car), the ones too many come off from the end.
    pub fn set_count(&mut self, file: &str, n: usize) {
        let at = self.lines_of(file);
        if n < at.len() {
            for i in at[n..].iter().rev() {
                self.text.remove(*i);
            }
            return;
        }
        let (pos, text) = match at.last() {
            Some(last) => (*last + 1, self.text.line(*last).to_string()),
            None => {
                let after_cars = (0..self.text.len())
                    .rev()
                    .find(|i| is_car(self.text.line(*i)))
                    .map(|i| i + 1)
                    .unwrap_or(self.text.len());
                (after_cars, file.trim().to_string())
            }
        };
        for _ in at.len()..n {
            self.text.insert(pos, &text);
        }
    }

    pub fn add(&mut self, file: &str) {
        let n = self.lines_of(file).len();
        self.set_count(file, n + 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AiLists;

    const SAMPLE: &str = "Made-up AI lists for the tests\r\n\
        \r\n\
        [ailist]\r\n\
        x\r\n\
        -1\r\n\
        0\r\n\
        \r\n\
        ' the cars\r\n\
        [aigroup_2]\r\n\
        NormalCars\r\n\
        \r\n\
        Vehicles\\Cars\\a.ovh\t7\r\n\
        \r\n\
        Vehicles\\Cars\\b.ovh\t0.5\r\n\
        [END]\r\n\
        \r\n\
        [aigroup_depot]\r\n\
        Buses\r\n\
        Depot 1\r\n\
        \r\n\
        [aigroup_depot_typgroup_2]\r\n\
        Vehicles\\Bus\\bus.bus\r\n\
        1001\tB-1001\tred\r\n\
        [end]\r\n\
        \r\n\
        [aigroup]\r\n\
        Trucks\r\n\
        Vehicles\\Trucks\\t.ovh\r\n\
        Vehicles\\Trucks\\u.ovh\r\n\
        [end]";

    fn lists(d: &AiListsDoc) -> AiLists {
        AiLists::parse(&CfgFile::from_bytes("ailists.cfg", &d.to_bytes()))
    }

    /// The groups and vehicles the document sees are those the game's reading sees.
    fn same_as_the_parser(d: &AiListsDoc) {
        let a = lists(d);
        let named: Vec<_> = a.groups.iter().filter(|g| g.name != "NotInGroup").collect();
        assert_eq!(named.len(), d.groups().len());
        for (p, g) in named.iter().zip(d.groups()) {
            assert_eq!(p.name, g.name);
            let files: Vec<(&str, f32)> = p
                .vehicles
                .iter()
                .map(|v| (v.file.as_str(), v.weight))
                .collect();
            let ours: Vec<(&str, f32)> = g
                .vehicles
                .iter()
                .map(|v| (v.file.as_str(), v.weight))
                .collect();
            assert_eq!(files, ours, "{}", g.name);
        }
    }

    #[test]
    fn a_file_reads_back_byte_for_byte() {
        let samples: Vec<Vec<u8>> = vec![
            SAMPLE.as_bytes().to_vec(),
            format!("{SAMPLE}\r\n").into_bytes(),
            SAMPLE.replace("\r\n", "\n").into_bytes(),
            b"[aigroup_2]\rG\r\rGr\xfcnau.ovh\t2\r[end]\r".to_vec(),
            omsi_cfg::encode_utf16_bom("[aigroup_2]\r\nG\r\n\r\nGrünau.ovh\t2\r\n[end]\r\n"),
            Vec::new(),
        ];
        for s in samples {
            let d = AiListsDoc::from_bytes(&s);
            assert_eq!(d.to_bytes(), s);
            same_as_the_parser(&d);
        }
    }

    #[test]
    fn a_weight_changes_its_line_only() {
        let mut d = AiListsDoc::from_bytes(SAMPLE.as_bytes());
        let g = d.find("normalcars").unwrap();
        d.set_weight(g, 1, 3.0).unwrap();
        assert_eq!(
            String::from_utf8(d.to_bytes()).unwrap(),
            SAMPLE.replace("b.ovh\t0.5", "b.ovh\t3")
        );
        d.set_weight(g, 0, 2.25).unwrap();
        assert!(String::from_utf8(d.to_bytes())
            .unwrap()
            .contains("a.ovh\t2.25\r\n"));
        same_as_the_parser(&d);
        // the trucks' [aigroup] has no weights; the depot is not changed here
        let t = d.find("Trucks").unwrap();
        assert_eq!(d.set_weight(t, 0, 2.0), Err(EditError::NoWeights));
        assert_eq!(d.set_weight(g, 0, f32::NAN), Err(EditError::BadWeight));
        assert_eq!(d.set_weight(g, 0, -1.0), Err(EditError::BadWeight));
        assert_eq!(
            d.add_vehicle(d.find("Buses").unwrap(), "x.bus", 1.0),
            Err(EditError::Depot)
        );
    }

    #[test]
    fn vehicles_come_and_go_and_move() {
        let mut d = AiListsDoc::from_bytes(SAMPLE.as_bytes());
        let (cars, trucks) = (d.find("NormalCars").unwrap(), d.find("Trucks").unwrap());
        d.add_vehicle(cars, "Vehicles\\Cars\\c.ovh", 4.0).unwrap();
        let text = String::from_utf8(d.to_bytes()).unwrap();
        assert!(
            text.contains("b.ovh\t0.5\r\nVehicles\\Cars\\c.ovh\t4\r\n[END]"),
            "{text}"
        );
        // the last line had no break, and still has none
        d.add_vehicle(trucks, "Vehicles\\Trucks\\v.ovh", 9.0)
            .unwrap();
        let text = String::from_utf8(d.to_bytes()).unwrap();
        assert!(
            text.ends_with("u.ovh\r\nVehicles\\Trucks\\v.ovh\r\n[end]"),
            "{text}"
        );
        same_as_the_parser(&d);

        d.move_vehicle(cars, 0, trucks).unwrap();
        let a = lists(&d);
        let files = |n: &str| -> Vec<String> {
            a.groups
                .iter()
                .find(|g| g.name == n)
                .unwrap()
                .vehicles
                .iter()
                .map(|v| v.file.clone())
                .collect()
        };
        assert_eq!(
            files("NormalCars"),
            ["Vehicles\\Cars\\b.ovh", "Vehicles\\Cars\\c.ovh"]
        );
        assert_eq!(files("Trucks").last().unwrap(), "Vehicles\\Cars\\a.ovh");
        same_as_the_parser(&d);

        // everything taken out again: the file as it was, but for the moved line
        let t = d.find("Trucks").unwrap();
        let n = d.groups()[t].vehicles.len();
        d.remove_vehicle(t, n - 1).unwrap();
        d.remove_vehicle(t, n - 2).unwrap();
        let c = d.find("NormalCars").unwrap();
        d.remove_vehicle(c, 1).unwrap();
        let text = String::from_utf8(d.to_bytes()).unwrap();
        assert_eq!(
            text,
            SAMPLE.replace("Vehicles\\Cars\\a.ovh\t7\r\n\r\n", "\r\n")
        );
    }

    /// A map that leaves the depot line out has its first vehicle there: taking it out
    /// leaves the line, empty, so the next vehicle does not become the depot.
    #[test]
    fn the_vehicle_on_the_depot_line() {
        let text =
            "[aigroup_2]\r\nTrucks\r\nVehicles\\T\\t.ovh\t2\r\nVehicles\\T\\u.ovh\t1\r\n[end]\r\n";
        let mut d = AiListsDoc::from_bytes(text.as_bytes());
        assert_eq!(d.groups()[0].vehicles.len(), 2);
        d.set_weight(0, 0, 5.0).unwrap();
        assert!(String::from_utf8(d.to_bytes())
            .unwrap()
            .contains("Trucks\r\nVehicles\\T\\t.ovh\t5\r\n"));
        d.remove_vehicle(0, 0).unwrap();
        d.remove_vehicle(0, 0).unwrap();
        assert_eq!(
            String::from_utf8(d.to_bytes()).unwrap(),
            "[aigroup_2]\r\nTrucks\r\n\r\n[end]\r\n"
        );
        same_as_the_parser(&d);
        assert!(lists(&d).groups[0].hof.is_none());
    }

    #[test]
    fn groups_are_added_at_the_end_and_taken_out_whole() {
        let mut d = AiListsDoc::from_bytes(SAMPLE.as_bytes());
        assert_eq!(d.add_group(" "), Err(EditError::NoName));
        assert_eq!(d.add_group("normalcars"), Err(EditError::Exists));
        let g = d.add_group("Taxis").unwrap();
        d.add_vehicle(g, "Vehicles\\Taxi\\taxi.ovh", 2.0).unwrap();
        let text = String::from_utf8(d.to_bytes()).unwrap();
        assert!(
            text.ends_with(
                "[end]\r\n\r\n[aigroup_2]\r\nTaxis\r\n\r\nVehicles\\Taxi\\taxi.ovh\t2\r\n[end]"
            ),
            "{text}"
        );
        same_as_the_parser(&d);
        d.remove_group(g).unwrap();
        assert_eq!(
            String::from_utf8(d.to_bytes()).unwrap(),
            format!("{SAMPLE}\r\n")
        );
        // a group before depot fleet lists holds them
        let fleet = "[aigroup_2]\r\nA\r\n\r\n[end]\r\n[aigroup_depot_typgroup_2]\r\nb.bus\r\n1\r\n[end]\r\n";
        let mut d = AiListsDoc::from_bytes(fleet.as_bytes());
        assert_eq!(d.remove_group(0), Err(EditError::HoldsFleet));
        // nor the default group the header names, or one before it
        let header = format!(
            "[ailist]\r\nx\r\n1\r\n0\r\n{}",
            &SAMPLE[SAMPLE.find("[aigroup_2]").unwrap()..]
        );
        let mut d = AiListsDoc::from_bytes(header.as_bytes());
        assert_eq!(d.remove_group(0), Err(EditError::Default));
        assert_eq!(d.remove_group(1), Err(EditError::Depot));
        d.remove_group(2).unwrap();
    }

    #[test]
    fn a_new_file_is_written_as_omsi_writes_its_own() {
        let mut d = AiListsDoc::new();
        let g = d.add_group("Cars").unwrap();
        d.add_vehicle(g, "Vehicles\\Grünau\\a.ovh", 1.0).unwrap();
        assert_eq!(
            d.to_bytes(),
            b"[aigroup_2]\r\nCars\r\n\r\nVehicles\\Gr\xfcnau\\a.ovh\t1\r\n[end]\r\n"
        );
    }

    #[test]
    fn parking_lists_keep_their_other_lines() {
        let text = "' parked cars\r\nVehicles\\P\\a.sco\r\nVehicles\\P\\b.sco\r\nVehicles\\P\\a.sco\r\n\r\nnot a car\r\n";
        let mut p = ParkListDoc::from_bytes(text.as_bytes());
        assert_eq!(p.to_bytes(), text.as_bytes());
        assert_eq!(
            p.cars(),
            [
                ("Vehicles\\P\\a.sco".to_string(), 2),
                ("Vehicles\\P\\b.sco".to_string(), 1)
            ]
        );
        p.set_count("vehicles\\p\\A.sco", 3);
        p.set_count("Vehicles\\P\\b.sco", 0);
        p.add("Vehicles\\P\\c.sco");
        assert_eq!(
            String::from_utf8(p.to_bytes()).unwrap(),
            "' parked cars\r\nVehicles\\P\\a.sco\r\nVehicles\\P\\a.sco\r\nVehicles\\P\\a.sco\r\nVehicles\\P\\c.sco\r\n\r\nnot a car\r\n"
        );
        p.set_count("Vehicles\\P\\a.sco", 1);
        assert_eq!(p.cars()[0], ("Vehicles\\P\\a.sco".to_string(), 1));
        let mut n = ParkListDoc::new();
        n.add("Vehicles\\P\\a.sco");
        n.add("Vehicles\\P\\a.sco");
        assert_eq!(
            n.to_bytes(),
            b"Vehicles\\P\\a.sco\r\nVehicles\\P\\a.sco\r\n"
        );
    }

    /// Every map of an installation, when there is one: its lists read back byte for byte,
    /// and its groups as the game reads them.
    #[test]
    fn the_installed_maps_read_back() {
        let root = std::env::var_os("OMSI_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("../../../OMSI 2 Original"));
        let Ok(maps) = std::fs::read_dir(root.join("maps")) else {
            eprintln!("skipped: no installation");
            return;
        };
        for map in maps.flatten() {
            for f in std::fs::read_dir(map.path())
                .into_iter()
                .flatten()
                .flatten()
            {
                let name = f.file_name().to_string_lossy().to_ascii_lowercase();
                let Ok(bytes) = std::fs::read(f.path()) else {
                    continue;
                };
                if name == "ailists.cfg" {
                    let d = AiListsDoc::from_bytes(&bytes);
                    assert_eq!(d.to_bytes(), bytes, "{}", f.path().display());
                    same_as_the_parser(&d);
                } else if name.starts_with("parklist_p") {
                    let p = ParkListDoc::from_bytes(&bytes);
                    assert_eq!(p.to_bytes(), bytes, "{}", f.path().display());
                }
            }
        }
    }
}
