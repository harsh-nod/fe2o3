//! Failure-only source/frame metadata, never a forwarding proof or fallback.
use fe2o3_mir_model::semantic_mir_v1::*;
use std::ffi::OsStr;
use std::io::{self, Write};

pub(super) struct Context {
    pub root: u32,
    pub view: [u8; 32],
    pub expanded_body: [u8; 32],
    pub wrapper: (u32, u32),
    pub checked_instance: u32,
    pub construction: (u32, Option<u32>, u32),
}

pub(super) fn emit(
    body: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    checked: SemanticFunctionIdV1,
    call_block: Option<SemanticBlockIdV1>,
    context: &Context,
) {
    let flag = std::env::var_os("FE2O3_TRACE_CAPABILITY_CUSTODY");
    if flag.as_deref() != Some(OsStr::new("1")) {
        return;
    }
    let _ = write_if_enabled(
        flag.as_deref(),
        &mut io::stderr().lock(),
        body,
        callables,
        checked,
        call_block,
        context,
    );
}

fn hex(out: &mut impl Write, bytes: &[u8; 32]) -> io::Result<()> {
    for byte in bytes {
        write!(out, "{byte:02x}")?;
    }
    Ok(())
}

// Each prefix is the fixed arity of the existing predicate. Unexpected counts
// remain explicit; no body scan, allocation, replay or graph query is needed.
pub(super) fn write_if_enabled(
    flag: Option<&OsStr>,
    out: &mut impl Write,
    body: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    checked: SemanticFunctionIdV1,
    call_block: Option<SemanticBlockIdV1>,
    context: &Context,
) -> io::Result<()> {
    if flag != Some(OsStr::new("1")) {
        return Ok(());
    }
    write!(out, "BF16_FORWARDER_FRAME source_identity=")?;
    hex(out, body.identity().as_bytes())?;
    write!(out, " expanded_body=")?;
    hex(out, &context.expanded_body)?;
    write!(out, " view=")?;
    hex(out, &context.view)?;
    writeln!(
        out,
        " root={} wrapper={:?} checked_instance={} checked_function={} construction={:?} call_block={:?} entry={} diagnostic_only=true",
        context.root,
        context.wrapper,
        context.checked_instance,
        checked.index(),
        context.construction,
        call_block.map(|b| b.index()),
        body.entry().index()
    )?;

    let abi = body.abi();
    let inputs = abi.source_input_types();
    let ownership = abi.source_argument_ownership();
    let statements: [Option<usize>; 2] =
        std::array::from_fn(|i| body.blocks().get(i).map(|b| b.statements().len()));
    writeln!(
        out,
        "BF16_FORWARDER_SHAPE inputs={} locals={} blocks={} statements_prefix={:?} abi_unwind={} abi_variadic={} ownership_count={} ownership_prefix={:?} input_types_prefix={:?} output={}",
        inputs.len(),
        body.locals().len(),
        body.blocks().len(),
        statements,
        abi.can_unwind(),
        abi.c_variadic(),
        ownership.len(),
        &ownership[..ownership.len().min(6)],
        &inputs[..inputs.len().min(6)],
        abi.source_output_type().index()
    )?;
    let locals: [Option<(u32, SemanticLocalRoleV1)>; 7] = std::array::from_fn(|i| {
        body.locals()
            .get(i)
            .map(|local| (local.ty().index(), local.role()))
    });
    writeln!(
        out,
        "BF16_FORWARDER_LOCALS prefix={locals:?} truncated={}",
        body.locals().len() > 7
    )?;

    let call = body
        .blocks()
        .get(body.entry().index() as usize)
        .and_then(|block| {
            if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() {
                Some(call)
            } else {
                None
            }
        });
    let Some(call) = call else {
        writeln!(out, "BF16_FORWARDER_CALL entry_call=false")?;
        return writeln!(
            out,
            "BF16_FORWARDER_ARGUMENTS count=0 available=false prefix=[]"
        );
    };
    let (callee_kind, callee_function) = match callables.get(call.callee().index() as usize) {
        Some(SemanticCallableDeclV1::Defined { function }) => ("Defined", Some(function.index())),
        Some(_) => ("Other", None),
        None => ("Missing", None),
    };
    let destination = call.destination().map(|destination| {
        let place = destination.place();
        let edge = destination.edge();
        let target_return = body
            .blocks()
            .get(edge.target().index() as usize)
            .is_some_and(|block| {
                matches!(block.terminator().kind(), SemanticTerminatorKindV1::Return)
            });
        (
            place.local().index(),
            place.ty().index(),
            place.projections().len(),
            edge.target().index(),
            edge.role(),
            target_return,
        )
    });
    writeln!(
        out,
        "BF16_FORWARDER_CALL entry_call=true callee={} callee_kind={} callee_function={:?} arguments={} variadic_abis={} unwind_unreachable={} destination={:?}",
        call.callee().index(),
        callee_kind,
        callee_function,
        call.arguments().len(),
        call.variadic_argument_abis().len(),
        matches!(call.unwind(), SemanticUnwindActionV1::Unreachable),
        destination
    )?;
    let args: [Option<(&str, Option<u32>, u32, Option<usize>)>; 5] = std::array::from_fn(|i| {
        call.arguments().get(i).map(|operand| {
            let (kind, place) = match operand {
                SemanticOperandV1::Copy(place) => ("Copy", Some(place)),
                SemanticOperandV1::Move(place) => ("Move", Some(place)),
                SemanticOperandV1::Constant(_) => ("Constant", None),
            };
            (
                kind,
                place.map(|p| p.local().index()),
                operand.ty().index(),
                place.map(|p| p.projections().len()),
            )
        })
    });
    writeln!(
        out,
        "BF16_FORWARDER_ARGUMENTS count={} available=true prefix={args:?} truncated={}",
        call.arguments().len(),
        call.arguments().len() > 5
    )
}
