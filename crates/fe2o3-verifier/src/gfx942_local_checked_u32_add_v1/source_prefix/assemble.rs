//! Assemble from original retained spans; the ordinal index is private scratch.

use super::*;
use fe2o3_kernel_ir::BlockId;
use normalize::source_step;

macro_rules! ordinary_exec {
    ($body:expr) => {
        $body
    };
}

include!("assemble_body.rs");

pub(super) fn source_prefix(
    source: &AdmittedSemanticMirV1,
    function: &SemanticFunctionDeclV1,
    prefix: &[SemanticStatementV1],
    spans: &[SemanticKirStatementOperationSpanV1],
    capture: ProductionCheckedU32AddCaptureV1<'_>,
    entry: BlockId,
) -> Result<(Vec<PrefixStep>, u32), CheckedU32PrefixErrorV1> {
    let types = source.types();
    let locals = function.locals();
    let root = capture.request().root().index();
    let function_index = capture.request().function().index();
    let block = capture.request().block().index();
    let kernel_block = entry.0;
    let operation = capture.operation();
    checked_u32_prefix_assemble_body_v1!(
        ordinary_exec,
        types,
        locals,
        prefix,
        spans,
        root,
        function_index,
        block,
        kernel_block,
        operation,
        selected,
        index,
        [],
        [],
        steps,
        next_operation,
        ordinal,
        [],
        []
    )
}
