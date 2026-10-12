//! What a page and the game exchange: the values of `window.omsi`, the requests a page
//! makes, the pointer, and the interface every page backend implements.

use std::path::PathBuf;

/// A value of the snapshot tree.
#[derive(Clone, Debug, PartialEq)]
pub enum ApiValue {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    List(Vec<ApiValue>),
    /// Properties in the order they are listed in.
    Map(Vec<(String, ApiValue)>),
}

impl ApiValue {
    /// A property of a [`ApiValue::Map`].
    pub fn get(&self, key: &str) -> Option<&ApiValue> {
        match self {
            ApiValue::Map(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// The same map with property `key` set to `value`.
    pub(crate) fn with(mut self, key: &str, value: ApiValue) -> ApiValue {
        if let ApiValue::Map(m) = &mut self {
            match m.iter_mut().find(|(k, _)| k == key) {
                Some(slot) => slot.1 = value,
                None => m.push((key.to_string(), value)),
            }
        }
        self
    }
}

/// What the pointer (a finger, the mouse) does on the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerKind {
    Down,
    Up,
    Move,
}

/// Something a page asks of the vehicle beyond a variable or a trigger: the IBIS duty.
/// The game applies it (`VehicleInstance::take_html_requests`), since only it knows the
/// driver's IBIS keys.
#[derive(Clone, Debug, PartialEq)]
pub enum HtmlRequest {
    /// `omsi.setRoute(index)`: line, route and destination of `omsi.depot.routes[index]`.
    SetRoute(usize),
    /// `omsi.setLine(text)`: the first route of that line in the depot file.
    SetLine(String),
    /// `omsi.setDestination(index)`: the destination sign, `omsi.depot.destinations[index]`.
    SetDestination(usize),
    /// `omsi.clearLine()`: the IBIS shows no line (before a route is started).
    ClearLine,
    /// `omsi.setNextStop(index)`: the duty goes on with stop `index` of its trip (stops
    /// before it are skipped; an earlier stop makes the stops from there on due again).
    SetNextStop(usize),
}

/// What `window.omsi` offers a page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageApi {
    /// A vehicle's page: the vehicle (`omsi.vehicle`), the depot and the IBIS requests
    /// (`omsi.setRoute` ...), besides the basic API.
    Vehicle,
    /// A scenery object's page: only the basic API - `omsi.setVar`, `.trigger`, `.getVar`,
    /// `.vars`, `.time`, `.date` and `.locale`.
    Scenery,
}

pub trait HtmlRenderer: Send {
    fn set_vars(&mut self, num: &[(String, f32)], strs: &[(String, String)]);
    fn poll_frame(&mut self) -> Option<Vec<u8>>;
    fn take_events(&mut self) -> Vec<(String, f32)>;
    /// Triggers the page pressed (`omsi.trigger(name)`) since the last call.
    fn take_triggers(&mut self) -> Vec<String> {
        Vec::new()
    }
    /// The pointer is at (`x`, `y`) in texture pixels. A backend that cannot be operated
    /// leaves this as it is.
    fn pointer(&mut self, _x: f32, _y: f32, _kind: PointerKind) {}
    /// The normalised state of the vehicle (`window.omsi.vehicle`, see
    /// [`crate::vehicle_api`]). Called before [`Self::set_vars`] whenever it changed. A
    /// backend that has no such object leaves this as it is.
    fn set_vehicle(&mut self, _api: &ApiValue) {}
    /// `omsi.time`, `omsi.date` and `omsi.locale` (see [`crate::vehicle_api::environment`]): a
    /// map whose entries become properties of `window.omsi`. Called before [`Self::set_vars`]
    /// whenever it changed.
    fn set_env(&mut self, _env: &ApiValue) {}
    /// The depot file as a page sees it (`window.omsi.depot`, see [`crate::vehicle_api::depot`]).
    /// Called once, before the first update.
    fn set_depot(&mut self, _depot: &ApiValue) {}
    /// The folders a page's pictures (`<img src>`, `url(...)`) are looked up in, in order.
    /// Called once, right after the page is created. A backend without pictures leaves it.
    fn set_asset_dirs(&mut self, _dirs: Vec<PathBuf>) {}
    /// Route, line and destination requests the page made since the last call.
    fn take_requests(&mut self) -> Vec<HtmlRequest> {
        Vec::new()
    }
    /// The departures of the stops the page asked for (`window.omsi.departures`, see
    /// [`crate::vehicle_api::departures`]). Called before [`Self::set_vars`] whenever it changed.
    fn set_departures(&mut self, _departures: &ApiValue) {}
    /// The stops the page asked departures for (`omsi.getDepartures(stop)`) since the last call,
    /// as keys: trimmed, lower case.
    fn take_departure_wants(&mut self) -> Vec<String> {
        Vec::new()
    }
}

/// A style sheet is pasted into the page, so its `url(...)` paths, which are relative to the
/// sheet, get the sheet's folder (`prefix`) put in front: they then read relative to the page.
pub(crate) fn rebase_css_urls(css: &str, prefix: &str) -> String {
    if prefix.is_empty() {
        return css.to_string();
    }
    let lower = css.to_ascii_lowercase();
    let mut out = String::with_capacity(css.len() + 64);
    let mut i = 0;
    while let Some(p) = lower[i..].find("url(") {
        let inner = i + p + 4;
        let Some(e) = css[inner..].find(')').map(|e| inner + e) else { break };
        out.push_str(&css[i..inner]);
        let raw = css[inner..e].trim();
        let (quote, path) = match raw.chars().next() {
            Some(c @ ('"' | '\'')) => (c.to_string(), raw.trim_matches(c)),
            _ => (String::new(), raw),
        };
        let absolute = path.is_empty() || path.starts_with(['/', '\\', '#']) || path.starts_with("data:") || path.contains("://");
        if absolute {
            out.push_str(&css[inner..e]);
        } else {
            out.push_str(&quote);
            out.push_str(prefix);
            out.push_str(path);
            out.push_str(&quote);
        }
        i = e;
    }
    out.push_str(&css[i..]);
    out
}
