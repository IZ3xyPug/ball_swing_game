// ── objects/mod.rs ────────────────────────────────────────────────────────────
// One file per object type.  Everything is re-exported so all existing
// `use crate::objects::*` and `use crate::objects::foo` call-sites continue
// to compile without any changes.

mod hooks;
mod pads;
mod spinners;
mod coins;
mod pickups;
mod gates;
mod gravity_wells;
mod turrets;
mod ui;
mod math;
mod rocket_pads;
mod planets;
mod black_holes;

pub use hooks::*;
pub use pads::*;
pub use spinners::*;
pub use coins::*;
pub use pickups::*;
pub use gates::*;
pub use gravity_wells::*;
pub use turrets::*;
pub use ui::*;
pub use math::*;
pub use rocket_pads::*;
pub use planets::*;
pub use black_holes::*;
