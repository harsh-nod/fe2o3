//! Read the retained typed AST directly; no caller-supplied normalization flags.

use super::*;
use fe2o3_mir_model::semantic_mir_v1::{SemanticLocalDeclV1, SemanticTypeDeclV1};

macro_rules! ordinary_exec {
    ($body:expr) => {
        $body
    };
}

include!("normalize_body.rs");

pub(super) fn is_u32(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    checked_u32_prefix_type_body_v1!(ordinary_exec, types, ty)
}

pub(super) fn scalar_local(
    types: &[SemanticTypeDeclV1],
    locals: &[SemanticLocalDeclV1],
    place: &SemanticPlaceV1,
) -> Result<usize, CheckedU32PrefixErrorV1> {
    checked_u32_prefix_local_body_v1!(ordinary_exec, types, locals, place)
}

pub(super) fn scalar_constant(
    types: &[SemanticTypeDeclV1],
    operand: &SemanticOperandV1,
) -> Option<u32> {
    checked_u32_prefix_constant_body_v1!(ordinary_exec, types, operand)
}

pub(super) fn source_step(
    types: &[SemanticTypeDeclV1],
    locals: &[SemanticLocalDeclV1],
    statement: &SemanticStatementV1,
    operations: u32,
) -> Result<Option<PrefixStep>, CheckedU32PrefixErrorV1> {
    checked_u32_prefix_source_step_body_v1!(ordinary_exec, types, locals, statement, operations)
}
