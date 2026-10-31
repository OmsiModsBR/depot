//! What the pages read from the rest of the crate.

pub(crate) use crate::host::Host;
pub(crate) use crate::pages::Page;
pub(crate) use crate::state::{
    available, fmt_bytes, mobile, root_problem, short_map, Flow, Squeeze, Squeezed, State,
};
pub(crate) use crate::{kit, shell};
pub(crate) use omsi_launcher_lib as core;
pub(crate) use omsi_ui::tr;
pub(crate) use omsi_ui::widgets::Ui;
