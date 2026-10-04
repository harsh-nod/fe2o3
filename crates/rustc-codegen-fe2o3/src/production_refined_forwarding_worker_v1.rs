//! Consuming, wire-free final-F handoff. No default or publication conversion.
#[path = "production_refined_forwarding_wire_v1.rs"]
mod wire;
use super::*;
use crate::production_native_source_lineage_v1::{
    NativeSourceLineageErrorV1, PreparedErasedNativeSourceLineageV1, PreparedNativeSourceLineageV1,
    try_prepare_erased_native_source_lineage_v1, try_prepare_native_source_lineage_v1,
};
use crate::production_pipeline::native_checked_output_handoff_v1::{
    NativeOutputHandoffErrorV1, OutputOwnerV1, SourceInputsV1, check_source_inputs_v1,
};
use crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1 as Ranked;
use crate::production_worker_handoff::{
    PreparedProductionWorkerHandoff as Handoff,
    refined_forwarding_v1::{self as assembly, FinalWorkerAssemblyErrorV1, FinalWorkerInputsV1},
};
use fe2o3_kernel_ir::{
    FormalMemoryObligations, FormalMemoryReceiptErrorV1,
    InertCanonicalKernelIrContractCatalogV1 as Catalog, InertFormalMemoryReceiptFormatV4 as Formal,
    MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1,
};
pub(crate) use wire::{
    PreparedRefinedForwardingWireV1, RefinedForwardingWireErrorV1, RefinedForwardingWireStorageV1,
};

#[derive(Debug)]
pub(crate) enum RefinedForwardingWorkerErrorV1 {
    Resource(Resource),
    Source(Box<NativeSourceLineageErrorV1>),
    SourceJoin(Box<NativeOutputHandoffErrorV1>),
    Assembly(Box<FinalWorkerAssemblyErrorV1>),
    Formal(Box<FormalMemoryReceiptErrorV1>),
    Mismatch(&'static str),
}
impl fmt::Display for RefinedForwardingWorkerErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RefinedForwardingWorkerErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Source(error) => Some(error.as_ref()),
            Self::SourceJoin(error) => Some(error.as_ref()),
            Self::Assembly(error) => Some(error.as_ref()),
            Self::Formal(error) => Some(error.as_ref()),
            Self::Mismatch(_) => None,
        }
    }
}
type E = RefinedForwardingWorkerErrorV1;
fn worker(value: E) -> ProductionPipelineError {
    super::error(RefinedForwardingNativeStageErrorV1::Worker(Box::new(value)))
}
fn resource(value: Resource) -> ProductionPipelineError {
    worker(E::Resource(value))
}
fn source(value: NativeSourceLineageErrorV1) -> ProductionPipelineError {
    worker(E::Source(Box::new(value)))
}
fn join(value: NativeOutputHandoffErrorV1) -> ProductionPipelineError {
    worker(E::SourceJoin(Box::new(value)))
}
fn mismatch(detail: &'static str) -> ProductionPipelineError {
    worker(E::Mismatch(detail))
}

#[allow(
    clippy::large_enum_variant,
    reason = "one moved original ranked/source lineage"
)]
enum SourceLineage {
    Direct(PreparedNativeSourceLineageV1),
    Erased(PreparedErasedNativeSourceLineageV1),
}
impl SourceLineage {
    fn ranked(&self) -> &Ranked {
        match self {
            Self::Direct(v) => v.ranked(),
            Self::Erased(v) => v.ranked(),
        }
    }
    fn catalog(&self) -> &Catalog {
        match self {
            Self::Direct(v) => v.proof().source().catalog(),
            Self::Erased(v) => v.proof().source().catalog(),
        }
    }
    fn replay_inputs<'a>(&'a self, budget: &mut Budget<'_>) -> Result<SourceInputsV1<'a>> {
        match self {
            Self::Direct(v) => {
                let replay = v.proof().source();
                let source = replay.source();
                source
                    .verify_equivalence_with_budget_v1(budget)
                    .map_err(|error| join(NativeOutputHandoffErrorV1::Source(error)))?;
                Ok(SourceInputsV1 {
                    semantic: source.semantic().semantic(),
                    original: source
                        .pre_ranked_executable()
                        .ok_or_else(|| mismatch("replayed original N"))?,
                    erased: None,
                    launch: source
                        .source_launch_roster()
                        .ok_or_else(|| mismatch("replayed source launch"))?,
                    catalog: replay.catalog(),
                })
            }
            Self::Erased(v) => {
                let replay = v.proof().source();
                let source = replay.source();
                source
                    .verify_equivalence(budget)
                    .map_err(|error| join(NativeOutputHandoffErrorV1::Source(error)))?;
                Ok(SourceInputsV1 {
                    semantic: source.original_source().semantic_ssa().source_semantic(),
                    original: source.original_source().executable(),
                    erased: Some(source.erased()),
                    launch: source.original_source().source_launch(),
                    catalog: replay.catalog(),
                })
            }
        }
    }
}

fn final_owner(native: &PreparedRefinedForwardingNativeOutputV1) -> FinalOwnerV1<'_> {
    match &native.owner {
        Composed::Direct(v) => FinalOwnerV1::Direct(v),
        Composed::Erased(v) => FinalOwnerV1::Erased(v),
    }
}
fn actual_source<'a>(
    native: &'a PreparedRefinedForwardingNativeOutputV1,
    catalog: &'a Catalog,
) -> Result<SourceInputsV1<'a>> {
    match &native.owner {
        Composed::Direct(v) => {
            OutputOwnerV1::Direct8(v.prefix().prefix().prefix().prefix().prefix())
        }
        Composed::Erased(v) => {
            OutputOwnerV1::Erased8(v.prefix().prefix().prefix().prefix().prefix())
        }
    }
    .source(catalog)
    .map_err(join)
}
fn source_header(native: &PreparedRefinedForwardingNativeOutputV1) -> usize {
    match &native.owner {
        Composed::Direct(_) => size_of::<PreparedNativeSourceLineageV1>(),
        Composed::Erased(_) => size_of::<PreparedErasedNativeSourceLineageV1>(),
    }
}
fn check_source(
    native: &PreparedRefinedForwardingNativeOutputV1,
    source: &SourceLineage,
    bindings: &AuthenticatedProductionBindings,
    budget: &mut Budget<'_>,
) -> Result<()> {
    scoped(native.retained_floor, budget, |budget| {
        let headers = size_of::<SourceInputsV1<'static>>()
            .checked_mul(2)
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        budget.reserve_storage(headers).map_err(resource)?;
        let original = actual_source(native, source.catalog())?;
        bindings
            .context_entries
            .validate_source(original.semantic)
            .map_err(|error| join(NativeOutputHandoffErrorV1::Context(Box::new(error))))?;
        check_source_inputs_v1(original, source.replay_inputs(budget)?, budget).map_err(join)?;
        if let SourceLineage::Erased(value) = source {
            value
                .check_output_catalog_v1(source.catalog(), budget)
                .map_err(source_error)?;
        }
        Ok(())
    })
}
fn prepare_source(
    native: &PreparedRefinedForwardingNativeOutputV1,
    ranked: Ranked,
    budget: &mut Budget<'_>,
) -> Result<(SourceLineage, usize)> {
    match &native.owner {
        Composed::Direct(v) => {
            let source_owner = v
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .source_semantic_kir();
            let (value, receipt) =
                try_prepare_native_source_lineage_v1(source_owner, ranked, budget)
                    .map_err(source)?;
            Ok((SourceLineage::Direct(value), receipt.retained_storage()))
        }
        Composed::Erased(v) => {
            let source_owner = v
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .erased_source();
            let (value, receipt) =
                try_prepare_erased_native_source_lineage_v1(source_owner, ranked, budget)
                    .map_err(source)?;
            Ok((SourceLineage::Erased(value), receipt.retained_storage()))
        }
    }
}
fn reports(native: &PreparedRefinedForwardingNativeOutputV1) -> &[FormalMemoryObligations] {
    match &native.owner {
        Composed::Direct(v) => v.kernels(),
        Composed::Erased(v) => v.kernels(),
    }
}
fn exact(a: &[u8], b: &[u8], detail: &'static str, budget: &mut Budget<'_>) -> Result<()> {
    budget
        .charge_work(
            a.len()
                .checked_add(b.len())
                .and_then(|n| n.checked_add(1))
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )
        .map_err(resource)?;
    if a != b {
        return Err(mismatch(detail));
    }
    Ok(())
}

fn copy_text(text: &str, budget: &mut Budget<'_>) -> Result<String> {
    budget
        .charge_work(
            text.len()
                .checked_add(1)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        )
        .map_err(resource)?;
    budget.reserve_storage(text.len()).map_err(resource)?;
    let mut copy = String::new();
    copy.try_reserve_exact(text.len())
        .map_err(|_| resource(Resource::Allocation))?;
    budget
        .reserve_storage(
            copy.capacity()
                .checked_sub(text.len())
                .ok_or_else(|| resource(Resource::Accounting))?,
        )
        .map_err(resource)?;
    copy.push_str(text);
    Ok(copy)
}

/// Returned storage is an unreserved logical codec-domain addition, not a
/// measured allocator capacity. New input String backing is paid separately by
/// actual capacity. Reused descriptor/FFI/V2 internals keep their existing opaque
/// scratch exclusions. The original bare LLVM remains live in `native`.
fn prepare_handoff(
    native: &PreparedRefinedForwardingNativeOutputV1,
    catalog: &Catalog,
    typed_roots: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    source_envelope: Option<&fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
    budget: &mut Budget<'_>,
) -> Result<(Handoff, usize)> {
    scoped(native.retained_floor, budget, |budget| {
        budget.charge_work(3).map_err(resource)?;
        let text = dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES;
        let descriptor = fe2o3_kernel_descriptor::MAX_DESCRIPTOR_TABLE_BYTES;
        let handoff = fe2o3_compiler_ffi::MAX_COMPILER_MODULE_HANDOFF_BYTES_V2;
        if native.llvm.len() > text {
            return Err(mismatch("bounded final F compiler text"));
        }
        let retained = handoff
            .checked_add(descriptor)
            .and_then(|n| n.checked_add(size_of::<Handoff>()))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        let scratch = text
            .checked_mul(3)
            .and_then(|n| n.checked_add(descriptor))
            .and_then(|n| n.checked_add(retained))
            .and_then(|n| n.checked_add(size_of::<FinalWorkerInputsV1<'static>>()))
            .and_then(|n| n.checked_add(size_of::<String>()))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        budget.reserve_storage(scratch).map_err(resource)?;
        let llvm = copy_text(&native.llvm, budget)?;
        let prepared = assembly::prepare_v1(
            FinalWorkerInputsV1 {
                owner: final_owner(native),
                catalog,
                profile: native.profile,
                typed_roots,
                source_envelope: source_envelope.cloned(),
            },
            llvm,
            budget,
        )
        .map_err(|error| worker(E::Assembly(Box::new(error))))?;
        budget.charge_work(1).map_err(resource)?;
        Ok((prepared, retained))
    })
}

fn formal_rows(
    native: &PreparedRefinedForwardingNativeOutputV1,
    budget: &mut Budget<'_>,
) -> Result<(Vec<Vec<u8>>, usize)> {
    scoped(native.retained_floor, budget, |budget| {
        budget
            .reserve_storage(size_of::<Vec<Vec<u8>>>())
            .map_err(resource)?;
        let reports = reports(native);
        if reports.len() != native.output().module().kernels.len() {
            return Err(mismatch("complete final F formal roster"));
        }
        let requested = reports
            .len()
            .checked_mul(size_of::<Vec<u8>>())
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        budget.reserve_storage(requested).map_err(resource)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(reports.len())
            .map_err(|_| resource(Resource::Allocation))?;
        let actual = rows
            .capacity()
            .checked_mul(size_of::<Vec<u8>>())
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        budget
            .reserve_storage(
                actual
                    .checked_sub(requested)
                    .ok_or_else(|| resource(Resource::Accounting))?,
            )
            .map_err(resource)?;
        let mut retained = actual
            .checked_add(size_of::<Vec<Vec<u8>>>())
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        for (report, kernel) in reports.iter().zip(&native.output().module().kernels) {
            budget
                .charge_work(
                    report
                        .kernel()
                        .as_str()
                        .len()
                        .checked_add(kernel.id.as_str().len())
                        .and_then(|n| n.checked_add(1))
                        .ok_or_else(|| resource(Resource::Arithmetic))?,
                )
                .map_err(resource)?;
            if report.kernel() != &kernel.id {
                return Err(mismatch("ordered final F formal identity"));
            }
            // The codec keeps its existing bounded scratch domain. Our retained
            // copy is distinct: requested and excess capacity are paid before use.
            let temporary = MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1
                .checked_add(size_of::<Formal>())
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            budget.reserve_storage(temporary).map_err(resource)?;
            let formal = Formal::from_current_obligations(report)
                .map_err(|error| worker(E::Formal(Box::new(error))))?;
            let payload = formal.canonical_bytes();
            if payload.len() > MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1 {
                return Err(mismatch("bounded final F formal payload"));
            }
            budget.charge_work(payload.len()).map_err(resource)?;
            budget.reserve_storage(payload.len()).map_err(resource)?;
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(payload.len())
                .map_err(|_| resource(Resource::Allocation))?;
            budget
                .reserve_storage(
                    bytes
                        .capacity()
                        .checked_sub(payload.len())
                        .ok_or_else(|| resource(Resource::Accounting))?,
                )
                .map_err(resource)?;
            bytes.extend_from_slice(payload);
            drop(formal);
            budget.release_storage(temporary).map_err(resource)?;
            retained = retained
                .checked_add(bytes.capacity())
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            rows.push(bytes);
        }
        Ok((rows, retained))
    })
}

fn check_formal_rows(
    native: &PreparedRefinedForwardingNativeOutputV1,
    actual: &[Vec<u8>],
    budget: &mut Budget<'_>,
) -> Result<()> {
    scoped(native.retained_floor, budget, |budget| {
        let (fresh, storage) = formal_rows(native, budget)?;
        budget.reserve_storage(storage).map_err(resource)?;
        if actual.len() != fresh.len() {
            return Err(mismatch("complete final F formal payloads"));
        }
        for (actual, expected) in actual.iter().zip(&fresh) {
            exact(actual, expected, "fresh final F formal payload", budget)?;
        }
        drop(fresh);
        budget.release_storage(storage).map_err(resource)?;
        Ok(())
    })
}

/// Owns the complete live compiler history and independently signed original
/// source once. No decoder, legacy worker conversion, Clone, or authority grant.
/// New wrapper/row handles and new String/formal Vec capacities are prepaid;
/// reused opaque descriptor/FFI codecs retain their bounded-engine exclusions.
pub(crate) struct PreparedRefinedForwardingWorkerHandoffV1 {
    native: PreparedRefinedForwardingNativeOutputV1,
    history: PreparedRefinedForwardingHistoryClaimsV1,
    bindings: AuthenticatedProductionBindings,
    source: SourceLineage,
    handoff: Handoff,
    formal: Vec<Vec<u8>>,
    retained_floor: usize,
}
#[derive(Clone, Copy)]
pub(crate) struct RefinedForwardingWorkerStorageV1(usize);
impl RefinedForwardingWorkerStorageV1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}
fn wrapper_header(native: &PreparedRefinedForwardingNativeOutputV1) -> Result<usize> {
    let moved_source = source_header(native)
        .checked_sub(size_of::<Ranked>())
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    size_of::<PreparedRefinedForwardingWorkerHandoffV1>()
        .checked_sub(size_of::<RefinedForwardingNativeProductionCompilationV1>())
        .and_then(|n| n.checked_sub(moved_source))
        .and_then(|n| n.checked_sub(size_of::<Handoff>()))
        .and_then(|n| n.checked_sub(size_of::<Vec<Vec<u8>>>()))
        .ok_or_else(|| resource(Resource::Arithmetic))
}

impl RefinedForwardingNativeProductionCompilationV1 {
    /// Consumes the genuine complete compiler owner. Missing signed source
    /// receipts refuse through the unchanged source-lineage constructor.
    /// The returned addition is unreserved; pay it before further controlled use.
    pub(crate) fn prepare_refined_forwarding_worker_handoff_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<(
        PreparedRefinedForwardingWorkerHandoffV1,
        RefinedForwardingWorkerStorageV1,
    )> {
        scoped(self.retained_floor, budget, |budget| {
            self.verify_equivalence(budget)?;
            let floor = budget.storage();
            budget
                .reserve_storage(wrapper_header(&self.native)?)
                .map_err(resource)?;
            let Self {
                native,
                history,
                ranked_verification,
                bindings,
                ..
            } = self;
            let (source, storage) = prepare_source(&native, ranked_verification, budget)?;
            budget.reserve_storage(storage).map_err(resource)?;
            check_source(&native, &source, &bindings, budget)?;
            let (handoff, storage) = prepare_handoff(
                &native,
                source.catalog(),
                &bindings.typed_descriptor_roots,
                bindings.transaction.compiler_ffi_envelope.as_ref(),
                budget,
            )?;
            budget.reserve_storage(storage).map_err(resource)?;
            let (formal, storage) = formal_rows(&native, budget)?;
            budget.reserve_storage(storage).map_err(resource)?;
            let value = PreparedRefinedForwardingWorkerHandoffV1 {
                native,
                history,
                bindings,
                source,
                handoff,
                formal,
                retained_floor: budget.storage(),
            };
            value.verify_equivalence(budget)?;
            let retained = budget
                .storage()
                .checked_sub(floor)
                .ok_or_else(|| resource(Resource::Accounting))?;
            Ok((value, RefinedForwardingWorkerStorageV1(retained)))
        })
    }
}
fn source_error(error: NativeSourceLineageErrorV1) -> ProductionPipelineError {
    source(error)
}

impl PreparedRefinedForwardingWorkerHandoffV1 {
    pub(crate) fn output(&self) -> &Graph {
        self.native.output()
    }
    pub(crate) fn original(&self) -> Result<&Graph> {
        self.native.original()
    }
    pub(crate) fn history_inputs(
        &self,
    ) -> fe2o3_kernel_opt::CanonicalRefinedForwardingHistoryInputsV1<'_> {
        self.history.inputs(&self.native)
    }
    pub(crate) fn native_output_parts(
        &self,
    ) -> (
        &fe2o3_compiler_ffi::CompilerModuleHandoffV2,
        &fe2o3_compiler_ffi::CompilerDescriptorSourceV1,
        &[u8; 32],
    ) {
        self.handoff.native_output_parts_v1()
    }
    pub(crate) fn final_formal_payloads(&self) -> &[Vec<u8>] {
        &self.formal
    }
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_floor
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    pub(crate) fn verify_equivalence(&self, budget: &mut Budget<'_>) -> Result<()> {
        scoped(self.retained_floor, budget, |budget| {
            if self
                .bindings
                .rustc_preflight_plan
                .rustc_identity_inventory_sha256()
                != self.bindings.rustc_identity_inventory.sha256()
            {
                return Err(ProductionPipelineError::RustcLineageMismatch);
            }
            if self.native.profile != self.bindings.rustc_target.profile()
                || self.source.ranked().root_count() != self.output().module().kernels.len()
                || !self
                    .source
                    .ranked()
                    .every_functional_verification_is_coherent()
            {
                return Err(mismatch("complete final F target/ranked custody"));
            }
            let checked = self.history.check(&self.native, budget)?;
            let storage = checked.retained_storage();
            budget.reserve_storage(storage).map_err(resource)?;
            drop(checked);
            budget.release_storage(storage).map_err(resource)?;
            check_source(&self.native, &self.source, &self.bindings, budget)?;
            validate_final_descriptor_evidence_v1(
                final_owner(&self.native),
                &self.bindings.typed_descriptor_roots,
                self.native.profile,
                budget,
            )
            .map_err(|error| {
                worker(E::Assembly(Box::new(
                    FinalWorkerAssemblyErrorV1::Descriptor(Box::new(error)),
                )))
            })?;
            self.native.verify_equivalence(budget)?;
            assembly::replay_v1(
                final_owner(&self.native),
                self.source.catalog(),
                self.native.profile,
                &self.handoff,
                budget,
            )
            .map_err(|error| worker(E::Assembly(Box::new(error))))?;
            let (expected, storage) = prepare_handoff(
                &self.native,
                self.source.catalog(),
                &self.bindings.typed_descriptor_roots,
                self.bindings.transaction.compiler_ffi_envelope.as_ref(),
                budget,
            )?;
            budget.reserve_storage(storage).map_err(resource)?;
            let (actual_module, actual_descriptor, actual_digest) =
                self.handoff.native_output_parts_v1();
            let (module, descriptor, digest) = expected.native_output_parts_v1();
            exact(
                actual_module.canonical_bytes(),
                module.canonical_bytes(),
                "exact final F V2/FFI/symbols",
                budget,
            )?;
            exact(
                actual_descriptor.canonical_bytes(),
                descriptor.canonical_bytes(),
                "exact final F typed descriptor",
                budget,
            )?;
            exact(actual_digest, digest, "exact final F LLVM digest", budget)?;
            drop(expected);
            budget.release_storage(storage).map_err(resource)?;
            check_formal_rows(&self.native, &self.formal, budget)?;
            budget.charge_work(1).map_err(resource)?;
            Ok(())
        })
    }
}

#[cfg(test)]
#[path = "production_refined_forwarding_worker_v1_tests.rs"]
mod tests;
