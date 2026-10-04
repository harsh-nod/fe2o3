//! Closed source profile for the allocated CombinedV4 terminal 133.
//! Literal physical roles are observations of actual MIR operands, never local
//! constant propagation or casts performed by this importer.

use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCanonAbiV1, SemanticExternAbiV1, SemanticFunctionAbiV1,
    SemanticGfx942OrderedRegionRegistersV31, SemanticScalarTypeV1, SemanticTypeDeclV1,
    SemanticTypeShapeV1,
};
use rustc_abi::ExternAbi;
use rustc_hir::Safety;
use rustc_middle::mir::{Body, Operand, START_BLOCK, TerminatorKind, UnwindAction};
use rustc_middle::ty::{FnSig, Instance, TyCtxt, TyKind, TypingEnv, UintTy};

pub(crate) const MAX_PROFILE_BLOCKS: usize = 4096;
const MAX_PROFILE_EDGES: usize = 16_384;

pub(crate) fn valid_signature(
    instance: Instance<'_>,
    signature: &FnSig<'_>,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
) -> bool {
    instance.args.is_empty()
        && signature.safety == Safety::Safe
        && signature.abi == ExternAbi::Rust
        && !signature.c_variadic
        && abi.canon_abi() == SemanticCanonAbiV1::Rust
        && abi.extern_abi() == SemanticExternAbiV1::Rust
        && !abi.c_variadic()
        && !abi.can_unwind()
        && signature.inputs().len() == 8
        && abi.source_input_types().len() == 8
        && signature.inputs().iter().enumerate().all(|(index, ty)| {
            matches!(ty.kind(), TyKind::Uint(kind)
                if *kind == if index < 3 { UintTy::U32 } else { UintTy::U8 })
        })
        && matches!(signature.output().kind(), TyKind::Uint(UintTy::U32))
        && abi
            .source_input_types()
            .iter()
            .enumerate()
            .all(|(index, ty)| unsigned_type(types, *ty, if index < 3 { 32 } else { 8 }))
        && unsigned_type(types, abi.source_output_type(), 32)
}

fn unsigned_type(
    types: &[SemanticTypeDeclV1],
    ty: fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1,
    bits: u16,
) -> bool {
    types.get(ty.index() as usize).is_some_and(|decl| {
        matches!(decl.shape(), SemanticTypeShapeV1::Scalar(
            SemanticScalarTypeV1::Integer { signed: false, bits: actual }) if *actual == bits)
    })
}

pub(crate) fn actual_registers<'tcx>(
    tcx: TyCtxt<'tcx>,
    operands: [&Operand<'tcx>; 5],
) -> Result<SemanticGfx942OrderedRegionRegistersV31, &'static str> {
    let mut literals = [None; 5];
    for (slot, operand) in operands.into_iter().enumerate() {
        let Operand::Constant(constant) = operand else {
            return Err("ordered region physical role is not an actual MIR constant");
        };
        if !matches!(constant.const_.ty().kind(), TyKind::Uint(UintTy::U8)) {
            return Err("ordered region physical role is not exact u8");
        }
        literals[slot] = constant
            .const_
            .try_eval_bits(tcx, TypingEnv::fully_monomorphized());
    }
    registers_from_literals(literals)
}

fn registers_from_literals(
    literals: [Option<u128>; 5],
) -> Result<SemanticGfx942OrderedRegionRegistersV31, &'static str> {
    let mut values = [0; 5];
    for (slot, value) in literals.into_iter().enumerate() {
        values[slot] = value
            .and_then(|value| u8::try_from(value).ok())
            .ok_or("ordered region physical constant cannot be evaluated as u8")?;
    }
    SemanticGfx942OrderedRegionRegistersV31::new(
        values[0],
        values[1],
        [values[2], values[3], values[4]],
    )
    .map_err(|_| "ordered region physical roles must be distinct v0..v63")
}

/// One unique entry prefix establishes unconditional placement before a guard.
/// A separate bounded reachability scan forbids any later re-entry of the pair.
pub(crate) fn require_unconditional_single_execution(
    body: &Body<'_>,
    region: u32,
) -> Result<(), &'static str> {
    let count = body.basic_blocks.len();
    if count == 0 || count > MAX_PROFILE_BLOCKS || region as usize >= count {
        return Err("ordered region source control-flow bound exceeded");
    }
    let mut visited = vec![false; count];
    let mut next = START_BLOCK;
    loop {
        let index = next.index();
        if visited[index] {
            return Err("ordered region entry prefix contains a loop");
        }
        visited[index] = true;
        if index == region as usize {
            break;
        }
        next = match body.basic_blocks[next].terminator().kind {
            TerminatorKind::Goto { target } => target,
            TerminatorKind::Call {
                target: Some(target),
                unwind: UnwindAction::Continue | UnwindAction::Unreachable,
                ..
            } => target,
            _ => return Err("ordered region must precede every conditional source edge"),
        };
    }
    visited.fill(false);
    let block = rustc_middle::mir::BasicBlock::from_usize(region as usize);
    let mut pending = Vec::with_capacity(count);
    let mut edges = 0_usize;
    for successor in body.basic_blocks[block].terminator().successors() {
        charge_edge(&mut edges)?;
        if !visited[successor.index()] {
            visited[successor.index()] = true;
            pending.push(successor);
        }
    }
    while let Some(block) = pending.pop() {
        if block.index() == region as usize {
            return Err("ordered region can execute again through a source loop");
        }
        for successor in body.basic_blocks[block].terminator().successors() {
            charge_edge(&mut edges)?;
            if !visited[successor.index()] {
                visited[successor.index()] = true;
                pending.push(successor);
            }
        }
    }
    Ok(())
}

fn charge_edge(edges: &mut usize) -> Result<(), &'static str> {
    if *edges >= MAX_PROFILE_EDGES {
        return Err("ordered region source control-flow edge bound exceeded");
    }
    *edges += 1;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_control_flow_edge_budget_is_exact_and_does_not_wrap() {
        let mut edges = MAX_PROFILE_EDGES - 1;
        charge_edge(&mut edges).unwrap();
        assert_eq!(edges, MAX_PROFILE_EDGES);
        assert!(charge_edge(&mut edges).is_err());
        assert_eq!(edges, MAX_PROFILE_EDGES);
    }

    #[test]
    fn physical_roles_require_five_evaluated_distinct_in_range_literals() {
        let registers =
            registers_from_literals([Some(32), Some(33), Some(34), Some(35), Some(36)]).unwrap();
        assert_eq!(registers.scratch(), 32);
        assert_eq!(registers.output(), 33);
        assert_eq!(registers.inputs(), [34, 35, 36]);
        assert_eq!(registers.vgpr_high_water(), 37);
        for slot in 0..5 {
            for bad in [None, Some(64), Some(255), Some(256), Some(u128::MAX)] {
                let mut values = [Some(0), Some(1), Some(2), Some(3), Some(63)];
                values[slot] = bad;
                assert!(registers_from_literals(values).is_err());
            }
            for other in 0..slot {
                let mut values = [Some(0), Some(1), Some(2), Some(3), Some(63)];
                values[slot] = values[other];
                assert!(registers_from_literals(values).is_err());
            }
        }
        assert_eq!(
            registers_from_literals([Some(0), Some(63), Some(1), Some(62), Some(31)])
                .unwrap()
                .vgpr_high_water(),
            64
        );
    }
}
