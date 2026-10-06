//! All active original bodies share the exact source-owned invocation roster.
//! Logical local ranges are not an allocation namespace. This source model
//! does not discharge actual call splicing or any memory interpretation.
use crate::mixed_optimizer_refinement_v26::semantics::byte_function_v30::ByteInterpretationContextV39 as ByteContext;
use crate::mixed_optimizer_refinement_v26::semantics::byte_function_v30::EmittedByteFunctionsV55;
use crate::mixed_optimizer_refinement_v26::semantics::target_view_contracts_v38::TargetByteViewContractsV38 as TargetContracts;

use super::{
    Error, Resource, Result, Writer,
    call_transfers::{CallTransfers, Transfer},
    control::SourceControl,
    vector,
};
use std::{mem::size_of, ops::Range};

#[path = "original_semantic_mir_invocation_generate_v34.rs"]
mod generate;

#[path = "original_semantic_mir_invocation_actual_v35.rs"]
mod actual;

#[path = "original_semantic_mir_invocation_bytes_v36.rs"]
mod bytes;

#[path = "original_semantic_mir_invocation_slots_v36.rs"]
mod slots;

#[path = "original_semantic_mir_invocation_source_scalar_v36.rs"]
mod source_scalar;

#[path = "original_semantic_mir_invocation_source_bytes_v36.rs"]
mod source_bytes;

#[path = "original_semantic_mir_invocation_source_frames_v36.rs"]
mod source_frames;

#[path = "original_semantic_mir_support_closure_v97.rs"]
mod support_closure;

#[path = "original_semantic_mir_invocation_source_enter_v36.rs"]
mod source_enter;

#[path = "original_semantic_mir_invocation_source_function_v36.rs"]
mod source_function;

#[path = "original_semantic_mir_invocation_byte_bindings_v36.rs"]
mod byte_bindings;

#[path = "original_semantic_mir_invocation_effects_v36.rs"]
mod effects;

#[path = "original_semantic_mir_invocation_paired_v36.rs"]
mod paired;

#[path = "original_semantic_mir_invocation_typed_tail_v49.rs"]
mod typed_tail;

#[path = "original_semantic_mir_tile_target_v176.rs"]
mod tile_target;

#[path = "original_semantic_mir_expanded_execution_bindings_v199.rs"]
mod expanded_execution;

#[path = "original_semantic_mir_expanded_generation_v221.rs"]
mod expanded_generation;

#[path = "original_semantic_mir_expanded_model_v280.rs"]
pub(super) mod expanded_model_v280;

#[path = "original_semantic_mir_reference_expressions_v69.rs"]
mod reference_expressions;

#[path = "original_semantic_mir_reference_consumer_v69.rs"]
mod reference_consumer;

#[cfg(test)]
#[path = "original_semantic_mir_typed_tail_v49_tests.rs"]
mod typed_tail_tests;

pub(super) struct Body {
    root: usize,
    instance: usize,
    locals: Range<usize>,
    blocks: Range<usize>,
    returned: Option<usize>,
    control: SourceControl,
}

pub(super) struct InvocationBodies<'a, 'plan, 'view, 'source> {
    transfers: &'a CallTransfers<'plan, 'view, 'source>,
    roots: Vec<Range<usize>>,
    bodies: Vec<Option<Body>>,
    locals: usize,
    required: usize,
}

fn mismatch() -> Error {
    Error::Statement("original MIR body differs from its exact invocation transfer")
}

// An explicit policy ceiling on sparse partition boundaries, not object bytes
// or a claim that an incomplete copy closure has been proved complete.
const MAX_PRIVATE_BYTE_BOUNDARIES_V38: usize = 1 << 20;
const SOURCE_TAG_NAMESPACE_V40: usize = 0;
const TARGET_TAG_NAMESPACE_V40: usize = 1;

pub(crate) fn generate_refinement_v36(
    relation: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
    launches: &[fe2o3_kernel_ir::ExplicitLaunchExtent],
    width: fe2o3_kernel_ir::FormalIndexWidth,
    endianness: fe2o3_kernel_ir::EndiannessV2,
    out: &mut Writer<'_, '_>,
) -> Result<[usize; 6]> {
    generate_refinement_inner_v49(relation, launches, width, endianness, None, &[], out)
}

pub(crate) fn generate_refinement_typed_v49<'a, 'owner, 'rows>(
    relation: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
    prefix: &'a fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV18<'a, 'owner, 'owner, 'rows>,
    licm: &'a fe2o3_kernel_analysis::CheckedCanonicalKirLicmV18<'owner>,
    relocated: &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'owner>,
    forwarding: &'a fe2o3_kernel_analysis::CheckedCanonicalKirCrossBlockForwardingV18<'owner>,
    final_inventory: &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'owner>,
    launches: &[fe2o3_kernel_ir::ExplicitLaunchExtent],
    width: fe2o3_kernel_ir::FormalIndexWidth,
    endianness: fe2o3_kernel_ir::EndiannessV2,
    out: &mut Writer<'_, '_>,
) -> Result<[usize; 6]> {
    generate_refinement_typed_with_references_v69(
        relation,
        prefix,
        licm,
        relocated,
        forwarding,
        final_inventory,
        launches,
        width,
        endianness,
        &[],
        out,
    )
}

pub(crate) fn generate_refinement_typed_with_references_v69<'a, 'owner, 'rows>(
    relation: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
    prefix: &'a fe2o3_kernel_analysis::CheckedCanonicalKirTransitionV18<'a, 'owner, 'owner, 'rows>,
    licm: &'a fe2o3_kernel_analysis::CheckedCanonicalKirLicmV18<'owner>,
    relocated: &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'owner>,
    forwarding: &'a fe2o3_kernel_analysis::CheckedCanonicalKirCrossBlockForwardingV18<'owner>,
    final_inventory: &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'owner>,
    launches: &[fe2o3_kernel_ir::ExplicitLaunchExtent],
    width: fe2o3_kernel_ir::FormalIndexWidth,
    endianness: fe2o3_kernel_ir::EndiannessV2,
    references: &[crate::SourceScalarReferenceInputV69<'_>],
    out: &mut Writer<'_, '_>,
) -> Result<[usize; 6]> {
    let tail = typed_tail::Tail::derive_final(
        relation,
        prefix,
        licm,
        relocated,
        forwarding,
        final_inventory,
        out,
    )?;
    generate_refinement_inner_v49(
        relation,
        launches,
        width,
        endianness,
        Some(tail),
        references,
        out,
    )
}

fn generate_refinement_inner_v49(
    relation: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
    launches: &[fe2o3_kernel_ir::ExplicitLaunchExtent],
    width: fe2o3_kernel_ir::FormalIndexWidth,
    endianness: fe2o3_kernel_ir::EndiannessV2,
    tail: Option<typed_tail::Tail<'_, '_, '_>>,
    references: &[crate::SourceScalarReferenceInputV69<'_>],
    out: &mut Writer<'_, '_>,
) -> Result<[usize; 6]> {
    use std::fmt::Write as _;
    out.budget.reserve_storage(generation_headers_v36())?;
    let source = relation.source(out.budget)?;
    let plan = super::invocations::InvocationPlan::derive(source, out)?;
    let slots = slots::SourceSlots::derive(&plan, relation, out)?;
    let inventory = relation.inventory(out.budget)?;
    let contracts = TargetContracts::derive(inventory, width, out)?;
    let tag_pairs = slots::SourceTagPairsV40::derive(&slots, &contracts, out)?;
    let mut byte_source = source_function::SourceByteProgram::derive(&plan, &slots, out)?;
    let byte_bindings = byte_bindings::SourceByteBindings::derive(&slots, out)?;
    let paired = paired::PairedInvocations::derive(&plan, &byte_source, width, out)?;
    let (physical, physical_storage) =
        fe2o3_kernel_analysis::analyze_canonical_kir_private_bytes_v38(
            inventory,
            fe2o3_kernel_analysis::CanonicalKirPrivateByteLimitsV38 {
                max_boundaries: MAX_PRIVATE_BYTE_BOUNDARIES_V38,
            },
            out.budget,
        )?;
    out.budget
        .reserve_storage(physical_storage.retained_storage())?;
    let roots = source.root_count(out.budget)?;
    if launches.len() != roots {
        return Err(mismatch());
    }
    let mut byte_actual = vector(roots, out)?;
    for root in 0..roots {
        tag_pairs.check(out)?;
        let (_, function) = source.root(root, out.budget)?;
        out.budget.charge_work(1)?;
        byte_actual.push(super::super::byte_function_v30::ByteFunctionV30::derive(
            inventory,
            &physical,
            fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                u32::try_from(function).map_err(|_| Resource::Arithmetic)?,
            ),
            ByteContext::classified(width, &contracts, TARGET_TAG_NAMESPACE_V40),
            &slots,
            out,
        )?);
    }
    out.budget.charge_work(2)?;
    let original_index_floor = out.budget.storage();
    let mut emitted_original = if tail.is_some() {
        Some(EmittedByteFunctionsV55::new(
            inventory,
            &physical,
            &slots,
            ByteContext::classified(width, &contracts, TARGET_TAG_NAMESPACE_V40),
            out,
        )?)
    } else {
        None
    };
    let original_index_storage = out
        .budget
        .storage()
        .checked_sub(original_index_floor)
        .ok_or(Resource::Accounting)?;
    let census = paired.census();
    emit_model_prelude_v187(out)?;
    tag_pairs.check(out)?;
    slots.emit_source_tag_contracts(SOURCE_TAG_NAMESPACE_V40, out)?;
    contracts.emit(TARGET_TAG_NAMESPACE_V40, out)?;
    tag_pairs.emit(SOURCE_TAG_NAMESPACE_V40, TARGET_TAG_NAMESPACE_V40, out)?;
    slots.emit(out)?;
    byte_source
        .emit(out)
        .map_err(|error| out.source_section_error(error, "original source byte program"))?;
    byte_bindings
        .emit(out)
        .map_err(|error| out.source_section_error(error, "original source byte bindings"))?;
    let index_bytes = match width {
        fe2o3_kernel_ir::FormalIndexWidth::Bits32 => 4,
        fe2o3_kernel_ir::FormalIndexWidth::Bits64 => 8,
        fe2o3_kernel_ir::FormalIndexWidth::Unknown => return Err(mismatch()),
    };
    write!(out, "spec fn invocation_runtime_index_bytes_v36() -> int {{ {index_bytes} }}\nspec fn invocation_runtime_little_endian_v36() -> bool {{ {} }}\n", matches!(endianness, fe2o3_kernel_ir::EndiannessV2::Little)).map_err(|_| out.error())?;
    for (root, (function, launch)) in byte_actual.iter().zip(launches).enumerate() {
        out.budget.charge_work(6)?;
        let fe2o3_kernel_ir::ExplicitLaunchExtent::Exact { rank, extents } = launch else {
            return Err(mismatch());
        };
        let emitted = match emitted_original.as_mut() {
            Some(index) => index.emit(function, root, out),
            None => function.emit(root, out),
        };
        emitted.map_err(|error| {
            out.source_section_error(error, "original canonical byte functions")
        })?;
        write!(out, "spec fn invocation_runtime_launch_{root}_v36() -> (int, Seq<int>) {{ ({rank}, seq![{}, {}, {}]) }}\n", extents[0], extents[1], extents[2]).map_err(|_| out.error())?;
        emit_execution_v37(relation, root, out)?;
        write!(out, "spec fn invocation_source_initial_runtime_{root}_v36(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, execution: MemoryExecutionContextV37) -> InvocationSourceByteStateV36 {{ invocation_source_byte_initial_{root}_v36(arguments, external, execution, invocation_runtime_little_endian_v36()) }}\nspec fn invocation_source_block_runtime_{root}_v36(source: InvocationSourceByteStateV36) -> InvocationSourceBlockResultV36 {{ invocation_source_byte_block_{root}_v36(source, invocation_runtime_little_endian_v36()) }}\nspec fn invocation_actual_micro_runtime_{root}_v36(cursor: MemoryMicroStateV30) -> MemoryMicroResultV30 {{ byte_micro_step_{root}_v30(cursor, invocation_runtime_little_endian_v36()) }}\n").map_err(|_| out.error())?;
    }
    paired.emit(out).map_err(|error| {
        out.source_section_error(error, "original paired invocation obligations")
    })?;
    byte_source
        .emit_cut_frame_proofs_v93(Some(&paired), out)
        .map_err(|error| out.source_section_error(error, "original source cut frame proofs"))?;
    byte_source
        .emit_thread_write_normal_proofs_v94(out)
        .map_err(|error| out.source_section_error(error, "original source write normal forms"))?;
    if let Some(tail) = tail {
        byte_bindings.emit_carrier_extensionality_v48(out)?;
        tail.emit(
            relation,
            &slots,
            &physical,
            &contracts,
            emitted_original.as_ref().ok_or_else(mismatch)?,
            width,
            out,
        )
        .map_err(|error| out.source_section_error(error, "typed optimizer tail"))?;
    }
    reference_consumer::emit(relation, &plan, &slots, references, launches, width, out)?;
    paired.check_cut_summary_owner_v96(&slots, out)?;
    support_closure::retain_referenced(out)?;
    write!(out, "}}\n").map_err(|_| out.error())?;
    drop(emitted_original);
    // The typed tail releases only its nested delta; refund this older reservation
    // separately, never any source model or intervening proof-emission credit.
    out.budget.release_storage(original_index_storage)?;
    drop(byte_actual);
    drop(physical);
    out.budget
        .release_storage(physical_storage.retained_storage())?;
    Ok(census)
}

fn emit_model_prelude_v187(out: &mut Writer<'_, '_>) -> Result<()> {
    use std::fmt::Write as _;
    write!(
        out,
        "use vstd::prelude::*;\nuse vstd::seq_lib::*;\nverus! {{\n"
    )
    .map_err(|_| out.error())?;
    super::relation::emit_prelude(out)?;
    write!(
        out,
        "{}{}{}{}{}{}{}{}{}{}{}",
        super::super::structured_state_v30::STATE,
        super::control_generate::SOURCE_STATE,
        super::super::cfg_trace::PRELUDE,
        super::super::byte_memory_v30::BYTE_MEMORY_V30,
        bytes::INVOCATION_BYTES_V36,
        bytes::CLASSIFIED_VIEW_LAWS_V40,
        source_bytes::SOURCE_BYTES_V36,
        source_bytes::SOURCE_POINTERS_V36,
        source_frames::SOURCE_FRAMES_V36,
        source_function::SOURCE_FUNCTION_V36,
        effects::INVOCATION_EFFECTS_V36,
    )
    .map_err(|_| out.error())
}

fn emit_execution_v37(
    relation: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    use std::fmt::Write as _;
    let source = relation.source(out.budget)?;
    let (function, _) = source.root(root, out.budget)?;
    let semantic = source.source_semantic(out.budget)?;
    out.budget.charge_work(6)?;
    let launch = semantic
        .functions()
        .get(function.index() as usize)
        .and_then(|function| function.kernel_entry())
        .ok_or_else(mismatch)?
        .source_contract()
        .launch();
    write!(out, "spec fn invocation_runtime_execution_{root}_v37(execution: MemoryExecutionContextV37) -> bool {{\n byte_execution_well_formed_v37(execution) && execution.rank == invocation_runtime_launch_{root}_v36().0 && execution.extent == invocation_runtime_launch_{root}_v36().1\n && execution.workgroup[0] * execution.workgroup[1] * execution.workgroup[2] <= {}\n && (forall|axis: int| 0 <= axis < 3 ==> execution.extent[axis] <= memory_value_modulus_v30(invocation_runtime_index_bytes_v36()) && execution.workgroup[axis] < memory_value_modulus_v30(invocation_runtime_index_bytes_v36()))", fe2o3_mir_model::semantic_mir_v1::MAX_SEMANTIC_WORKGROUP_THREADS_V1).map_err(|_| out.error())?;
    if let Some(required) = launch.and_then(|launch| launch.required()) {
        out.budget.charge_work(3)?;
        let [x, y, z] = required.as_array();
        write!(
            out,
            "\n && execution.workgroup == seq![{x}int, {y}int, {z}int]"
        )
        .map_err(|_| out.error())?;
    }
    if let Some(maximum) = launch.and_then(|launch| launch.maximum()) {
        out.budget.charge_work(3)?;
        let [x, y, z] = maximum.as_array();
        write!(out, "\n && execution.workgroup[0] <= {x} && execution.workgroup[1] <= {y} && execution.workgroup[2] <= {z}")
            .map_err(|_| out.error())?;
    }
    write!(out, "\n}}\n").map_err(|_| out.error())?;
    Ok(())
}

fn generation_headers_v36() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<&fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>>()
        + h::<&mut Writer<'_, '_>>()
        + h::<super::invocations::InvocationPlan<'_, '_>>()
        + h::<slots::SourceSlots<'_, '_>>()
        + h::<TargetContracts<'_, '_>>()
        + h::<slots::SourceTagPairsV40<'_, '_, '_, '_, '_>>()
        + h::<&TargetContracts<'_, '_>>()
        + h::<&slots::SourceTagPairsV40<'_, '_, '_, '_, '_>>()
        + h::<source_function::SourceByteProgram<'_, '_, '_>>()
        + h::<byte_bindings::SourceByteBindings<'_, '_, '_>>()
        + h::<Option<EmittedByteFunctionsV55<'_, '_, slots::SourceSlots<'_, '_>>>>()
        + h::<&EmittedByteFunctionsV55<'_, '_, slots::SourceSlots<'_, '_>>>()
        + h::<
            Vec<
                super::super::byte_function_v30::ByteFunctionV30<
                    '_,
                    '_,
                    slots::SourceSlots<'_, '_>,
                >,
            >,
        >()
        + h::<&[fe2o3_kernel_ir::ExplicitLaunchExtent]>()
        + h::<fe2o3_kernel_ir::FormalIndexWidth>()
        + h::<fe2o3_kernel_ir::EndiannessV2>()
        + h::<paired::PairedInvocations<'_, '_, '_>>()
        + h::<fe2o3_kernel_analysis::CanonicalKirPrivateByteAnalysisV38<'_, '_>>()
        + h::<fe2o3_kernel_analysis::CanonicalKirPrivateByteStorageV38>()
        + h::<fe2o3_kernel_analysis::CanonicalKirPrivateByteLimitsV38>()
        + size_of::<
            std::result::Result<
                (
                    fe2o3_kernel_analysis::CanonicalKirPrivateByteAnalysisV38<'_, '_>,
                    fe2o3_kernel_analysis::CanonicalKirPrivateByteStorageV38,
                ),
                fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1,
            >,
        >()
        + h::<[usize; 6]>()
        + h::<Option<fe2o3_mir_model::semantic_mir_v1::SemanticKernelLaunchBoundsV1>>()
        + h::<fe2o3_mir_model::semantic_mir_v1::SemanticWorkgroupDimensionsV1>()
        + h::<[u32; 3]>()
        + 7 * size_of::<&()>()
        + 10 * size_of::<usize>()
}

impl<'a, 'plan, 'view, 'source> InvocationBodies<'a, 'plan, 'view, 'source> {
    pub(super) fn derive(
        transfers: &'a CallTransfers<'plan, 'view, 'source>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        let plan = transfers.plan(out)?;
        let source = plan.source(out)?;
        out.budget.reserve_storage(headers())?;
        let semantic = source.source_semantic(out.budget)?;
        let ssa = source.source_ssa(out.budget)?;
        let root_count = source.root_count(out.budget)?;
        let mut count = 0usize;
        for root in 0..root_count {
            out.budget.charge_work(1)?;
            count = count
                .checked_add(plan.root(root, out)?.instances.len())
                .ok_or(Resource::Arithmetic)?;
        }
        let mut roots = vector(root_count, out)?;
        let mut bodies = vector(count, out)?;
        let mut locals = 0;
        for root in 0..root_count {
            let scope = plan.root(root, out)?;
            if scope.instances.start != bodies.len() {
                return Err(mismatch());
            }
            roots.push(scope.instances.clone());
            for instance in 0..scope.instances.len() {
                out.budget.charge_work(5)?;
                let row = plan.instance(root, instance, out)?;
                if row.locals.start != locals || row.locals.start > row.locals.end {
                    return Err(mismatch());
                }
                locals = row.locals.end;
                if !row.active {
                    bodies.push(None);
                    continue;
                }
                let function = semantic
                    .functions()
                    .get(row.function.index() as usize)
                    .ok_or_else(mismatch)?;
                let original_plan = ssa
                    .plan_for_function(row.function)
                    .ok_or_else(mismatch)?
                    .plan();
                let calls = transfers.context(root, instance, out)?;
                let control = SourceControl::derive_instance(
                    semantic.types(),
                    function,
                    original_plan,
                    &calls,
                    out,
                )?;
                if control.locals != row.locals.len() || control.blocks.len() != row.blocks.len() {
                    return Err(mismatch());
                }
                let returned = match row.incoming {
                    None if instance == 0 => None,
                    Some((parent, site)) if parent < instance => {
                        let parent = transfers.context(root, parent, out)?;
                        let (index, transfer) = parent.call(site.index() as usize, out)?;
                        if transfer.child != instance
                            || transfer.child_locals != row.locals
                            || transfer.entry
                                != row
                                    .blocks
                                    .start
                                    .checked_add(control.entry.get() as usize)
                                    .ok_or(Resource::Arithmetic)?
                        {
                            return Err(mismatch());
                        }
                        // The independently read callee Return roster must be
                        // exactly the one retained by the source transfer.
                        let mut cursor = 0;
                        for (block, declaration) in function.blocks().iter().enumerate() {
                            out.budget.charge_work(2)?;
                            if matches!(declaration.terminator().kind(), super::Terminator::Return)
                            {
                                let pc = row
                                    .blocks
                                    .start
                                    .checked_add(block)
                                    .ok_or(Resource::Arithmetic)?;
                                if transfer.return_blocks.get(cursor) != Some(&pc) {
                                    return Err(mismatch());
                                }
                                cursor += 1;
                            }
                        }
                        if cursor != transfer.return_blocks.len() {
                            return Err(mismatch());
                        }
                        Some(index)
                    }
                    _ => return Err(mismatch()),
                };
                bodies.push(Some(Body {
                    root,
                    instance,
                    locals: row.locals.clone(),
                    blocks: row.blocks.clone(),
                    returned,
                    control,
                }));
            }
        }
        if bodies.len() != count {
            return Err(mismatch());
        }
        Ok(Self {
            transfers,
            roots,
            bodies,
            locals,
            required: out.budget.storage(),
        })
    }

    fn check(&self, out: &Writer<'_, '_>) -> Result<()> {
        let plan = self.transfers.plan(out)?;
        if out.budget.storage() < self.required {
            return Err(plan
                .source(out)?
                .retain_query_resource_error_v18(Resource::Accounting)
                .into());
        }
        Ok(())
    }

    fn transfer(
        &self,
        index: usize,
        out: &Writer<'_, '_>,
    ) -> Result<&super::call_transfers::DirectTransfer> {
        self.check(out)?;
        match self
            .transfers
            .rows(out)?
            .get(index)
            .map(|row| &row.transfer)
        {
            Some(Transfer::Direct(transfer)) => Ok(transfer),
            _ => Err(mismatch()),
        }
    }

    pub(super) fn emit_steps(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.check(out)?;
        generate::emit_steps(self, out)
    }
}

fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<InvocationBodies<'_, '_, '_, '_>>()
        + h::<Body>()
        + h::<Vec<Option<Body>>>()
        + h::<Vec<Range<usize>>>()
        + h::<SourceControl>()
        + h::<Range<usize>>()
        + h::<Option<usize>>()
        + h::<&super::invocations::Instance>()
        + h::<&super::invocations::Root>()
        + h::<&super::invocations::InvocationPlan<'_, '_>>()
        + h::<&super::call_transfers::DirectTransfer>()
        + h::<&[super::call_transfers::CallRow]>()
        + h::<&fe2o3_lower_mir_kernel::ProductionSourceOwnedViewV18<'_>>()
        + h::<&fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1>()
        + h::<&fe2o3_pliron::ProductionSemanticSsaOwnerV1>()
        + h::<(&CallTransfers<'_, '_, '_>, &mut Writer<'_, '_>)>()
        + 12 * size_of::<usize>()
        + 8 * size_of::<&()>()
}

#[cfg(test)]
#[path = "original_semantic_mir_invocation_body_v34_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "original_semantic_mir_classified_production_v40_tests.rs"]
mod classified_tests;
