//! OMSI 2's tutorials (`Tutorials/<n>`): four lessons, each a situation (OMSI
//! TTutorialMan: Strg.osn, Fast.osn, LIN.osn, SPEZ.osn) and a run of pages - `<step>.html`
//! in the language's folder with `<step>.jpg` beside them, stepped through in the order of
//! their numbers. The original moves on by itself when the step's action is done (the key
//! pressed, the switch thrown: TTutorialMan.virtual_00 checks them); here Enter or Page
//! Down moves on and Page Up back.
use std::path::PathBuf;

pub const SITUATIONS: [&str; 4] = [
    "Tutorials/STRG.osn",
    "Tutorials/FAST.osn",
    "Tutorials/LIN.osn",
    "Tutorials/SPEZ.osn",
];

pub struct Page {
    pub title: String,
    pub text: String,
    pub image: Option<PathBuf>,
}

pub struct Tutorial {
    pub pages: Vec<Page>,
    pub at: usize,
    pub hidden: bool,
}

/// The text of a tutorial page: the heading and the paragraphs, without the markup.
pub fn page_text(html: &str) -> (String, String) {
    let body = html.split_once("</style>").map(|x| x.1).unwrap_or(html);
    let title = body
        .split_once("<h2>")
        .and_then(|(_, r)| r.split_once("</h2>"))
        .map(|(t, _)| strip(t))
        .unwrap_or_default();
    let rest = body.split_once("</h2>").map(|x| x.1).unwrap_or(body);
    let text = strip(
        &rest
            .replace("</p>", "\n")
            .replace("<br>", "\n")
            .replace("<li>", "\n• "),
    );
    let lines: Vec<String> = text
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect();
    (title, lines.join("\n"))
}

fn strip(s: &str) -> String {
    let mut out = String::new();
    let mut tag = false;
    for c in s.chars() {
        match c {
            '<' => tag = true,
            '>' => tag = false,
            _ if !tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&quot;", "\"")
        .replace("&amp;", "&")
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

impl Tutorial {
    pub fn next(&mut self) {
        self.at = (self.at + 1).min(self.pages.len().saturating_sub(1));
    }

    pub fn back(&mut self) {
        self.at = self.at.saturating_sub(1);
    }

    pub fn page(&self) -> Option<&Page> {
        self.pages.get(self.at)
    }
}
