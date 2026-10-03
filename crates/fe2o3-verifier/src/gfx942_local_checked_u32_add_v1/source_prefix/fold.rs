//! Shared origin transition; source and KIR adapters construct independent folds.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Origin {
    Uninitialized,
    Argument(usize),
    Constant(u32),
}

#[derive(Clone, Copy, Debug)]
pub(super) enum PrefixInput {
    Cell(usize),
    Constant(u32),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PrefixStep {
    pub(super) destination: usize,
    pub(super) input: PrefixInput,
}

macro_rules! ordinary_exec {
    ($body:expr) => {
        $body
    };
}

include!("fold_body.rs");

pub(super) fn fold(state: &mut [Origin], steps: &[PrefixStep]) -> bool {
    checked_u32_prefix_fold_body_v1!(ordinary_exec, state, steps, index, [])
}
