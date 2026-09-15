//! Prefix-bounded non-body ABI observation, never signature acceptance.
use super::*;

const PREFIX: usize = 4;

fn charge(remaining: &mut usize, amount: usize) -> bool {
    if let Some(next) = remaining.checked_sub(amount) {
        *remaining = next;
        true
    } else {
        *remaining = 0;
        false
    }
}

pub(super) fn write(
    out: &mut impl Write,
    index: u32,
    callables: &[SemanticCallableDeclV1],
    remaining: &mut usize,
) -> io::Result<()> {
    if !charge(remaining, 4) {
        return writeln!(out, "callable_abi callee={index} work_truncated=true");
    }
    let Some(callable) = callables.get(index as usize) else {
        return writeln!(out, "callable_abi callee={index} missing=true");
    };
    let Some(binding) = callable.binding() else {
        let SemanticCallableDeclV1::Defined { function } = callable else { unreachable!() };
        return writeln!(out, "callable_abi callee={index} defined_function={} non_body_abi_unavailable=true", function.index());
    };
    if let SemanticCallableDeclV1::CompilerIntrinsic { operation, operation_identity, .. } = callable {
        writeln!(out, "callable_intrinsic kind={:?} identity={operation_identity:?}", std::mem::discriminant(operation))?;
        if let SemanticCompilerIntrinsicOperationV1::ExecutionCapability { contract } = operation {
            writeln!(out, "callable_execution kind={:?} source_identity={:?}",
                std::mem::discriminant(&contract.operation()), contract.source_identity())?;
        }
    }
    let abi = binding.abi();
    writeln!(out,
        "callable_abi callee={index} kind={:?} source_identity={:?} abi_identity={:?} canon={:?} extern={:?} unwind={} variadic={} fixed={} source_count={} physical_count={} source_output={} physical_output={} return_mode={:?}",
        std::mem::discriminant(callable), binding.identity(), abi.identity(), abi.canon_abi(), abi.extern_abi(),
        abi.can_unwind(), abi.c_variadic(), abi.fixed_count(), abi.source_input_types().len(),
        abi.arguments().len(), abi.source_output_type().index(), abi.return_value().adjusted_ty().index(),
        abi.return_value().mode())?;
    for (i, ty) in abi.source_input_types().iter().take(PREFIX).enumerate() {
        if !charge(remaining, 4) {
            return writeln!(out, "callable_abi callee={index} work_truncated=true");
        }
        writeln!(out, "callable_source_arg={i} ty={} ownership={:?}",
            ty.index(), abi.source_argument_ownership().get(i))?;
    }
    for (i, arg) in abi.arguments().iter().take(PREFIX).enumerate() {
        if !charge(remaining, 4) {
            return writeln!(out, "callable_abi callee={index} work_truncated=true");
        }
        // PassMode contains fixed-size attributes and at most eight cast registers;
        // do not Debug-print ABI values' variable-size adjusted type layouts.
        writeln!(out, "callable_physical_arg={i} role={:?} source_ty={} adjusted_ty={} mode={:?}",
            arg.role(), arg.ty().index(), arg.value().adjusted_ty().index(), arg.mode())?;
    }
    writeln!(out, "callable_abi callee={index} prefix_truncated={} work_truncated=false remaining={remaining}",
        abi.source_input_types().len() > PREFIX || abi.arguments().len() > PREFIX)
}
