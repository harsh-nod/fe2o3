//! Error-only source observations. No definition, receipt, or authority is created.

use super::*;
use fe2o3_mir_model::semantic_mir_v1::{SemanticConstantValueV1, SemanticTypeShapeV1};
use std::fmt::Write as _;

const MAX_BYTES: usize = 3072;
const MAX_TRANSFER_SEARCH: usize = 256;

pub(super) fn annotate(
    semantic: &AdmittedInertSemanticMirV1,
    view: &SemanticExpandedRootV1,
    mut error: ProductionSemanticSsaErrorV1,
) -> ProductionSemanticSsaErrorV1 {
    let detail = describe(semantic, view, &error);
    if let ProductionSemanticSsaErrorV1::ExpandedExecution {
        return_transfer_diagnostic,
        ..
    } = &mut error
    {
        *return_transfer_diagnostic = detail;
    }
    error
}

fn describe(
    semantic: &AdmittedInertSemanticMirV1,
    view: &SemanticExpandedRootV1,
    error: &ProductionSemanticSsaErrorV1,
) -> Option<String> {
    let ProductionSemanticSsaErrorV1::ExpandedExecution {
        root,
        execution_view_identity,
        source_block: Some((instance, function, source_block)),
        source_statement: Some(SemanticExpandedStatementOriginV1::ReturnTransfer { callee }),
        source_local: Some(origin),
        error,
        ..
    } = error
    else {
        return None;
    };
    let ProductionSemanticSsaErrorV1::Planner {
        function: execution_function,
        error: SsaPlannerErrorV1::UndefinedAtUse {
            block, variable, ..
        },
    } = error.as_ref()
    else {
        return None;
    };
    if *root != view.root()
        || execution_view_identity != view.identity()
        || *execution_function != view.source_body()
        || instance != callee
        || origin.instance() != *callee
        || origin.function() != *function
        || view.local_origins().get(variable.get() as usize) != Some(origin)
    {
        return None;
    }
    let frame = view.instances().get(callee.index() as usize)?;
    let source = semantic.functions().get(function.index() as usize)?;
    let local = source.locals().get(origin.local().index() as usize)?;
    if frame.function() != *function
        || frame.function_identity() != source.identity()
        || local.role() != SemanticLocalRoleV1::Return
    {
        return None;
    }
    let expanded_block = view.body().blocks().get(block.get() as usize)?;
    let block_origin = view.block_origins().get(block.get() as usize)?;
    if block_origin.instance() != *callee
        || block_origin.function() != *function
        || block_origin.block() != *source_block
    {
        return None;
    }
    let (statement, assignment) = expanded_block
        .statements()
        .iter()
        .zip(block_origin.statements())
        .take(MAX_TRANSFER_SEARCH)
        .enumerate()
        .find_map(|(index, (statement, marker))| {
            if marker != &(SemanticExpandedStatementOriginV1::ReturnTransfer { callee: *callee }) {
                return None;
            }
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                return None;
            };
            let SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place)) =
                assignment.value().kind()
            else {
                return None;
            };
            (place.local().index() == variable.get()
                && place.projections().is_empty()
                && place.ty() == local.ty())
            .then_some((index, assignment))
        })?;

    let mut out = Bounded::default();
    let _ = write!(out, "function={} identity=", function.index());
    for byte in source.identity().as_bytes() {
        let _ = write!(out, "{byte:02x}");
    }
    let _ = write!(
        out,
        " local={} role={:?} source_output={} return_mode={} entry={} blocks={}; ",
        origin.local().index(),
        local.role(),
        source.abi().source_output_type().index(),
        pass_mode(source.abi().return_value().mode()),
        source.entry().index(),
        source.blocks().len()
    );
    write_type(&mut out, semantic, local.ty());
    if let Some(ty) = semantic.types().get(local.ty().index() as usize) {
        match ty.shape() {
            SemanticTypeShapeV1::Tuple(fields)
            | SemanticTypeShapeV1::Aggregate(fields)
            | SemanticTypeShapeV1::Union(fields) => {
                for field in fields.fields().iter().take(4) {
                    let _ = write!(out, "; field ");
                    write_type(&mut out, semantic, *field);
                }
            }
            _ => {}
        }
    }
    let _ = write!(
        out,
        "; transfer=bb{}s{} Move(local{}) -> local{} type{} dst_origin={:?}; expanded_exit=",
        block.get(),
        statement,
        variable.get(),
        assignment.destination().local().index(),
        assignment.destination().ty().index(),
        view.local_origins()
            .get(assignment.destination().local().index() as usize)
    );
    write_terminator(&mut out, expanded_block.terminator().kind());
    if let (Some(parent), Some(call_block)) = (frame.parent(), frame.call_block()) {
        if let Some(parent_frame) = view.instances().get(parent.index() as usize) {
            let _ = write!(
                out,
                "; caller=instance{} function{} bb{} ",
                parent.index(),
                parent_frame.function().index(),
                call_block.index()
            );
            if let Some(block) = semantic
                .functions()
                .get(parent_frame.function().index() as usize)
                .and_then(|function| function.blocks().get(call_block.index() as usize))
            {
                write_terminator(&mut out, block.terminator().kind());
            }
        }
    }
    // Prioritize the failing block and entry; never scan an entire large body.
    let mut selected = [None; 4];
    let mut count = 0;
    for index in [
        source_block.index() as usize,
        source.entry().index() as usize,
        0,
        1,
    ] {
        if index < source.blocks().len() && !selected.contains(&Some(index)) {
            selected[count] = Some(index);
            count += 1;
        }
    }
    for index in selected.into_iter().flatten() {
        let block = &source.blocks()[index];
        let _ = write!(
            out,
            "; source_bb{index} statements={} [",
            block.statements().len()
        );
        for (statement, operation) in block.statements().iter().take(4).enumerate() {
            let _ = write!(out, " s{statement}:");
            write_statement(&mut out, operation.kind());
        }
        if block.statements().len() > 4 {
            let _ = write!(out, " ...[statements truncated]");
        }
        let _ = write!(out, " ] ");
        write_terminator(&mut out, block.terminator().kind());
    }
    if source.blocks().len() > count {
        let _ = write!(out, "; [blocks truncated]");
    }
    Some(out.finish())
}

fn pass_mode(mode: &SemanticAbiPassModeV1) -> &'static str {
    match mode {
        SemanticAbiPassModeV1::Ignore => "Ignore",
        SemanticAbiPassModeV1::Direct(_) => "Direct",
        SemanticAbiPassModeV1::Pair { .. } => "Pair",
        SemanticAbiPassModeV1::Cast { .. } => "Cast",
        SemanticAbiPassModeV1::Indirect { .. } => "Indirect",
    }
}

fn write_type(out: &mut Bounded, semantic: &AdmittedInertSemanticMirV1, id: SemanticTypeIdV1) {
    let Some(ty) = semantic.types().get(id.index() as usize) else {
        return;
    };
    let _ = write!(out, "type={} identity=", id.index());
    for byte in ty.identity().as_bytes() {
        let _ = write!(out, "{byte:02x}");
    }
    let _ = write!(
        out,
        " size={:?} align={} uninhabited={} shape=",
        ty.layout().size_bytes(),
        ty.layout().alignment_bytes(),
        ty.layout().is_uninhabited()
    );
    match ty.shape() {
        SemanticTypeShapeV1::Tuple(fields)
        | SemanticTypeShapeV1::Aggregate(fields)
        | SemanticTypeShapeV1::Union(fields) => {
            let name = match ty.shape() {
                SemanticTypeShapeV1::Tuple(_) => "Tuple",
                SemanticTypeShapeV1::Union(_) => "Union",
                _ => "Aggregate",
            };
            let _ = write!(
                out,
                "{name}(fields={}, prefix={:?}{})",
                fields.fields().len(),
                &fields.fields()[..fields.fields().len().min(6)],
                if fields.fields().len() > 6 {
                    ",truncated"
                } else {
                    ""
                }
            );
        }
        SemanticTypeShapeV1::Enum {
            discriminant,
            variants,
        } => {
            let _ = write!(
                out,
                "Enum(discriminant={},variants={})",
                discriminant.index(),
                variants.len()
            );
        }
        SemanticTypeShapeV1::Array { element, length } => {
            let _ = write!(out, "Array(element={},length={length})", element.index());
        }
        SemanticTypeShapeV1::Slice { element } => {
            let _ = write!(out, "Slice(element={})", element.index());
        }
        SemanticTypeShapeV1::Pointer(pointer) => {
            let _ = write!(
                out,
                "Pointer(kind={:?},pointee={},metadata={:?})",
                pointer.kind(),
                pointer.pointee().index(),
                pointer.metadata()
            );
        }
        SemanticTypeShapeV1::Scalar(scalar) => {
            let _ = write!(out, "Scalar({scalar:?})");
        }
        SemanticTypeShapeV1::ValidityScalar(_) => {
            let _ = write!(out, "ValidityScalar");
        }
        SemanticTypeShapeV1::FunctionPointer {
            arguments,
            return_type,
            ..
        } => {
            let _ = write!(
                out,
                "FunctionPointer(arguments={},return={})",
                arguments.fields().len(),
                return_type.index()
            );
        }
        SemanticTypeShapeV1::Unit => {
            let _ = write!(out, "Unit");
        }
        SemanticTypeShapeV1::Never => {
            let _ = write!(out, "Never");
        }
        SemanticTypeShapeV1::Opaque => {
            let _ = write!(out, "Opaque");
        }
    }
}

fn write_statement(out: &mut Bounded, statement: &SemanticStatementKindV1) {
    match statement {
        SemanticStatementKindV1::Assign(assignment) => {
            let _ = write!(
                out,
                "Assign(local{},projections={},type{})=",
                assignment.destination().local().index(),
                assignment.destination().projections().len(),
                assignment.value().result_type().index()
            );
            match assignment.value().kind() {
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) => {
                    let _ = write!(
                        out,
                        "Copy(local{},projections={})",
                        place.local().index(),
                        place.projections().len()
                    );
                }
                SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place)) => {
                    let _ = write!(
                        out,
                        "Move(local{},projections={})",
                        place.local().index(),
                        place.projections().len()
                    );
                }
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(value))
                    if matches!(value.value(), SemanticConstantValueV1::ZeroSized) =>
                {
                    let _ = write!(out, "ZeroSized");
                }
                SemanticRvalueKindV1::Aggregate(value) => {
                    let _ = write!(
                        out,
                        "Aggregate({:?},operands={})",
                        value.kind(),
                        value.operands().len()
                    );
                }
                _ => {
                    let _ = write!(out, "other-rvalue");
                }
            }
        }
        SemanticStatementKindV1::StorageLive(local) => {
            let _ = write!(out, "StorageLive({})", local.index());
        }
        SemanticStatementKindV1::StorageDead(local) => {
            let _ = write!(out, "StorageDead({})", local.index());
        }
        SemanticStatementKindV1::SetDiscriminant {
            place,
            variant_index,
        } => {
            let _ = write!(
                out,
                "SetDiscriminant(local{},variant={variant_index})",
                place.local().index()
            );
        }
        SemanticStatementKindV1::Deinitialize(place) => {
            let _ = write!(out, "Deinitialize(local{})", place.local().index());
        }
        SemanticStatementKindV1::Nop => {
            let _ = write!(out, "Nop");
        }
        _ => {
            let _ = write!(out, "other-statement");
        }
    }
}

fn write_terminator(out: &mut Bounded, terminator: &SemanticTerminatorKindV1) {
    match terminator {
        SemanticTerminatorKindV1::Return => {
            let _ = write!(out, "Return");
        }
        SemanticTerminatorKindV1::Call(call) => {
            let _ = write!(
                out,
                "Call(callable{},arguments={},",
                call.callee().index(),
                call.arguments().len()
            );
            if let Some(destination) = call.destination() {
                let _ = write!(
                    out,
                    "dst=local{}/projections{},normal={:?},",
                    destination.place().local().index(),
                    destination.place().projections().len(),
                    destination.edge()
                );
            }
            let _ = write!(out, "unwind={:?})", call.unwind());
        }
        SemanticTerminatorKindV1::Goto(edge) => {
            let _ = write!(out, "Goto({edge:?})");
        }
        SemanticTerminatorKindV1::Unreachable => {
            let _ = write!(out, "Unreachable");
        }
        SemanticTerminatorKindV1::Abort => {
            let _ = write!(out, "Abort");
        }
        SemanticTerminatorKindV1::UnwindResume => {
            let _ = write!(out, "UnwindResume");
        }
        SemanticTerminatorKindV1::UnwindTerminate => {
            let _ = write!(out, "UnwindTerminate");
        }
        _ => {
            let _ = write!(out, "other-terminator(edges={})", terminator.edge_count());
        }
    }
}

#[derive(Default)]
struct Bounded {
    text: String,
    truncated: bool,
}

impl fmt::Write for Bounded {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if self.truncated || text.len() > MAX_BYTES - 16 - self.text.len() {
            self.truncated = true;
            return Err(fmt::Error);
        }
        self.text.push_str(text);
        Ok(())
    }
}

impl Bounded {
    fn finish(mut self) -> String {
        if self.truncated {
            self.text.push_str(" ...[truncated]");
        }
        self.text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undefined_return_diagnostic_bytes_are_strictly_bounded() {
        let mut out = Bounded::default();
        for _ in 0..4096 {
            let _ = write!(out, "0123456789");
        }
        let output = out.finish();
        assert!(output.len() <= MAX_BYTES);
        assert!(output.ends_with("...[truncated]"));
    }

    #[test]
    fn undefined_return_diagnostic_does_not_split_utf8_on_truncation() {
        let mut out = Bounded::default();
        let _ = write!(out, "{}", "x".repeat(MAX_BYTES - 17));
        let _ = write!(out, "{}", char::from_u32(0x03bb).unwrap());
        let output = out.finish();
        assert!(output.len() <= MAX_BYTES);
        assert!(output.ends_with("...[truncated]"));
    }
}
