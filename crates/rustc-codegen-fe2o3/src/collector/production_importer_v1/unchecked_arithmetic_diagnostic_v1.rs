//! Bounded diagnostics from the exact preflight producers; no admission changes.

use super::ProductionSemanticPreflightPlanV1;
use fe2o3_mir_model::semantic_mir_v1::{SemanticMirErrorV1, SemanticMirLocationV1};
use rustc_middle::ty::TyCtxt;
use std::fmt::{self, Write};

const MAX_BYTES: usize = 16_384;
const MAX_BLOCKS: usize = 4;
const MAX_STATEMENTS: usize = 8;
const MAX_EDGES: usize = 4_096;
const MAX_CALLEES: usize = 4;

pub(super) fn report(
    tcx: TyCtxt<'_>,
    plan: &ProductionSemanticPreflightPlanV1<'_>,
    error: &SemanticMirErrorV1,
) {
    let SemanticMirErrorV1::UnprovenUncheckedArithmetic {
        operation,
        location,
    } = error
    else {
        return;
    };
    let function = match location {
        SemanticMirLocationV1::Statement { function, .. }
        | SemanticMirLocationV1::Function(function) => *function,
        _ => return,
    };
    let Some(producer) = plan.function_producers().get(function.index() as usize) else {
        return;
    };
    let mut output = BoundedText::default();
    let _ = writeln!(
        output,
        "fe2o3 unchecked-arithmetic source: operation={operation:?} location={location:?} identity={:?} path={} instance={:?}",
        producer.identities.function(),
        tcx.def_path_str(producer.instance.def_id()),
        producer.instance
    );
    if let SemanticMirLocationV1::Statement {
        block, statement, ..
    } = location
        && let Some(body) = plan.body_producers().get(function.index() as usize)
        && let Some(mapped) = body.blocks.get(block.index() as usize)
    {
        let _ = writeln!(
            output,
            "semantic bb{} statement{} maps into retained rustc {:?} (normalization may expand statements)",
            block.index(),
            statement,
            mapped.rustc_block
        );
    }
    if let Some(body) = plan.function_mir(function) {
        append_body(&mut output, body);
    }
    for call in plan
        .direct_call_producers()
        .iter()
        .take(MAX_EDGES)
        .filter(|call| call.caller == function)
        .take(MAX_CALLEES)
    {
        let Some(callee) = plan.function_producers().get(call.callee.index() as usize) else {
            continue;
        };
        let _ = writeln!(
            output,
            "retained direct edge: bb{} callee={} identity={:?} path={} instance={:?}",
            call.block,
            call.callee.index(),
            callee.identities.function(),
            tcx.def_path_str(callee.instance.def_id()),
            callee.instance
        );
        if let Some(body) = plan.function_mir(call.callee) {
            append_body(&mut output, body);
        }
    }
    eprintln!("{}", output.text);
}

fn append_body(output: &mut BoundedText, body: &rustc_middle::mir::Body<'_>) {
    for (block, data) in body.basic_blocks.iter_enumerated().take(MAX_BLOCKS) {
        let _ = writeln!(output, "  {block:?}:");
        for (index, statement) in data.statements.iter().take(MAX_STATEMENTS).enumerate() {
            let _ = writeln!(output, "    stmt{index}: {:?}", statement.kind);
        }
        let _ = writeln!(output, "    terminator: {:?}", data.terminator().kind);
    }
}

#[derive(Default)]
struct BoundedText {
    text: String,
}

impl Write for BoundedText {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let remaining = MAX_BYTES.saturating_sub(self.text.len());
        let mut end = value.len().min(remaining);
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        self.text.push_str(&value[..end]);
        if end == value.len() {
            Ok(())
        } else {
            Err(fmt::Error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arithmetic_source_diagnostic_has_a_total_byte_bound() {
        let mut output = BoundedText::default();
        assert!(output.write_str(&"x".repeat(MAX_BYTES - 1)).is_ok());
        assert!(output.write_str("\u{03bb}").is_err());
        assert_eq!(output.text.len(), MAX_BYTES - 1);
        assert!(output.write_str("xy").is_err());
        assert_eq!(output.text.len(), MAX_BYTES);
        assert!(output.write_str("z").is_err());
        assert_eq!(output.text.len(), MAX_BYTES);
    }
}
