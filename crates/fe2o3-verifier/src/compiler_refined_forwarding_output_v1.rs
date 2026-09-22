//! Independent inert final-F wire agreement with an already admitted signed source.
//! Capability projection derives its own actual-F reachability allowances;
//! exact embedded descriptor bytes alone do not substitute for that check.
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

use crate::{
    ValidatedNativeCompilerRankedSourceProofV1 as Direct,
    ValidatedNativeCompilerUnitLocalErasedSourceProofV1 as Erased,
};
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_amdgcn_model::{
    DescriptorCapabilityProjectionErrorV1, NativeV12TextDescriptorReplayErrorV1,
    ProductionTargetCoordinateErrorV1, check_canonical_v12_descriptor_capabilities_v1,
    check_native_v12_text_descriptor_relation_v1,
    check_production_target_coordinate_preservation_v1,
};
use fe2o3_compiler_ffi::{
    CompilerDescriptorSourceErrorV1, CompilerDescriptorSourceV1 as Descriptor,
    CompilerModuleHandoffErrorV2, CompilerModuleHandoffV2 as Native,
    INERT_REFINED_FORWARDING_HASH_STORAGE_V1 as HASH_STORAGE,
    INERT_REFINED_FORWARDING_READ_STORAGE_V1 as READ_STORAGE,
    InertRefinedForwardingContentIdentityV1 as Identity,
    InertRefinedForwardingOutputErrorV1 as FrameError,
    InertRefinedForwardingOutputFieldV1 as Field, InertRefinedForwardingOutputRefV1 as Frame,
    InertRefinedForwardingRouteV1 as Route, MAX_INERT_REFINED_FORWARDING_STORAGE_V1 as MAX_STORAGE,
    inert_refined_forwarding_history_identity_v1, inert_refined_forwarding_output_identity_v1,
};
use fe2o3_compiler_lineage::{
    InertNativeNeutralSubjectV1 as Subject, InertProofBindingAssociationErrorV4,
    InertProofBindingAssociationV4 as Association, LineageErrorV3, MultiRootProofRosterErrorV3,
    MultiRootProofRosterTranscriptV3 as Roster, NativeNeutralModuleErrorV1,
    NativeNeutralModuleRefV1, NativeNeutralSubjectErrorV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, FormalMemoryReceiptErrorV1,
    InertCanonicalKernelIrContractCatalogV1 as Catalog,
    VerifiedCanonicalKernelIrModuleV12 as Graph,
};
use fe2o3_kernel_opt::{
    CanonicalRefinedForwardingHistoryErrorV1, CanonicalRefinedForwardingHistoryLimitsV1 as Limits,
    DecodedRefinedForwardingHistoryV1 as History, RefinedForwardingHistoryRoleV1 as Role,
};
use fe2o3_lower_mir_kernel::{
    CanonicalOutputFormalSourceAnchorV1 as Anchor, CanonicalOutputGuardedFormalMemoryErrorV1,
    OriginalNativeFormalMemoryErrorV1, OriginalNativeFormalMemoryStorageV1,
    OriginalNativeFormalMemoryV1, ProductionSemanticKirErrorV1, ProductionSourceLaunchRosterV1,
    analyze_canonical_output_guarded_formal_memory_v1, analyze_original_native_formal_memory_v1,
    analyze_original_unit_local_formal_memory_v1,
};
use fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1 as Semantic;

#[path = "compiler_refined_forwarding_output_joins_v1.rs"]
mod joins;

#[path = "compiler_refined_forwarding_recovery_v1.rs"]
mod recovery;
pub use recovery::{
    RecoveredCompilerRefinedForwardingOutputV1, RecoveredCompilerRefinedForwardingStorageV1,
    recover_compiler_refined_forwarding_output_v1,
};

/// Bytes cannot construct this input: both variants retain independently imported
/// signed ranked evidence and their connected original source owners.
#[derive(Clone, Copy)]
pub enum RefinedForwardingOriginalSourceProofV1<'s> {
    Direct(&'s Direct),
    Erased(&'s Erased),
}
type Source<'s> = RefinedForwardingOriginalSourceProofV1<'s>;

#[derive(Debug)]
pub enum CompilerRefinedForwardingOutputErrorV1 {
    Resource(Resource),
    Framing(FrameError<Resource>),
    History(CanonicalRefinedForwardingHistoryErrorV1),
    HistoryWire(fe2o3_kernel_opt::RefinedForwardingHistoryWireErrorV1),
    SourcePacket(crate::NativeCompilerSourceProofErrorV1),
    Source(ProductionSemanticKirErrorV1),
    OriginalFormal(OriginalNativeFormalMemoryErrorV1),
    FinalFormal(CanonicalOutputGuardedFormalMemoryErrorV1),
    NativeEnvelope(NativeNeutralModuleErrorV1),
    NativeSubject(NativeNeutralSubjectErrorV1),
    Roster(MultiRootProofRosterErrorV3),
    Association(InertProofBindingAssociationErrorV4),
    Lineage(LineageErrorV3),
    FormalReceipt(FormalMemoryReceiptErrorV1),
    Native(CompilerModuleHandoffErrorV2),
    Descriptor(CompilerDescriptorSourceErrorV1),
    Capabilities(DescriptorCapabilityProjectionErrorV1),
    Coordinates(ProductionTargetCoordinateErrorV1),
    TextDescriptor(NativeV12TextDescriptorReplayErrorV1),
    Mismatch(&'static str),
    Panicked,
}
type E = CompilerRefinedForwardingOutputErrorV1;
type R<T> = Result<T, E>;
impl From<Resource> for E {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "inert final-F/source agreement: {self:?}")
    }
}
impl std::error::Error for E {}

/// Only the new borrowed receipt header, returned unreserved. All input backing,
/// the complete source-proof receipt and decoded-history receipts remain paid by
/// the caller until the returned receipt is dropped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerRefinedForwardingOutputStorageV1(usize);
impl CompilerRefinedForwardingOutputStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}
type Storage = CompilerRefinedForwardingOutputStorageV1;

/// Move-only agreement, not execution, rustc ABI authentication or publication.
/// The signed source is supplied independently, never decoded from the frame.
///
/// ```compile_fail
/// use fe2o3_verifier::CheckedCompilerRefinedForwardingOutputV1 as Checked;
/// fn duplicate(value: Checked<'_, '_, '_>) { let _ = value.clone(); }
/// ```
///
/// ```compile_fail
/// use fe2o3_verifier::CheckedCompilerRefinedForwardingOutputV1 as Checked;
/// fn detach<'a>(value: Checked<'a, 'a, 'a>) -> Checked<'static, 'static, 'static> { value }
/// ```
///
/// ```compile_fail
/// use fe2o3_verifier::CheckedCompilerRefinedForwardingOutputV1 as Checked;
/// fn mutate(value: Checked<'_, '_, '_>) { value.output().module().kernels.clear(); }
/// ```
///
/// ```compile_fail
/// use fe2o3_verifier::CheckedCompilerRefinedForwardingOutputV1 as Checked;
/// fn manufacture() -> Checked<'static, 'static, 'static> { Checked::default() }
/// ```
pub struct CheckedCompilerRefinedForwardingOutputV1<'a, 'h, 'w> {
    frame: &'a Frame<'w>,
    history: &'a History<'h, 'w>,
    source: Source<'a>,
    identity: Identity,
    history_identity: Identity,
    limits: Limits,
    storage: Storage,
}
type Checked<'a, 'h, 'w> = CheckedCompilerRefinedForwardingOutputV1<'a, 'h, 'w>;
impl<'a, 'h, 'w> Checked<'a, 'h, 'w> {
    pub fn output(&self) -> &Graph {
        self.history.graph(Role::F)
    }
    pub const fn identity(&self) -> Identity {
        self.identity
    }
    pub const fn history_identity(&self) -> Identity {
        self.history_identity
    }
    pub const fn limits(&self) -> Limits {
        self.limits
    }
    pub const fn storage(&self) -> Storage {
        self.storage
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub const fn authenticates_execution(&self) -> bool {
        false
    }
    pub const fn authenticates_rustc_abi(&self) -> bool {
        false
    }
    pub fn replay(&self, budget: &mut Budget<'_>) -> R<()> {
        let required = input_floor(self.frame, self.history, self.source)?
            .checked_add(self.storage.0)
            .ok_or(Resource::Arithmetic)?;
        if budget.storage() < required {
            return Err(Resource::Accounting.into());
        }
        scoped(budget, |budget| {
            let (fresh, receipt) = check_compiler_refined_forwarding_output_v1(
                self.frame,
                self.history,
                self.source,
                budget,
            )?;
            budget.reserve_storage(receipt.0)?;
            budget.charge_work(
                size_of::<Limits>()
                    .checked_add(2 * size_of::<Identity>())
                    .and_then(|n| n.checked_add(1))
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if fresh.identity != self.identity
                || fresh.history_identity != self.history_identity
                || fresh.limits != self.limits
            {
                return Err(E::Mismatch("exact retained content identity and limits"));
            }
            drop(fresh);
            budget.release_storage(receipt.0)?;
            Ok(())
        })
    }
}

struct Inputs<'a> {
    semantic: &'a Semantic,
    original: &'a Graph,
    erased: Option<&'a Graph>,
    launch: &'a ProductionSourceLaunchRosterV1,
    catalog: &'a Catalog,
    middle: &'a Roster,
    correspondence: &'a Roster,
    verus: &'a Roster,
}
impl<'a> Source<'a> {
    fn route(self) -> Route {
        match self {
            Self::Direct(_) => Route::Direct,
            Self::Erased(_) => Route::Erased,
        }
    }
    fn anchor(self) -> Anchor<'a> {
        match self {
            Self::Direct(p) => Anchor::Direct(p.source().source()),
            Self::Erased(p) => Anchor::Erased(p.source().source()),
        }
    }
    fn floor(self) -> R<usize> {
        match self {
            Self::Direct(p) => p
                .source()
                .source()
                .pre_ranked_retained_analysis_storage_v1()
                .ok_or(E::Mismatch("connected original Direct source")),
            Self::Erased(p) => Ok(p.source().source().retained_storage_floor_v1()),
        }
    }
    fn inputs(self, budget: &mut Budget<'_>) -> R<Inputs<'a>> {
        match self {
            Self::Direct(p) => {
                let s = p.source().source();
                s.verify_equivalence_with_budget_v1(budget)
                    .map_err(E::Source)?;
                Ok(Inputs {
                    semantic: s.semantic().semantic(),
                    original: s.pre_ranked_executable().ok_or(E::Mismatch("original N"))?,
                    erased: None,
                    launch: s
                        .source_launch_roster()
                        .ok_or(E::Mismatch("source launch"))?,
                    catalog: p.source().catalog(),
                    middle: p.middle_end_roster(),
                    correspondence: p.correspondence_roster(),
                    verus: p.verus_roster(),
                })
            }
            Self::Erased(p) => {
                let s = p.source().source();
                s.verify_equivalence(budget).map_err(E::Source)?;
                Ok(Inputs {
                    semantic: s.original_source().semantic_ssa().source_semantic(),
                    original: s.original_source().executable(),
                    erased: Some(s.erased()),
                    launch: s.original_source().source_launch(),
                    catalog: p.source().catalog(),
                    middle: p.middle_end_roster(),
                    correspondence: p.correspondence_roster(),
                    verus: p.verus_roster(),
                })
            }
        }
    }
    fn original_formal(
        self,
        budget: &mut Budget<'_>,
    ) -> R<(
        OriginalNativeFormalMemoryV1<'a>,
        OriginalNativeFormalMemoryStorageV1,
    )> {
        match self {
            Self::Direct(p) => {
                analyze_original_native_formal_memory_v1(p.source().source(), budget)
            }
            Self::Erased(p) => {
                analyze_original_unit_local_formal_memory_v1(p.source().source(), budget)
            }
        }
        .map_err(E::OriginalFormal)
    }
    fn signed_count(self) -> usize {
        match self {
            Self::Direct(p) => p.root_count(),
            Self::Erased(p) => p.root_count(),
        }
    }
    fn has_signed_root(self, root: usize) -> bool {
        match self {
            Self::Direct(p) => p.signed_ranked_proof(root).is_some(),
            Self::Erased(p) => p.signed_ranked_proof(root).is_some(),
        }
    }
}

fn input_floor(frame: &Frame<'_>, history: &History<'_, '_>, source: Source<'_>) -> R<usize> {
    // The history frame borrows the History field inside the outer wire, so its
    // bytes are counted once. Independently allocated decoded graphs are separate.
    let history_bytes = history.frame().canonical_bytes();
    let separate = if std::ptr::eq(history_bytes, frame.field(Field::History)) {
        0
    } else {
        history_bytes.len()
    };
    source
        .floor()?
        .checked_add(frame.canonical_bytes().len())
        .and_then(|n| n.checked_add(separate))
        .and_then(|n| n.checked_add(size_of::<Frame<'_>>()))
        .and_then(|n| n.checked_add(history.frame().storage().retained_storage()))
        .and_then(|n| n.checked_add(history.storage().retained_storage()))
        .ok_or(Resource::Arithmetic.into())
}

fn scoped<'w, T>(budget: &mut Budget<'w>, run: impl FnOnce(&mut Budget<'w>) -> R<T>) -> R<T> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'w> as usize;
    let mut payloads = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            payloads[0] = Some(payload);
            Err(E::Panicked)
        }
    };
    let cleanup = if budget.work_ledger_identity_v1() != ledger
        || slot != budget as *const Budget<'w> as usize
        || budget.storage() < floor
    {
        Err(Resource::Accounting)
    } else {
        budget.release_storage(budget.storage() - floor)
    };
    if let Err(error) = cleanup {
        let rejected = std::mem::replace(&mut result, Err(error.into()));
        payloads[1] = catch_unwind(AssertUnwindSafe(|| drop(rejected))).err();
    }
    drop(payloads);
    result
}

fn bytes(a: &[u8], b: &[u8], field: &'static str, budget: &mut Budget<'_>) -> R<()> {
    budget.charge_work(
        a.len()
            .checked_add(b.len())
            .and_then(|n| n.checked_add(1))
            .ok_or(Resource::Arithmetic)?,
    )?;
    if a != b {
        return Err(E::Mismatch(field));
    }
    Ok(())
}

fn codec<T>(len: usize, budget: &mut Budget<'_>) -> R<()> {
    // Preserve the existing constituent codecs' bounded internal allocation
    // domains. Prepay newly visible headers and coexisting canonical copies.
    budget.reserve_storage(
        len.checked_mul(2)
            .and_then(|n| n.checked_add(size_of::<T>()))
            .ok_or(Resource::Arithmetic)?,
    )?;
    budget.charge_work(
        len.checked_mul(3)
            .and_then(|n| n.checked_add(1))
            .ok_or(Resource::Arithmetic)?,
    )?;
    Ok(())
}

fn envelope(wire: &[u8], graph: &Graph, catalog: &Catalog, budget: &mut Budget<'_>) -> R<Subject> {
    budget.charge_work(112)?;
    let value = NativeNeutralModuleRefV1::decode(wire).map_err(E::NativeEnvelope)?;
    let subject = joins::subject(graph, catalog)?;
    if value.subject() != &subject {
        return Err(E::Mismatch("exact native envelope subject"));
    }
    bytes(
        value.graph_bytes(),
        graph.canonical().canonical_bytes(),
        "native graph",
        budget,
    )?;
    bytes(
        value.catalog_bytes(),
        catalog.canonical_bytes(),
        "native catalog",
        budget,
    )?;
    Ok(subject)
}

/// Checks every content relation against independent signed source custody.
/// Source/history/wire backing and their retained receipts are caller-owned.
/// New scans/copies/headers are prepaid on this Budget; inherited formal/native
/// engines and codecs retain their existing domains, not a process-RSS promise.
pub fn check_compiler_refined_forwarding_output_v1<'a, 'h, 'w>(
    frame: &'a Frame<'w>,
    history: &'a History<'h, 'w>,
    source: Source<'a>,
    budget: &mut Budget<'_>,
) -> R<(Checked<'a, 'h, 'w>, Storage)> {
    scoped(budget, |budget| {
        budget.charge_work(4)?;
        if budget.storage_limit() > MAX_STORAGE {
            return Err(E::Mismatch("bounded storage cap"));
        }
        if frame.route() != source.route() {
            return Err(E::Mismatch("independent source route"));
        }
        if budget.storage() < input_floor(frame, history, source)? {
            return Err(Resource::Accounting.into());
        }
        let storage = CompilerRefinedForwardingOutputStorageV1(size_of::<Checked<'_, '_, '_>>());
        budget.reserve_storage(
            storage
                .0
                .checked_add(size_of::<Inputs<'_>>())
                .and_then(|n| n.checked_add(READ_STORAGE))
                .ok_or(Resource::Arithmetic)?,
        )?;
        bytes(
            frame.field(Field::History),
            history.frame().canonical_bytes(),
            "complete history field",
            budget,
        )?;
        let checked_history = history.check_semantics(budget).map_err(E::History)?;
        budget.reserve_storage(checked_history.storage().retained_storage())?;
        let inputs = source.inputs(budget)?;
        let count = inputs.semantic.roots().len();
        if count == 0 || count != source.signed_count() || count != frame.root_count() as usize {
            return Err(E::Mismatch("complete signed source roster"));
        }
        for ordinal in 0..count {
            budget.charge_work(1)?;
            if !source.has_signed_root(ordinal) {
                return Err(E::Mismatch("signed source root"));
            }
        }
        bytes(
            frame.field(Field::SemanticMir),
            inputs.semantic.canonical_encoding(),
            "semantic source bytes",
            budget,
        )?;
        let original_subject = envelope(
            frame.field(Field::OriginalNative),
            inputs.original,
            inputs.catalog,
            budget,
        )?;
        let neutral = match inputs.erased {
            Some(erased) => {
                bytes(
                    frame.field(Field::Erased),
                    erased.canonical().canonical_bytes(),
                    "actual E",
                    budget,
                )?;
                erased
            }
            None => {
                if !frame.field(Field::Erased).is_empty() {
                    return Err(E::Mismatch("Direct empty E"));
                }
                inputs.original
            }
        };
        bytes(
            frame.field(Field::OriginalMiddleEnd),
            inputs.middle.canonical_bytes(),
            "signed middle roster",
            budget,
        )?;
        bytes(
            frame.field(Field::OriginalCorrespondence),
            inputs.correspondence.canonical_bytes(),
            "signed correspondence roster",
            budget,
        )?;
        bytes(
            frame.field(Field::OriginalVerus),
            inputs.verus.canonical_bytes(),
            "signed Verus roster",
            budget,
        )?;
        joins::association(frame, budget)?;
        let (original_formal, original_storage) = source.original_formal(budget)?;
        budget.reserve_storage(original_storage.retained_storage())?;
        codec::<Roster>(frame.field(Field::OriginalFormalMemory).len(), budget)?;
        let original_roster =
            Roster::decode(frame.field(Field::OriginalFormalMemory)).map_err(E::Roster)?;
        joins::formal(
            &inputs,
            inputs.original,
            &original_subject,
            &original_roster,
            original_formal.kernels(),
            budget,
        )?;
        drop(original_roster);
        drop(original_formal);
        budget.release_storage(original_storage.retained_storage())?;

        codec::<Native>(frame.field(Field::NativeV2).len(), budget)?;
        let native = Native::decode(frame.field(Field::NativeV2)).map_err(E::Native)?;
        let profile = joins::profile(&native)?;
        let (coordinates, coordinate_storage) = check_production_target_coordinate_preservation_v1(
            neutral,
            history.graph(Role::B),
            profile,
            budget,
        )
        .map_err(E::Coordinates)?;
        budget.reserve_storage(coordinate_storage.retained_storage())?;
        drop(coordinates);
        budget.release_storage(coordinate_storage.retained_storage())?;
        let output = history.graph(Role::F);
        let final_subject = envelope(
            frame.field(Field::FinalNative),
            output,
            inputs.catalog,
            budget,
        )?;
        let (final_formal, final_storage) =
            analyze_canonical_output_guarded_formal_memory_v1(output, source.anchor(), budget)
                .map_err(E::FinalFormal)?;
        budget.reserve_storage(final_storage.retained_storage())?;
        codec::<Roster>(frame.field(Field::FinalFormalMemory).len(), budget)?;
        let final_roster =
            Roster::decode(frame.field(Field::FinalFormalMemory)).map_err(E::Roster)?;
        joins::formal(
            &inputs,
            output,
            &final_subject,
            &final_roster,
            final_formal.kernels(),
            budget,
        )?;
        codec::<Descriptor>(frame.field(Field::Descriptor).len(), budget)?;
        let descriptor =
            Descriptor::decode(frame.field(Field::Descriptor)).map_err(E::Descriptor)?;
        joins::roots(frame, &inputs, output, &native, &descriptor, budget)?;
        joins::capabilities(output, &descriptor, budget)?;
        budget.charge_work(native.module_bytes().len())?;
        let llvm =
            std::str::from_utf8(native.module_bytes()).map_err(|_| E::Mismatch("LLVM UTF-8"))?;
        let relation = check_native_v12_text_descriptor_relation_v1(
            output,
            inputs.catalog,
            output.canonical().canonical_bytes(),
            profile,
            descriptor.table(),
            llvm,
            budget,
        )
        .map_err(E::TextDescriptor)?;
        budget.reserve_storage(relation.storage().retained_storage())?;
        budget.reserve_storage(HASH_STORAGE)?;
        let limit = budget.storage_limit();
        let identity = inert_refined_forwarding_output_identity_v1(frame, limit, |work| {
            budget.charge_work(work)
        })
        .map_err(E::Framing)?;
        let history_identity = inert_refined_forwarding_history_identity_v1(
            frame.field(Field::History),
            limit,
            |work| budget.charge_work(work),
        )
        .map_err(E::Framing)?;
        budget.charge_work(1)?;
        drop(relation);
        drop(descriptor);
        drop(final_roster);
        drop(final_formal);
        drop(native);
        drop(checked_history);
        Ok((
            Checked {
                frame,
                history,
                source,
                identity,
                history_identity,
                limits: history.frame().limits(),
                storage,
            },
            storage,
        ))
    })
}

#[cfg(test)]
#[path = "compiler_refined_forwarding_output_v1_tests.rs"]
mod tests;
