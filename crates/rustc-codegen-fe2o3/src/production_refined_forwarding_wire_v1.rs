//! Inert encoding consumes and retains the actual live signed final-F owner.
#[path = "production_refined_forwarding_entry_v1.rs"]
mod entry;
use super::{PreparedRefinedForwardingWorkerHandoffV1 as Live, SourceLineage};
use crate::production_pipeline::ProductionPipelineError;
use fe2o3_compiler_ffi::{
    INERT_REFINED_FORWARDING_ENCODE_STORAGE_V1 as ENCODE_STORAGE,
    INERT_REFINED_FORWARDING_HASH_STORAGE_V1 as HASH_STORAGE,
    INERT_REFINED_FORWARDING_READ_STORAGE_V1 as READ_STORAGE,
    InertRefinedForwardingOutputErrorV1 as FrameError,
    InertRefinedForwardingOutputFieldV1 as Field, InertRefinedForwardingRouteV1 as Route,
    MAX_INERT_REFINED_FORWARDING_OUTPUT_BYTES_V1 as MAX_BYTES,
    MAX_INERT_REFINED_FORWARDING_STORAGE_V1 as MAX_STORAGE,
    encode_inert_refined_forwarding_output_into_v1, inert_refined_forwarding_output_len_v1,
    read_inert_refined_forwarding_output_v1,
};
use fe2o3_compiler_lineage::{
    InertCanonicalSemanticMirReceiptV3, InertFormalMemoryReceiptV3, InertKernelIrReceiptV3,
    InertLineageContentIdentityV3 as Content, InertMiddleEndReceiptV3,
    InertMirToKirCorrespondenceReceiptV3, InertNativeNeutralSubjectV1 as Subject,
    InertProofBindingAssociationErrorV3, InertProofBindingAssociationErrorV4,
    InertProofBindingAssociationInputsV4 as AssociationInputs,
    InertProofBindingAssociationV4 as Association, LineageErrorV3, MultiRootProofRosterErrorV3,
    MultiRootProofRosterInputsV3 as RosterInputs, MultiRootProofRosterKindV3 as Kind,
    MultiRootProofRosterRootInputV3 as Row, MultiRootProofRosterTranscriptV3 as Roster,
    NativeNeutralModuleErrorV1, NativeNeutralSubjectErrorV1, encode_native_neutral_module_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, FormalMemoryObligations,
    FormalMemoryReceiptErrorV1, InertCanonicalKernelIrContractCatalogV1 as Catalog,
    InertFormalMemoryReceiptFormatV4 as Formal, MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1,
    VerifiedCanonicalKernelIrModuleV12 as Graph,
};
use fe2o3_kernel_opt::{
    InertRefinedForwardingHistoryBytesV1 as HistoryBytes, RefinedForwardingHistoryWireErrorV1,
    encode_refined_forwarding_history_v1,
};
use fe2o3_lower_mir_kernel::{
    OriginalNativeFormalMemoryErrorV1, analyze_original_native_formal_memory_v1,
    analyze_original_unit_local_formal_memory_v1,
};
use fe2o3_verifier::{
    CompilerRefinedForwardingOutputErrorV1, RefinedForwardingOriginalSourceProofV1 as Source,
    recover_compiler_refined_forwarding_output_v1,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[derive(Debug)]
pub(crate) enum RefinedForwardingWireErrorV1 {
    Resource(Resource),
    Live(ProductionPipelineError),
    History(RefinedForwardingHistoryWireErrorV1),
    Framing(FrameError<Resource>),
    OriginalFormal(OriginalNativeFormalMemoryErrorV1),
    Formal(FormalMemoryReceiptErrorV1),
    Subject(NativeNeutralSubjectErrorV1),
    Native(NativeNeutralModuleErrorV1),
    Roster(MultiRootProofRosterErrorV3),
    Association(InertProofBindingAssociationErrorV4),
    Identity(InertProofBindingAssociationErrorV3),
    Lineage(LineageErrorV3),
    Verification(CompilerRefinedForwardingOutputErrorV1),
    Mismatch(&'static str),
    Panicked,
}
type E = RefinedForwardingWireErrorV1;
type R<T> = Result<T, E>;
impl From<Resource> for E {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "inert final-F wire: {self:?}")
    }
}
impl std::error::Error for E {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RefinedForwardingWireStorageV1(usize);
impl RefinedForwardingWireStorageV1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

/// No conversion into a worker/publication artifact. The live compiler owner
/// remains retained; independent byte replay does not authenticate its origin.
pub(crate) struct PreparedRefinedForwardingWireV1 {
    live: Live,
    wire: Vec<u8>,
    retained_floor: usize,
}
impl PreparedRefinedForwardingWireV1 {
    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        &self.wire
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub(crate) const fn authenticates_execution(&self) -> bool {
        false
    }
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_floor
    }
    pub(crate) fn verify_equivalence(&self, budget: &mut Budget<'_>) -> R<()> {
        if budget.storage() < self.retained_floor {
            return Err(Resource::Accounting.into());
        }
        scoped(budget, |budget| {
            self.live.verify_equivalence(budget).map_err(E::Live)?;
            scoped(budget, |budget| {
                let parts = prepare(&self.live, budget)?;
                let fields = fields(&self.live, &parts);
                budget.reserve_storage(READ_STORAGE)?;
                let limit = budget.storage_limit();
                let frame = read_inert_refined_forwarding_output_v1(&self.wire, limit, |w| {
                    budget.charge_work(w)
                })
                .map_err(E::Framing)?;
                for (field, expected) in FIELDS.into_iter().zip(fields) {
                    same(frame.field(field), expected, budget)?;
                }
                drop(frame);
                drop(parts);
                Ok(())
            })?;
            verify(&self.live, &self.wire, budget)?;
            budget.charge_work(1)?;
            Ok(())
        })
    }
}

impl Live {
    pub(crate) fn into_inert_output_wire_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> R<(
        PreparedRefinedForwardingWireV1,
        RefinedForwardingWireStorageV1,
    )> {
        if budget.storage() < self.retained_storage_floor_v1() {
            return Err(Resource::Accounting.into());
        }
        let entry_floor = budget.storage();
        scoped(budget, |budget| {
            budget.charge_work(4)?;
            if budget.storage_limit() > MAX_STORAGE {
                return Err(E::Mismatch("bounded storage cap"));
            }
            self.verify_equivalence(budget).map_err(E::Live)?;
            let header = size_of::<PreparedRefinedForwardingWireV1>()
                .checked_sub(size_of::<Live>())
                .ok_or(Resource::Arithmetic)?;
            budget.reserve_storage(header)?;
            let wire = scoped(budget, |budget| {
                let parts = prepare(&self, budget)?;
                let fields = fields(&self, &parts);
                let length = inert_refined_forwarding_output_len_v1::<Resource>(&fields)
                    .map_err(E::Framing)?;
                if length > MAX_BYTES {
                    return Err(E::Mismatch("bounded complete output"));
                }
                let mut wire = vector::<u8>(length, budget)?;
                budget.charge_work(length)?;
                wire.resize(length, 0);
                budget.reserve_storage(ENCODE_STORAGE)?;
                let limit = budget.storage_limit();
                encode_inert_refined_forwarding_output_into_v1(
                    fields,
                    route(&self),
                    &mut wire,
                    limit,
                    |w| budget.charge_work(w),
                )
                .map_err(E::Framing)?;
                drop(parts);
                Ok(wire)
            })?;
            // The new owner header already covers the Vec handle. Only its
            // actual transferred backing is added before independent decoding.
            budget.reserve_storage(wire.capacity())?;
            verify(&self, &wire, budget)?;
            let retained = header
                .checked_add(wire.capacity())
                .ok_or(Resource::Arithmetic)?;
            let retained_floor = entry_floor
                .checked_add(retained)
                .ok_or(Resource::Arithmetic)?;
            budget.charge_work(1)?;
            Ok((
                PreparedRefinedForwardingWireV1 {
                    live: self,
                    wire,
                    retained_floor,
                },
                RefinedForwardingWireStorageV1(retained),
            ))
        })
    }
}

const FIELDS: [Field; 14] = [
    Field::History,
    Field::NativeV2,
    Field::Descriptor,
    Field::SemanticMir,
    Field::OriginalNative,
    Field::Erased,
    Field::OriginalInputV4,
    Field::OriginalFormalMemory,
    Field::FinalNative,
    Field::FinalFormalMemory,
    Field::OriginalMiddleEnd,
    Field::OriginalCorrespondence,
    Field::OriginalVerus,
    Field::Roots,
];

struct Parts {
    history: HistoryBytes,
    original_formal: Roster,
    final_native: Vec<u8>,
    final_formal: Roster,
    association: Association,
    roots: Vec<u8>,
}

fn source(live: &Live) -> Source<'_> {
    match &live.source {
        SourceLineage::Direct(s) => Source::Direct(s.proof()),
        SourceLineage::Erased(s) => Source::Erased(s.proof()),
    }
}
fn route(live: &Live) -> Route {
    match &live.source {
        SourceLineage::Direct(_) => Route::Direct,
        SourceLineage::Erased(_) => Route::Erased,
    }
}
fn middle(live: &Live) -> &Roster {
    match &live.source {
        SourceLineage::Direct(s) => s.proof().middle_end_roster(),
        SourceLineage::Erased(s) => s.proof().middle_end_roster(),
    }
}
fn original_native(live: &Live) -> &[u8] {
    match &live.source {
        SourceLineage::Direct(s) => s.native_module(),
        SourceLineage::Erased(s) => s.original_native_module(),
    }
}
fn fields<'a>(live: &'a Live, parts: &'a Parts) -> [&'a [u8]; 14] {
    let (native, descriptor, _) = live.native_output_parts();
    let (semantic, erased, middle, correspondence, verus) = match &live.source {
        SourceLineage::Direct(s) => {
            let p = s.proof();
            (
                p.source()
                    .source()
                    .semantic()
                    .semantic()
                    .canonical_encoding(),
                &[][..],
                p.middle_end_roster(),
                p.correspondence_roster(),
                p.verus_roster(),
            )
        }
        SourceLineage::Erased(s) => {
            let p = s.proof();
            (
                p.source()
                    .source()
                    .original_source()
                    .semantic_ssa()
                    .source_semantic()
                    .canonical_encoding(),
                p.source().source().erased().canonical().canonical_bytes(),
                p.middle_end_roster(),
                p.correspondence_roster(),
                p.verus_roster(),
            )
        }
    };
    [
        parts.history.canonical_bytes(),
        native.canonical_bytes(),
        descriptor.canonical_bytes(),
        semantic,
        original_native(live),
        erased,
        parts.association.canonical_bytes(),
        parts.original_formal.canonical_bytes(),
        &parts.final_native,
        parts.final_formal.canonical_bytes(),
        middle.canonical_bytes(),
        correspondence.canonical_bytes(),
        verus.canonical_bytes(),
        &parts.roots,
    ]
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
    let restored = if ledger != budget.work_ledger_identity_v1()
        || slot != budget as *const Budget<'w> as usize
        || budget.storage() < floor
    {
        Err(Resource::Accounting)
    } else {
        budget.release_storage(budget.storage() - floor)
    };
    if let Err(error) = restored {
        let rejected = std::mem::replace(&mut result, Err(error.into()));
        payloads[1] = catch_unwind(AssertUnwindSafe(|| drop(rejected))).err();
    }
    drop(payloads);
    result
}
fn vector<T>(count: usize, budget: &mut Budget<'_>) -> R<Vec<T>> {
    let requested = count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(
        requested
            .checked_add(size_of::<Vec<T>>())
            .ok_or(Resource::Arithmetic)?,
    )?;
    budget.charge_work(1)?;
    let mut out = Vec::new();
    out.try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    budget.reserve_storage(
        out.capacity()
            .checked_mul(size_of::<T>())
            .ok_or(Resource::Arithmetic)?
            .checked_sub(requested)
            .ok_or(Resource::Accounting)?,
    )?;
    Ok(out)
}
fn copied(bytes: &[u8], budget: &mut Budget<'_>) -> R<Vec<u8>> {
    let mut out = vector(bytes.len(), budget)?;
    budget.charge_work(bytes.len())?;
    out.extend_from_slice(bytes);
    Ok(out)
}
fn same(a: &[u8], b: &[u8], budget: &mut Budget<'_>) -> R<()> {
    budget.charge_work(
        a.len()
            .checked_add(b.len())
            .and_then(|n| n.checked_add(1))
            .ok_or(Resource::Arithmetic)?,
    )?;
    if a != b {
        return Err(E::Mismatch("exact live-derived output field"));
    }
    Ok(())
}
fn codec<T>(bytes: usize, budget: &mut Budget<'_>) -> R<()> {
    budget.reserve_storage(
        bytes
            .checked_mul(2)
            .and_then(|n| n.checked_add(size_of::<T>()))
            .ok_or(Resource::Arithmetic)?,
    )?;
    budget.charge_work(
        bytes
            .checked_mul(3)
            .and_then(|n| n.checked_add(1))
            .ok_or(Resource::Arithmetic)?,
    )?;
    Ok(())
}
fn subject(graph: &Graph, catalog: &Catalog) -> R<Subject> {
    Subject::new(
        *graph.canonical().identity().digest(),
        graph.canonical().identity().canonical_length(),
        *catalog.digest(),
        u64::try_from(catalog.canonical_bytes().len()).map_err(|_| Resource::Arithmetic)?,
    )
    .map_err(E::Subject)
}

fn payloads(reports: &[FormalMemoryObligations], budget: &mut Budget<'_>) -> R<Vec<Vec<u8>>> {
    let mut rows = vector(reports.len(), budget)?;
    for report in reports {
        let (row, retained) = scoped(budget, |budget| {
            budget.reserve_storage(
                size_of::<Formal>()
                    .checked_add(MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            budget.charge_work(MAX_FORMAL_MEMORY_RECEIPT_BYTES_V1)?;
            let formal = Formal::from_current_obligations(report).map_err(E::Formal)?;
            let row = copied(formal.canonical_bytes(), budget)?;
            let retained = row.capacity();
            drop(formal);
            Ok((row, retained))
        })?;
        budget.reserve_storage(retained)?;
        rows.push(row);
    }
    Ok(rows)
}

fn roster(live: &Live, graph: &Graph, payloads: &[Vec<u8>], budget: &mut Budget<'_>) -> R<Roster> {
    let signed = middle(live);
    let count = signed.root_count();
    if count != graph.module().kernels.len() || payloads.len() != count {
        return Err(E::Mismatch("complete formal payload roster"));
    }
    let mut rows = vector::<Row<'_>>(count, budget)?;
    let mut length = 184usize
        .checked_add(count.checked_mul(4).ok_or(Resource::Arithmetic)?)
        .ok_or(Resource::Arithmetic)?;
    for i in 0..count {
        budget.charge_work(
            signed
                .canonical_bytes()
                .len()
                .checked_add(graph.canonical().canonical_bytes().len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        let root = signed.root(i).ok_or(E::Mismatch("actual signed row"))?;
        let mut matches = graph
            .module()
            .kernels
            .iter()
            .enumerate()
            .filter(|(_, k)| k.id.as_str() == root.export_symbol());
        let (index, kernel) = matches.next().ok_or(E::Mismatch("formal output root"))?;
        if matches.next().is_some() {
            return Err(E::Mismatch("unique formal output root"));
        }
        let payload = &payloads[index];
        for size in [
            100,
            root.logical_name().len(),
            root.export_symbol().len(),
            kernel.id.as_str().len(),
            payload.len(),
        ] {
            length = length.checked_add(size).ok_or(Resource::Arithmetic)?;
        }
        rows.push(Row {
            semantic_root: root.semantic_root(),
            semantic_root_identity: root.semantic_root_identity(),
            kernel_binding: root.kernel_binding(),
            source_rank: root.source_rank(),
            workgroup: root.workgroup(),
            logical_name: root.logical_name(),
            export_symbol: root.export_symbol(),
            kernel_id: kernel.id.as_str(),
            payload,
        });
    }
    codec::<Roster>(length, budget)?;
    let result = Roster::new(RosterInputs {
        kind: Kind::FormalMemory,
        semantic_mir_sha256: signed.semantic_mir_sha256(),
        native_neutral_subject: subject(graph, live.source.catalog())?,
        roster_identity: signed.roster_identity(),
        canonical_kernel_order: signed.canonical_kernel_order(),
        roots: &rows,
    })
    .map_err(E::Roster)?;
    if result.canonical_bytes().len() != length {
        return Err(E::Mismatch("exact formal roster extent"));
    }
    drop(rows);
    Ok(result)
}

fn original_formal(live: &Live, budget: &mut Budget<'_>) -> R<Roster> {
    let (report, receipt) = match source(live) {
        Source::Direct(p) => analyze_original_native_formal_memory_v1(p.source().source(), budget),
        Source::Erased(p) => {
            analyze_original_unit_local_formal_memory_v1(p.source().source(), budget)
        }
    }
    .map_err(E::OriginalFormal)?;
    budget.reserve_storage(receipt.retained_storage())?;
    let payloads = payloads(report.kernels(), budget)?;
    let result = roster(live, report.original(), &payloads, budget)?;
    drop(payloads);
    drop(report);
    Ok(result)
}

fn association(live: &Live, formal: &Roster, budget: &mut Budget<'_>) -> R<Association> {
    budget.reserve_storage(
        size_of::<AssociationInputs>()
            .checked_add(5 * size_of::<Content>())
            .ok_or(Resource::Arithmetic)?,
    )?;
    let (semantic, middle, correspondence, verus) = match &live.source {
        SourceLineage::Direct(s) => {
            let p = s.proof();
            (
                p.source()
                    .source()
                    .semantic()
                    .semantic()
                    .canonical_encoding(),
                p.middle_end_roster(),
                p.correspondence_roster(),
                p.verus_roster(),
            )
        }
        SourceLineage::Erased(s) => {
            let p = s.proof();
            (
                p.source()
                    .source()
                    .original_source()
                    .semantic_ssa()
                    .source_semantic()
                    .canonical_encoding(),
                p.middle_end_roster(),
                p.correspondence_roster(),
                p.verus_roster(),
            )
        }
    };
    macro_rules! identity {
        ($ty:ty, $bytes:expr) => {{
            scoped(budget, |budget| {
                let bytes = $bytes;
                codec::<$ty>(bytes.len(), budget)?;
                budget.reserve_storage(HASH_STORAGE)?;
                // Pinned V3 domains are under 64 bytes; include fixed hash work.
                budget.charge_work(256)?;
                let value =
                    <$ty>::from_canonical_preimage(copied(bytes, budget)?).map_err(E::Lineage)?;
                let identity =
                    Content::new(*value.identity().sha256(), value.identity().byte_len())
                        .map_err(E::Identity)?;
                drop(value);
                Ok(identity)
            })?
        }};
    }
    let inputs = AssociationInputs::new(
        identity!(InertCanonicalSemanticMirReceiptV3, semantic),
        identity!(InertMiddleEndReceiptV3, middle.canonical_bytes()),
        identity!(InertKernelIrReceiptV3, original_native(live)),
        identity!(
            InertMirToKirCorrespondenceReceiptV3,
            correspondence.canonical_bytes()
        ),
        identity!(InertFormalMemoryReceiptV3, formal.canonical_bytes()),
    );
    codec::<Association>(
        fe2o3_compiler_lineage::MAX_INERT_PROOF_BINDING_ASSOCIATION_BYTES_V4,
        budget,
    )?;
    Association::new(inputs, verus.canonical_bytes()).map_err(E::Association)
}

fn prepare(live: &Live, budget: &mut Budget<'_>) -> R<Parts> {
    budget.reserve_storage(
        size_of::<Parts>()
            .checked_add(size_of::<[&[u8]; 14]>())
            .ok_or(Resource::Arithmetic)?,
    )?;
    let history =
        encode_refined_forwarding_history_v1(live.history_inputs(), budget).map_err(E::History)?;
    budget.reserve_storage(history.storage().retained_storage())?;
    let original_formal = original_formal(live, budget)?;
    let final_formal = roster(live, live.output(), live.final_formal_payloads(), budget)?;
    let association = association(live, &original_formal, budget)?;
    let output = live.output().canonical().canonical_bytes();
    let catalog = live.source.catalog().canonical_bytes();
    let requested = 112usize
        .checked_add(output.len())
        .and_then(|n| n.checked_add(catalog.len()))
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(
        requested
            .checked_add(size_of::<Vec<u8>>())
            .ok_or(Resource::Arithmetic)?,
    )?;
    budget.charge_work(requested)?;
    let final_native = encode_native_neutral_module_v1(
        &subject(live.output(), live.source.catalog())?,
        output,
        catalog,
    )
    .map_err(E::Native)?;
    budget.reserve_storage(
        final_native
            .capacity()
            .checked_sub(requested)
            .ok_or(Resource::Accounting)?,
    )?;
    let roots = roots(live, budget)?;
    Ok(Parts {
        history,
        original_formal,
        final_native,
        final_formal,
        association,
        roots,
    })
}

fn roots(live: &Live, budget: &mut Budget<'_>) -> R<Vec<u8>> {
    budget.reserve_storage(size_of::<
        crate::production_pipeline::native_checked_output_handoff_v1::SourceInputsV1<'_>,
    >())?;
    let inputs = live.source.replay_inputs(budget).map_err(E::Live)?;
    let signed = middle(live);
    let descriptors = live.native_output_parts().1.table().kernels();
    let count = signed.root_count();
    if !(1..=fe2o3_compiler_ffi::MAX_INERT_REFINED_FORWARDING_ROOTS_V1).contains(&count) {
        return Err(E::Mismatch("bounded signed root count"));
    }
    let capacity = 4usize
        .checked_add(count.checked_mul(219 + 512).ok_or(Resource::Arithmetic)?)
        .ok_or(Resource::Arithmetic)?;
    let mut out = vector::<u8>(capacity, budget)?;
    budget.charge_work(capacity)?;
    out.extend_from_slice(
        &u32::try_from(count)
            .map_err(|_| Resource::Arithmetic)?
            .to_le_bytes(),
    );
    for i in 0..count {
        let root = signed.root(i).ok_or(E::Mismatch("signed root"))?;
        let launch = *inputs
            .launch
            .roots()
            .get(i)
            .ok_or(E::Mismatch("source launch"))?;
        let scan = inputs
            .original
            .canonical()
            .canonical_bytes()
            .len()
            .checked_add(live.output().canonical().canonical_bytes().len())
            .and_then(|n| n.checked_add(live.native_output_parts().1.canonical_bytes().len()))
            .and_then(|n| n.checked_add(signed.canonical_bytes().len()))
            .ok_or(Resource::Arithmetic)?;
        budget.charge_work(scan)?;
        let n = inputs
            .original
            .module()
            .kernels
            .iter()
            .position(|k| k.id.as_str() == root.export_symbol())
            .ok_or(E::Mismatch("original root"))?;
        let f = live
            .output()
            .module()
            .kernels
            .iter()
            .position(|k| k.id.as_str() == root.export_symbol())
            .ok_or(E::Mismatch("final root"))?;
        let d = descriptors
            .iter()
            .position(|k| k.entry_name().as_str() == root.export_symbol())
            .ok_or(E::Mismatch("descriptor root"))?;
        let nf = inputs
            .original
            .module()
            .functions
            .iter()
            .position(|v| v.id == inputs.original.module().kernels[n].entry)
            .ok_or(E::Mismatch("original function"))?;
        let ff = live
            .output()
            .module()
            .functions
            .iter()
            .position(|v| v.id == live.output().module().kernels[f].entry)
            .ok_or(E::Mismatch("final function"))?;
        out.extend_from_slice(&root.semantic_root().to_le_bytes());
        out.extend_from_slice(&root.semantic_root_identity());
        word(&mut out, d)?;
        out.extend_from_slice(descriptors[d].kernel_id().as_bytes());
        for v in [n, nf, f, ff] {
            word(&mut out, v)?;
        }
        out.extend_from_slice(&root.kernel_binding());
        out.push(launch.source_rank());
        let wg = launch
            .source_launch()
            .exact_workgroup()
            .ok_or(E::Mismatch("exact source workgroup"))?;
        out.push(1);
        for v in wg.into_iter().chain(launch.source_launch().max_grid()) {
            out.extend_from_slice(&v.to_le_bytes());
        }
        let layout = launch.layout();
        out.extend_from_slice(&layout.grid_identity().to_le_bytes());
        for v in layout
            .global_extents()
            .into_iter()
            .chain(layout.workgroup_extents())
            .chain([layout.subgroup_size()])
        {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.push(u8::from(layout.full_physical_workgroups()));
        for name in [root.logical_name(), root.export_symbol()] {
            if name.len() > 256 {
                return Err(E::Mismatch("bounded root name"));
            }
            word(&mut out, name.len())?;
            out.extend_from_slice(name.as_bytes());
        }
    }
    Ok(out)
}
fn word(out: &mut Vec<u8>, value: usize) -> R<()> {
    out.extend_from_slice(
        &u32::try_from(value)
            .map_err(|_| Resource::Arithmetic)?
            .to_le_bytes(),
    );
    Ok(())
}

fn verify(live: &Live, wire: &[u8], budget: &mut Budget<'_>) -> R<()> {
    scoped(budget, |budget| {
        let source_packet = match &live.source {
            SourceLineage::Direct(source) => source.source_packet(),
            SourceLineage::Erased(source) => source.source_packet(),
        };
        let (checked, receipt) =
            recover_compiler_refined_forwarding_output_v1(wire, source_packet, budget)
                .map_err(E::Verification)?;
        budget.reserve_storage(receipt.retained_storage())?;
        budget.charge_work(1)?;
        drop(checked);
        Ok(())
    })
}

#[cfg(test)]
#[path = "production_refined_forwarding_wire_v1_tests.rs"]
mod tests;
