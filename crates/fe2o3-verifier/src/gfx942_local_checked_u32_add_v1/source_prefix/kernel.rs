//! Actual retained KIR prefix evaluation; not machine or launch authority.

use super::*;
use fe2o3_kernel_ir::Operation;
use std::collections::BTreeMap;

include!("kernel_body.rs");

macro_rules! ordinary_exec {
    ($body:expr) => {
        $body
    };
}

fn constant_binding(operation: &Operation) -> Option<(u32, u32)> {
    checked_u32_prefix_kernel_constant_body_v1!(ordinary_exec, operation)
}

fn terminal_origin(
    terminal: &Operation,
    previous: &Operation,
    origins: &BTreeMap<u32, Origin>,
    operand: u32,
    value: u32,
    overflow: u32,
    literal: u32,
) -> Result<Origin, CheckedU32PrefixErrorV1> {
    checked_u32_prefix_kernel_terminal_body_v1!(
        ordinary_exec,
        terminal,
        previous,
        origins,
        operand,
        value,
        overflow,
        literal
    )
}

pub(super) fn kernel_prefix(
    arguments: &[CheckedU32PrefixArgumentV1],
    operations: &[Operation],
    capture: ProductionCheckedU32AddCaptureV1<'_>,
) -> Result<Origin, CheckedU32PrefixErrorV1> {
    let operand = capture.operand().0;
    let value = capture.value().0;
    let overflow = capture.overflow().0;
    let literal = capture.literal();
    checked_u32_prefix_kernel_assemble_body_v1!(
        ordinary_exec,
        arguments,
        operations,
        operand,
        value,
        overflow,
        literal,
        origins,
        argument,
        [],
        [],
        index,
        [],
        []
    )
}

#[cfg(test)]
#[path = "tests/kernel.rs"]
mod tests;
