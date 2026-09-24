//! Actual rustc-source token -> inert canonical semantic call annotation.
//! No Function/Module/materializer/renderer is invoked at this frontend step.
//! The normal body producer consumes the one-use token, attaches this exact
//! record to the matching call and requires the occurrence table drained.
use crate::production_complete_body_source_occurrences_vnext::CompleteBodySourceVNext;
use dialect_amdgcn::{
    Gfx942CompleteBodyBoundaryV1, Gfx942CompleteBodyPlanV1, Gfx942CompleteBodyResourcesV1,
    Gfx942CompleteBodySymbolV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1,
    Gfx942OrderedProgramRegistersV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCompleteBodyPackingVNext, SemanticCompleteBodySourceVNext,
};
use rustc_abi::{CanonAbi, ExternAbi};
use rustc_target::callconv::PassMode;
use sha2::{Digest, Sha256};

pub(crate) struct ProducedCompleteBodySemanticSourceVNext {
    pub(crate) source: SemanticCompleteBodySourceVNext,
    pub(crate) packing: SemanticCompleteBodyPackingVNext,
    /// Compared to the actual generated semantic call's five literal u8 inputs.
    pub(crate) registers: [u8; 5],
}

/// Consumes the genuine source token; there is no inert record/token constructor
/// and no public source-authentication API. A refusal invalidates this import.
pub(crate) fn produce_complete_body_semantic_source_vnext(
    source: CompleteBodySourceVNext<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<ProducedCompleteBodySemanticSourceVNext, String> {
    budget.charge_work(128).map_err(|e| e.to_string())?;
    let [root] = source.plan().function_producers() else {
        return Err("complete-body current root roster differs".into());
    };
    let [abi] = source.plan().function_abi_producers() else {
        return Err("complete-body current root ABI roster differs".into());
    };
    if root.identities != source.root_identities()
        || abi.function.index() != 0
        || abi.extern_abi != ExternAbi::GpuKernel
        || abi.fn_abi.conv != CanonAbi::GpuKernel
        || abi.fn_abi.c_variadic
        || abi.fn_abi.can_unwind
        || abi.fn_abi.fixed_count != 5
        || abi.source_inputs.len() != 5
        || abi.fn_abi.args.len() != 5
        || !abi.source_output.is_unit()
        || !abi.fn_abi.ret.layout.ty.is_unit()
        || !matches!(abi.fn_abi.ret.mode, PassMode::Ignore)
    {
        return Err("complete-body promoted kernel ABI differs".into());
    }
    for (position, (source_ty, argument)) in abi
        .source_inputs
        .iter()
        .zip(abi.fn_abi.args.iter())
        .enumerate()
    {
        if *source_ty != argument.layout.ty {
            return Err("complete-body adjusted source ABI type differs".into());
        }
        let valid = if position == 0 {
            matches!(argument.mode, PassMode::Pair(_, _))
                && argument.layout.size.bytes() == 16
                && argument.layout.align.abi.bytes() == 8
        } else {
            matches!(argument.mode, PassMode::Direct(_))
                && matches!(
                    source_ty.kind(),
                    rustc_middle::ty::TyKind::Uint(rustc_middle::ty::UintTy::U32)
                )
                && argument.layout.size.bytes() == 4
                && argument.layout.align.abi.bytes() == 4
        };
        if !valid {
            return Err("complete-body adjusted source ABI layout or pass mode differs".into());
        }
    }
    let export = root
        .export_name
        .as_deref()
        .ok_or("complete-body current export absent")?;
    // Same exact grammar as normal final emission. Never sanitize or truncate.
    Gfx942CompleteBodySymbolV1::new(export).map_err(|e| e.to_string())?;
    let frontend = root
        .frontend_contract
        .as_ref()
        .ok_or("complete-body frontend binding absent")?;
    if frontend.resource_contract().is_some_and(|r| {
        r.static_shared_memory_bytes() != 0 || r.max_dynamic_shared_memory_bytes() != 0
    }) {
        return Err("complete-body source declares unadmitted workgroup storage".into());
    }
    budget
        .charge_work(frontend.canonical_bytes().len())
        .map_err(|e| e.to_string())?;
    let identity = source.root_identities();
    let record = SemanticCompleteBodySourceVNext::new(
        [
            *identity.function().as_bytes(),
            *identity.item_definition().as_bytes(),
            *identity.monomorphization().as_bytes(),
            *identity.generic_type_arguments().as_bytes(),
            *identity.const_generic_arguments().as_bytes(),
        ],
        source.mir_body_sha256(),
        *source.semantic_block_identity().as_bytes(),
        abi.rustc_source_signature_sha256,
        abi.rustc_fn_abi_sha256,
        Sha256::digest(frontend.canonical_bytes()).into(),
        source.raw_block(),
    )
    .map_err(str::to_owned)?;
    let physical = source.registers();
    let registers = Gfx942OrderedProgramRegistersV1::new(
        physical[0],
        physical[1],
        [physical[2], physical[3], physical[4]],
    )
    .map_err(|e| e.to_string())?;
    // Parent ledger prepays adaptation to the existing work-only model checker.
    // This is a declaration check, not emitted ABI/resource/native evidence.
    budget.charge_work(576).map_err(|e| e.to_string())?;
    let mut model_work = CanonicalKernelIrWorkBudgetV1::new(576);
    let checked = Gfx942CompleteBodyPlanV1::check_packed(
        Gfx942CompleteBodyBoundaryV1::PROFILE,
        registers,
        Gfx942CompleteBodyResourcesV1::required(registers),
        source.packed(),
        &mut model_work,
    )
    .map_err(|e| e.to_string())?;
    if checked.block_count() != 1 && checked.block_count() != 4 {
        return Err(
            "complete-body initial source importer requires one block or selector diamond".into(),
        );
    }
    let packed = source.packed();
    Ok(ProducedCompleteBodySemanticSourceVNext {
        source: record,
        registers: physical,
        packing: SemanticCompleteBodyPackingVNext {
            block_count: packed.block_count(),
            instruction_count: packed.instruction_count(),
            block_words: packed.block_words(),
            instruction_words: packed.instruction_words(),
        },
    })
}
