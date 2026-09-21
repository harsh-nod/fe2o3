#[path = "../common/canonical_wire_observation_v1.rs"]
mod common;
pub use common::{STORAGE, WORK, decode, dump, hex, operation, site};

#[derive(Clone, Copy)]
pub struct Case {
    pub name: &'static str,
    pub wire: &'static str,
    pub golden: &'static str,
    pub splits: usize,
    pub forwards: usize,
}
macro_rules! case {
    ($name:literal, $splits:literal, $forwards:literal) => {
        Case {
            name: $name,
            wire: include_str!(concat!($name, ".hex")),
            golden: include_str!(concat!($name, ".golden")),
            splits: $splits,
            forwards: $forwards,
        }
    };
}
pub const CASES: [Case; 13] = [
    case!("dynamic-loop", 1, 1),
    case!("diamond", 1, 1),
    case!("duplicate-edge", 1, 1),
    case!("two-functions", 2, 2),
    case!("ungrounded-phi", 0, 0),
    case!("step-two", 0, 1),
    case!("unchecked-add", 0, 1),
    case!("global-clobber", 1, 0),
    case!("trap-cut", 1, 0),
    case!("volatile", 1, 0),
    case!("alignment", 1, 0),
    case!("noop", 0, 0),
    case!("different-stores", 1, 0),
];
