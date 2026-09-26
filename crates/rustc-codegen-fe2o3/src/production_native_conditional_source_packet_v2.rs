//! Same-visit producer transport followed by independent, content-only replay.
//! Original source custody stays in the ranked phase; no ordinary/F authority.
use super::{
    Budget, Induction, NativeSourceLineageErrorV1, PreparedNativeSourceProofPacketV1, Resource,
    owned_packet, packet,
};
use crate::compiler_descriptor::TypedDescriptorRootV1;
use crate::production_ranked_projection_v1::{
    ProductionRankedRootProgramV1 as Root, ProductionRankedSemanticProgramV1 as Ranked,
    ProductionRankedVerificationErrorV1,
};
use fe2o3_kernel_descriptor::KernelId;
use fe2o3_lower_mir_kernel::{
    ProductionHelperSourcePolicyV1, ProductionPreRankedKirOwnerV1 as Source,
    ProductionSourceLaunchInputV1, ProductionSourceLaunchRootInputV1,
    encode_production_ranked_source_rows_v1,
};
use fe2o3_verifier::{
    NativeConditionalRootPolicyV2, NativeConditionalSourcePacketInputV2,
    NativeConditionalSourceRootV2, ReplayedNativeConditionalSourceV2 as Proof,
    encode_native_conditional_source_packet_v2, validate_native_conditional_source_packet_v2,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

pub(crate) type PreparedConditionalSourcePacketV2 = PreparedNativeSourceProofPacketV1<Proof>;

#[derive(Debug)]
pub(crate) enum ConditionalPacketErrorV2 {
    Resource(Resource),
    SourcePhase(ProductionRankedVerificationErrorV1),
    Lineage(NativeSourceLineageErrorV1),
    Recipe(fe2o3_pliron::ProductionRankedRecipeWireErrorV1),
    Rows(fe2o3_lower_mir_kernel::ProductionRankedSourceRowsWireErrorV1),
    Induction(fe2o3_mir_model::SemanticU32InductionEvidenceErrorV1),
    Session(fe2o3_pliron::ProductionSessionErrorV1),
    Packet(fe2o3_verifier::NativeConditionalPacketErrorV2),
    Replay(fe2o3_verifier::NativeConditionalSourceProofErrorV2),
    Mismatch(&'static str),
}
type E = ConditionalPacketErrorV2;
impl From<Resource> for E {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "conditional source packet: {self:?}")
    }
}
impl std::error::Error for E {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(match self {
            Self::Resource(e) => e,
            Self::SourcePhase(e) => e,
            Self::Lineage(e) => e,
            Self::Recipe(e) => e,
            Self::Rows(e) => e,
            Self::Induction(e) => e,
            Self::Session(e) => e,
            Self::Packet(e) => e,
            Self::Replay(e) => e,
            Self::Mismatch(_) => return None,
        })
    }
}

/// Consumes only the existing roster handle. Both returned owners stay paid on
/// their original accounts; this is NOT the ordinary unreserved packet API.
/// No opaque C1 failure or unwind is wrapped in an unconditional refund scope.
pub(crate) fn prepare_retained_native_conditional_source_packet_v2(
    ranked: Ranked,
    descriptors: &[TypedDescriptorRootV1],
    target: &mut Budget<'_>,
) -> Result<(Ranked, PreparedConditionalSourcePacketV2), E> {
    retain(target, |target| {
        let (ranked, result) = ranked
            .with_conditional_producer_inputs_v2(|source, roots, source_budget| {
                // Preserve the nested Result until the owning source postcheck.
                Ok(
                    if source_budget.work_ledger_identity_v1() == target.work_ledger_identity_v1() {
                        Err(Resource::Accounting.into())
                    } else {
                        assemble(source, roots, descriptors, target)
                    },
                )
            })
            .map_err(E::SourcePhase)?;
        let packet = result?;
        let retained = packet.retained_storage().map_err(E::Lineage)?;
        Ok(((ranked, packet), retained))
    })
}

// Refund only known scratch on successful transfer. Unknown inner accounting
// failures and panics deliberately retain their terminal reservations.
fn retain<'w, T>(
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Budget<'w>) -> Result<(T, usize), E>,
) -> Result<T, E> {
    let floor = budget.storage();
    let account = budget.work_ledger_identity_v1();
    let address = budget as *const Budget<'_> as usize;
    let result = catch_unwind(AssertUnwindSafe(|| run(budget)));
    let intact = budget.work_ledger_identity_v1() == account
        && budget as *const Budget<'_> as usize == address
        && budget.storage() >= floor;
    match result {
        Err(payload) => resume_unwind(payload),
        Ok(result) if !intact => {
            drop(result);
            Err(Resource::Accounting.into())
        }
        Ok(Err(error)) => Err(error),
        Ok(Ok((value, retained))) => {
            let Some(scratch) = (budget.storage() - floor).checked_sub(retained) else {
                drop(value);
                return Err(Resource::Accounting.into());
            };
            if let Err(error) = budget.release_storage(scratch) {
                drop(value);
                return Err(error.into());
            }
            Ok(value)
        }
    }
}

struct Payload {
    induction: Induction,
    recipe: Vec<u8>,
    rows: Vec<u8>,
}

fn assemble(
    source: &Source,
    roots: &[Root],
    descriptors: &[TypedDescriptorRootV1],
    budget: &mut Budget<'_>,
) -> Result<PreparedConditionalSourcePacketV2, E> {
    let semantic = source.semantic_ssa().source_semantic();
    let launches = source.source_launch().roots();
    budget.charge_work(5)?;
    if source.helper_source_policy_v1() != ProductionHelperSourcePolicyV1::RawEmpty
        || roots.is_empty()
        || roots.len() > fe2o3_compiler_lineage::MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3
        || roots.len() != semantic.roots().len()
        || roots.len() != launches.len()
        || roots.len() != descriptors.len()
        || roots.len() != source.executable().module().kernels.len()
    {
        return Err(E::Mismatch("complete original Direct conditional roster"));
    }
    budget.reserve_storage(owned_packet::packet_header::<Proof>().map_err(E::Lineage)?)?;
    let (native_module, _) = packet::encode_original_native_envelope_v1(
        *semantic.semantic_sha256().as_bytes(),
        source.executable(),
        budget,
    )
    .map_err(E::Lineage)?;
    let mut payloads = vector(roots.len(), budget)?;
    for (ordinal, root) in roots.iter().enumerate() {
        let launch = launches[ordinal];
        let descriptor = &descriptors[ordinal];
        let function = &semantic.functions()[semantic.roots()[ordinal].index() as usize];
        let input = root
            .conditional_producer_inputs_v2()
            .ok_or(E::Mismatch("missing retained conditional transport"))?;
        budget.charge_work(
            root.logical_name()
                .len()
                .checked_add(descriptor.logical_name().len())
                .and_then(|n| n.checked_add(root.export_symbol().len()))
                .and_then(|n| n.checked_add(descriptor.entry_symbol().len()))
                .and_then(|n| n.checked_add(200))
                .ok_or(Resource::Arithmetic)?,
        )?;
        if root.semantic_root() != semantic.roots()[ordinal]
            || root.semantic_root_identity() != function.identity()
            || launch.selected_root() != root.semantic_root()
            || launch.semantic_root_identity() != root.semantic_root_identity()
            || launch.kernel_binding() != *root.kernel_binding()
            || descriptor.kernel_binding_bytes() != *root.kernel_binding()
            || descriptor.logical_name() != root.logical_name()
            || descriptor.entry_symbol().as_bytes() != root.export_symbol()
            || root.source_rank() != launch.source_rank()
            || input.input.semantic_root != root.semantic_root().index()
            || input.input.launch_rank != root.source_rank()
        {
            return Err(E::Mismatch(
                "ordered original root/collector/source identities",
            ));
        }
        require_launch(descriptor, launch.source_launch())?;
        let kernel = input.input.pending.kernel().map_err(E::Session)?;
        let induction = encode_induction(root.semantic_u32_induction(), budget)?;
        let (recipe, storage) =
            fe2o3_pliron::encode_production_ranked_recipe_v1(kernel, budget).map_err(E::Recipe)?;
        budget.reserve_storage(storage.retained_storage())?;
        let (rows, storage) = encode_production_ranked_source_rows_v1(
            &input.input.access_sources,
            &input.input.executable_effect_sources,
            budget,
        )
        .map_err(E::Rows)?;
        budget.reserve_storage(storage.retained_storage())?;
        payloads.push(Payload {
            induction,
            recipe,
            rows,
        });
    }
    let mut rows = vector(roots.len(), budget)?;
    let mut policies = vector(roots.len(), budget)?;
    for (ordinal, (root, payload)) in roots.iter().zip(&payloads).enumerate() {
        budget.charge_work(2)?;
        let input = root
            .conditional_producer_inputs_v2()
            .ok_or(E::Mismatch("conditional custody changed"))?;
        rows.push(NativeConditionalSourceRootV2 {
            semantic_root: root.semantic_root().index(),
            launch_rank: root.source_rank(),
            launch: ProductionSourceLaunchRootInputV1::new(
                root.logical_name(),
                *root.kernel_binding(),
                launches[ordinal].source_launch(),
            ),
            induction_bytes: payload.induction.canonical_bytes(),
            recipe_bytes: &payload.recipe,
            source_rows_bytes: &payload.rows,
            ranked_ir: &input.input.ranked_ir,
            cpu_input_bytes: input.transport.cpu_input_v1(),
            staging_commitments: input.transport.staging_commitments_v1(),
            effect_receipts: input.effect_receipts,
            formula_receipt: input.transport.formula_receipt_v2(),
        });
        policies.push(NativeConditionalRootPolicyV2 {
            semantic_root: root.semantic_root().index(),
            effects: input.effect_policy,
            formula: input.formula_policy,
        });
    }
    let order = canonical_order(roots.len(), |i| *roots[i].kernel_binding(), budget)?;
    let (source_packet, storage) = encode_native_conditional_source_packet_v2(
        NativeConditionalSourcePacketInputV2 {
            semantic_mir: semantic.canonical_encoding(),
            native_module: &native_module,
            canonical_kernel_order: &order,
            roots: &rows,
        },
        budget,
    )
    .map_err(E::Packet)?;
    budget.reserve_storage(storage.retained_storage())?;
    let (proof, storage) =
        validate_native_conditional_source_packet_v2(&source_packet, &policies, budget)
            .map_err(E::Replay)?;
    budget.reserve_storage(storage.retained_storage())?;
    let retained = owned_packet::packet_header::<Proof>()
        .map_err(E::Lineage)?
        .checked_add(storage.retained_storage())
        .and_then(|n| n.checked_add(native_module.capacity()))
        .and_then(|n| n.checked_add(source_packet.capacity()))
        .ok_or(Resource::Arithmetic)?;
    // Borrowed views and encoded scratch die before retain() releases scratch.
    PreparedNativeSourceProofPacketV1::from_parts(packet::NativeSourcePacketPartsV1 {
        proof,
        native_module,
        source_packet,
        retained,
    })
    .map_err(E::Lineage)
}

fn require_launch(
    descriptor: &TypedDescriptorRootV1,
    source: ProductionSourceLaunchInputV1,
) -> Result<(), E> {
    let launch = descriptor
        .source_launch()
        .ok_or(E::Mismatch("original collector launch"))?;
    let workgroup = match launch.block_size() {
        fe2o3_artifacts::BlockSize::Exact(v) => Some([v.x(), v.y(), v.z()]),
        _ => None,
    };
    let grid = launch.max_grid();
    if source
        != ProductionSourceLaunchInputV1::new(
            launch.rank(),
            workgroup,
            [grid.x(), grid.y(), grid.z()],
        )
    {
        return Err(E::Mismatch("exact original collector launch"));
    }
    Ok(())
}

fn encode_induction(
    report: &fe2o3_mir_model::SemanticU32InductionNoOverflowReportV1,
    budget: &mut Budget<'_>,
) -> Result<Induction, E> {
    // The inherited codec holds two certificate arrays and two byte arrays.
    // Its fixed wire fields fit the public evidence types' in-memory extents;
    // prepay those four type-sized extents before entering that bounded codec.
    let one = report
        .certificates()
        .len()
        .checked_mul(size_of::<
            fe2o3_mir_model::SemanticU32InductionNoOverflowCertificateEvidenceV1,
        >())
        .and_then(|n| n.checked_add(size_of::<Induction>()))
        .ok_or(Resource::Arithmetic)?;
    let prepaid = one.checked_mul(4).ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(prepaid)?;
    budget.charge_work(prepaid)?;
    let induction = Induction::from_report(report).map_err(E::Induction)?;
    if induction.canonical_bytes().len() > one {
        return Err(Resource::Accounting.into());
    }
    Ok(induction)
}

fn vector<T>(count: usize, budget: &mut Budget<'_>) -> Result<Vec<T>, E> {
    budget.charge_work(3)?;
    let requested = count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(
        requested
            .checked_add(size_of::<Vec<T>>())
            .ok_or(Resource::Arithmetic)?,
    )?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    let actual = values
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(actual.checked_sub(requested).ok_or(Resource::Accounting)?)?;
    Ok(values)
}

fn canonical_order(
    count: usize,
    binding: impl Fn(usize) -> [u8; 32],
    budget: &mut Budget<'_>,
) -> Result<Vec<u32>, E> {
    let mut order = vector(count, budget)?;
    for index in 0..count {
        budget.charge_work(1)?;
        order.push(u32::try_from(index).map_err(|_| Resource::Arithmetic)?);
        let mut slot = index;
        while slot != 0 {
            budget.charge_work(33)?;
            let a = KernelId::from_bytes(binding(order[slot - 1] as usize));
            let b = KernelId::from_bytes(binding(order[slot] as usize));
            match a.cmp(&b) {
                std::cmp::Ordering::Equal => return Err(E::Mismatch("duplicate kernel binding")),
                std::cmp::Ordering::Less => break,
                std::cmp::Ordering::Greater => order.swap(slot - 1, slot),
            }
            slot -= 1;
        }
    }
    Ok(order)
}

#[cfg(test)]
#[path = "production_native_conditional_source_packet_v2_tests.rs"]
mod tests;
