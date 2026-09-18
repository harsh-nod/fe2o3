//! Native source/N reconstruction and signature-to-Middle commitment binding.
//! Embedded receipt keys establish internal signature consistency only. Protected
//! compiler and launch origin remain a separate consuming join. This does not
//! reconstruct ranked/source equivalence from the serialized Middle text.

use crate::CanonicalProductionMirPlironVerusExecutionEvidenceV1 as Signed;
use fe2o3_compiler_lineage::{
    MultiRootProofRosterKindV3 as Kind, MultiRootProofRosterRootV3 as Root,
    MultiRootProofRosterTranscriptV3 as Roster, NativeNeutralModuleRefV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::{
    ProductionSourceLaunchRootInputV1, ReplayedNativeSourceV1,
    replay_native_source_correspondence_v1,
};
use fe2o3_mir_model::{
    InertCanonicalSemanticU32InductionEvidenceV1 as Induction,
    analyze_semantic_u32_induction_no_overflow_v1,
};
use fe2o3_pliron::InertProductionMiddleEndEvidenceV5 as Middle;
use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest as _, Sha256};

/// Untrusted ordered aggregate commitment input, not an imported receipt.
/// The actual aggregate signature is checked before these rows can bind Middle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeCompilerStagingCommitmentV1 {
    /// Individual staged receipt identity.
    pub receipt: [u8; 32],
    /// Normalized individual effect obligation.
    pub effect: [u8; 32],
    /// Individual receipt signer identity.
    pub signer: [u8; 32],
    /// Individual proof execution identity.
    pub execution: [u8; 32],
    /// Verus executable/configuration, solver executable/configuration and runtime.
    pub toolchain: [[u8; 32]; 5],
}
impl NativeCompilerStagingCommitmentV1 {
    fn digests(self) -> [DigestV1; 9] {
        [
            self.receipt,
            self.effect,
            self.signer,
            self.execution,
            self.toolchain[0],
            self.toolchain[1],
            self.toolchain[2],
            self.toolchain[3],
            self.toolchain[4],
        ]
        .map(DigestV1::from_untrusted_bytes)
    }
}

/// Complete ordered staging slice for one semantic root. This is a typed
/// borrowed packet input, not a final native artifact wire-format extension.
#[derive(Clone, Copy)]
pub struct NativeCompilerRootStagingV1<'a> {
    /// Actual semantic root ID, in the complete roster's semantic-root order.
    pub semantic_root: u32,
    /// Every staged commitment in its original aggregate-hashing order.
    pub commitments: &'a [NativeCompilerStagingCommitmentV1],
}

/// Borrowed native source packet. Correspondence-root payloads are the exact
/// existing semantic-induction encoding; full correspondence is freshly rebuilt,
/// not approved from opaque payloads. Launch origin is not inferred from rows.
#[derive(Clone, Copy)]
pub struct NativeCompilerSourceProofInputsV1<'a> {
    pub semantic_mir: &'a [u8],
    pub native_module: &'a [u8],
    pub middle_end_roster: &'a [u8],
    pub correspondence_roster: &'a [u8],
    pub verus_roster: &'a [u8],
    pub launch_inputs: &'a [ProductionSourceLaunchRootInputV1<'a>],
    pub staging_roots: &'a [NativeCompilerRootStagingV1<'a>],
}

#[derive(Debug)]
pub enum NativeCompilerSourceProofErrorV1 {
    Resource(Resource),
    Native(fe2o3_compiler_lineage::NativeNeutralModuleErrorV1),
    Roster(fe2o3_compiler_lineage::MultiRootProofRosterErrorV3),
    Source(fe2o3_lower_mir_kernel::NativeSourceReplayErrorV1),
    Middle(fe2o3_pliron::ProductionMiddleEndEvidenceCodecErrorV5),
    Signed(crate::ProductionMirPlironVerusExecutionEvidenceErrorV1),
    Induction(fe2o3_mir_model::SemanticU32InductionAnalysisErrorV1),
    InductionWire(fe2o3_mir_model::SemanticU32InductionEvidenceErrorV1),
    Mismatch(&'static str),
}
impl std::fmt::Display for NativeCompilerSourceProofErrorV1 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "native source proof: {self:?}")
    }
}
impl std::error::Error for NativeCompilerSourceProofErrorV1 {}
impl From<Resource> for NativeCompilerSourceProofErrorV1 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
type E = NativeCompilerSourceProofErrorV1;

struct CheckedRoot {
    middle: Middle,
    induction: Induction,
    signed: Signed,
    staging: Vec<NativeCompilerStagingCommitmentV1>,
}

/// Move-only independently reconstructed source/N and imported aggregate receipt
/// cryptographically bound to the exact Middle identity and ordered commitments.
/// The ranked graph/source relation is NOT reconstructed from Middle's text.
/// Not a proof of B/O, LLVM refinement, trusted signer origin, or runtime safety.
///
/// ```compile_fail
/// use fe2o3_verifier::ValidatedNativeCompilerSourceProofV1;
/// fn duplicate(value: ValidatedNativeCompilerSourceProofV1) { let _ = value.clone(); }
/// ```
pub struct ValidatedNativeCompilerSourceProofV1 {
    source: ReplayedNativeSourceV1,
    middle: Roster,
    correspondence: Roster,
    verus: Roster,
    roots: Vec<CheckedRoot>,
}
impl ValidatedNativeCompilerSourceProofV1 {
    pub fn source(&self) -> &ReplayedNativeSourceV1 {
        &self.source
    }
    pub fn root_count(&self) -> usize {
        self.roots.len()
    }
    pub fn middle_end_roster(&self) -> &Roster {
        &self.middle
    }
    pub fn correspondence_roster(&self) -> &Roster {
        &self.correspondence
    }
    pub fn verus_roster(&self) -> &Roster {
        &self.verus
    }
    pub fn signed_ranked_proof(&self, root: usize) -> Option<&Signed> {
        self.roots.get(root).map(|r| &r.signed)
    }
    /// Exact ordered commitments that were bound by this root's aggregate signature.
    pub fn staging_commitments(&self, root: usize) -> Option<&[NativeCompilerStagingCommitmentV1]> {
        self.roots.get(root).map(|root| root.staging.as_slice())
    }
    /// The imported aggregate binding commits to the exact decoded Middle identity.
    pub const fn binds_signed_receipt_to_middle_identity(&self) -> bool {
        true
    }
    /// Serialized Middle text is not a reconstructible typed ranked/source proof.
    pub const fn replays_ranked_source_relation(&self) -> bool {
        false
    }
    pub const fn authenticates_compiler_or_launch_origin(&self) -> bool {
        false
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Source receipt, actual root-vector capacity and declared codec payload/header
/// reservations. Inherited semantic/roster/signature engines remain independently
/// bounded domains, not allocations measured by this logical canonical ledger.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeCompilerSourceProofStorageV1(usize);
impl NativeCompilerSourceProofStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

fn header_equal(a: Root<'_>, b: Root<'_>) -> bool {
    a.semantic_root() == b.semantic_root()
        && a.semantic_root_identity() == b.semantic_root_identity()
        && a.kernel_binding() == b.kernel_binding()
        && a.source_rank() == b.source_rank()
        && a.workgroup() == b.workgroup()
        && a.logical_name() == b.logical_name()
        && a.export_symbol() == b.export_symbol()
        && a.kernel_id() == b.kernel_id()
}

fn hash_frame(hash: &mut Sha256, frame: &[u8], budget: &mut Budget<'_>) -> Result<(), E> {
    budget.charge_work(
        8usize
            .checked_add(frame.len())
            .ok_or(Resource::Arithmetic)?,
    )?;
    hash.update(
        u64::try_from(frame.len())
            .map_err(|_| Resource::Arithmetic)?
            .to_le_bytes(),
    );
    hash.update(frame);
    Ok(())
}

fn check_aggregate_binding(
    signed: &Signed,
    rows: &[NativeCompilerStagingCommitmentV1],
    budget: &mut Budget<'_>,
) -> Result<(), E> {
    use crate::mir_pliron_per_compilation_verus_v1::{
        aggregate_obligation_commitment_work_v1, aggregate_obligation_from_commitments_v1,
    };
    budget.charge_work(2)?;
    let claims = signed.claims();
    if u64::try_from(rows.len()).map_err(|_| Resource::Arithmetic)?
        != claims.retained_policy_checked_staging()
    {
        return Err(E::Mismatch("complete signed staging roster"));
    }
    let binding = signed.imported_proof().binding();
    let subjects = binding.subjects();
    budget.charge_work(
        aggregate_obligation_commitment_work_v1(rows.len()).ok_or(Resource::Arithmetic)?,
    )?;
    let actual = aggregate_obligation_from_commitments_v1(
        [
            claims.contract_identity(),
            claims.parallel_contract_identity(),
            claims.pliron_evidence_identity(),
            claims.composition_template_identity(),
            claims.generated_source_identity(),
            subjects.safe_reference_identity(),
            subjects.safe_reference_source_hash(),
            subjects.safe_reference_mir_hash(),
            subjects.kernel_subject_identity(),
            subjects.kernel_mir_hash(),
        ],
        rows.iter()
            .copied()
            .map(NativeCompilerStagingCommitmentV1::digests),
    );
    budget.charge_work(32)?;
    if actual != binding.normalized_obligation_effect_ir_hash() {
        return Err(E::Mismatch("signed aggregate obligation binding"));
    }
    Ok(())
}

/// Replays source/N through the normal constructor, imports every actual signed
/// ranked receipt, compares exact source/root subjects, replays induction, and
/// independently rehashes the canonical ranked roster and exact aggregate
/// obligation. The latter binds Middle and every ordered staging commitment to
/// the imported signature; it does not reconstruct ranked/source equivalence.
/// No unsigned fallback. Staging slices are explicit typed inputs, not a native
/// artifact wire format or authority to authenticate their compiler origin.
/// The full launch inputs remain explicit caller-retained facts; a later protected
/// join must bind their source origin. No final O/formal or publication claim.
///
/// Entry8; complete borrowed codec lengths are charged before decoding. Root
/// headers/signature subjects and identity frames are paid before comparison.
/// Returned payload transfers; reserve its receipt before another allocation.
pub fn validate_native_compiler_source_proof_v1(
    inputs: NativeCompilerSourceProofInputsV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<
    (
        ValidatedNativeCompilerSourceProofV1,
        NativeCompilerSourceProofStorageV1,
    ),
    E,
> {
    budget.charge_work(8)?;
    let floor = budget.storage();
    let token = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'_> as usize;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let wire = inputs
            .middle_end_roster
            .len()
            .checked_add(inputs.correspondence_roster.len())
            .and_then(|n| n.checked_add(inputs.verus_roster.len()))
            .ok_or(Resource::Arithmetic)?;
        let header = std::mem::size_of::<ValidatedNativeCompilerSourceProofV1>()
            .checked_sub(std::mem::size_of::<ReplayedNativeSourceV1>())
            .ok_or(Resource::Arithmetic)?;
        let codec_storage = wire
            .checked_mul(2)
            .and_then(|n| n.checked_add(header))
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(codec_storage)?;
        budget.charge_work(
            wire.checked_add(inputs.native_module.len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        let native = NativeNeutralModuleRefV1::decode(inputs.native_module).map_err(E::Native)?;
        let middle = Roster::decode(inputs.middle_end_roster).map_err(E::Roster)?;
        let correspondence = Roster::decode(inputs.correspondence_roster).map_err(E::Roster)?;
        let verus = Roster::decode(inputs.verus_roster).map_err(E::Roster)?;
        for (roster, kind) in [
            (&middle, Kind::MiddleEnd),
            (&correspondence, Kind::Correspondence),
            (&verus, Kind::VerusExecution),
        ] {
            budget.charge_work(
                164usize
                    .checked_add(
                        roster
                            .canonical_kernel_order()
                            .len()
                            .checked_mul(4)
                            .ok_or(Resource::Arithmetic)?,
                    )
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if roster.kind() != kind
                || roster.native_neutral_subject() != native.subject()
                || roster.root_count() != middle.root_count()
                || roster.semantic_mir_sha256() != middle.semantic_mir_sha256()
                || roster.roster_identity() != middle.roster_identity()
                || roster.canonical_kernel_order() != middle.canonical_kernel_order()
            {
                return Err(E::Mismatch("complete native roster subjects/order"));
            }
        }
        let (source, source_storage) = replay_native_source_correspondence_v1(
            inputs.semantic_mir,
            native.graph_bytes(),
            native.catalog_bytes(),
            inputs.launch_inputs,
            budget,
        )
        .map_err(E::Source)?;
        budget.reserve_storage(source_storage.retained_storage())?;
        let semantic = source.source().semantic_ssa().source_semantic();
        let graph = source.source().executable();
        budget.charge_work(132)?;
        if native.subject().graph_digest() != graph.canonical().identity().digest()
            || native.subject().graph_length() != native.graph_bytes().len() as u64
            || native.subject().catalog_digest() != source.catalog().digest()
            || native.subject().catalog_length() != source.catalog().canonical_bytes().len() as u64
            || middle.semantic_mir_sha256() != *semantic.semantic_sha256().as_bytes()
            || middle.root_count() != semantic.roots().len()
            || middle.root_count() != source.source().source_launch().roots().len()
        {
            return Err(E::Mismatch("reconstructed source/native subject"));
        }
        budget.charge_work(1)?;
        if inputs.staging_roots.len() != middle.root_count() {
            return Err(E::Mismatch("complete staging root roster"));
        }
        let mut staging_storage = 0usize;
        let mut roots = Vec::new();
        let requested = middle
            .root_count()
            .checked_mul(std::mem::size_of::<CheckedRoot>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(requested)?;
        roots
            .try_reserve_exact(middle.root_count())
            .map_err(|_| Resource::Allocation)?;
        let root_storage = roots
            .capacity()
            .checked_mul(std::mem::size_of::<CheckedRoot>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(
            root_storage
                .checked_sub(requested)
                .ok_or(Resource::Accounting)?,
        )?;
        for (ordinal, semantic_root) in semantic.roots().iter().enumerate() {
            let row = middle
                .root(ordinal)
                .ok_or(E::Mismatch("missing middle root"))?;
            let induction_row = correspondence
                .root(ordinal)
                .ok_or(E::Mismatch("missing correspondence root"))?;
            let signed_row = verus
                .root(ordinal)
                .ok_or(E::Mismatch("missing signed root"))?;
            budget.charge_work(2)?;
            let staging_input = &inputs.staging_roots[ordinal];
            if staging_input.semantic_root != semantic_root.index() {
                return Err(E::Mismatch("ordered staging semantic root"));
            }
            let names = row
                .logical_name()
                .len()
                .checked_add(row.export_symbol().len())
                .and_then(|n| n.checked_add(row.kernel_id().len()))
                .ok_or(Resource::Arithmetic)?;
            budget.charge_work(
                320usize
                    .checked_add(names.checked_mul(3).ok_or(Resource::Arithmetic)?)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            let function = &semantic.functions()[semantic_root.index() as usize];
            let entry = function
                .kernel_entry()
                .ok_or(E::Mismatch("semantic root entry"))?;
            let launch = source.source().source_launch().roots()[ordinal];
            if !header_equal(row, induction_row)
                || !header_equal(row, signed_row)
                || row.semantic_root() != semantic_root.index()
                || row.semantic_root_identity() != *function.identity().as_bytes()
                || row.kernel_binding() != *entry.kernel_binding_identity().as_bytes()
                || row.export_symbol().as_bytes() != entry.export_symbol().as_bytes()
                || row.kernel_id() != row.export_symbol()
                || row.source_rank() != launch.source_rank()
                || row.workgroup().map(u64::from) != launch.layout().workgroup_extents()
            {
                return Err(E::Mismatch("semantic/native/ranked root or launch"));
            }
            let mut matched = false;
            for kernel in &graph.module().kernels {
                budget.charge_work(4 + kernel.id.as_str().len() + row.kernel_id().len())?;
                if kernel.id.as_str() == row.kernel_id() {
                    if matched
                        || kernel.entry.as_str() != row.kernel_id()
                        || kernel.domain.rank() != row.source_rank()
                    {
                        return Err(E::Mismatch("native kernel root"));
                    }
                    matched = true;
                }
            }
            if !matched {
                return Err(E::Mismatch("absent native kernel root"));
            }
            budget.charge_work(
                row.payload()
                    .len()
                    .checked_add(induction_row.payload().len())
                    .and_then(|n| n.checked_add(signed_row.payload().len()))
                    .ok_or(Resource::Arithmetic)?,
            )?;
            let decoded_middle = Middle::decode(row.payload()).map_err(E::Middle)?;
            if decoded_middle.source_semantic_identity() != semantic.semantic_sha256().as_bytes() {
                return Err(E::Mismatch("ranked middle-end semantic subject"));
            }
            let selection = semantic
                .select_kernel_body_for_root_v1(*semantic_root)
                .ok_or(E::Mismatch("selected source body"))?;
            let report = analyze_semantic_u32_induction_no_overflow_v1(semantic, selection.body())
                .map_err(E::Induction)?;
            let induction = Induction::from_report(&report).map_err(E::InductionWire)?;
            budget
                .charge_work(induction.canonical_bytes().len() + induction_row.payload().len())?;
            if induction.canonical_bytes() != induction_row.payload() {
                return Err(E::Mismatch("exact source induction replay"));
            }
            let signed = Signed::decode(signed_row.payload()).map_err(E::Signed)?;
            budget.charge_work(32)?;
            if signed.claims().pliron_evidence_identity().as_bytes()
                != decoded_middle.identity().sha256()
            {
                return Err(E::Mismatch("signed ranked subject"));
            }
            check_aggregate_binding(&signed, staging_input.commitments, budget)?;
            budget.charge_work(3)?;
            let requested = staging_input
                .commitments
                .len()
                .checked_mul(std::mem::size_of::<NativeCompilerStagingCommitmentV1>())
                .ok_or(Resource::Arithmetic)?;
            budget.reserve_storage(requested)?;
            let mut staging = Vec::new();
            staging
                .try_reserve_exact(staging_input.commitments.len())
                .map_err(|_| Resource::Allocation)?;
            let capacity = staging
                .capacity()
                .checked_mul(std::mem::size_of::<NativeCompilerStagingCommitmentV1>())
                .ok_or(Resource::Arithmetic)?;
            budget.reserve_storage(
                capacity
                    .checked_sub(requested)
                    .ok_or(Resource::Accounting)?,
            )?;
            for &row in staging_input.commitments {
                budget.charge_work(289)?;
                staging.push(row);
            }
            staging_storage = staging_storage
                .checked_add(capacity)
                .ok_or(Resource::Arithmetic)?;
            roots.push(CheckedRoot {
                middle: decoded_middle,
                induction,
                signed,
                staging,
            });
        }
        let mut hash = Sha256::new();
        let domain = b"FE2O3/PRODUCTION-RANKED-KERNEL-ROSTER-IDENTITY/V1\0";
        budget.charge_work(domain.len() + 8)?;
        hash.update(domain);
        hash.update((roots.len() as u64).to_le_bytes());
        for &ordinal in middle.canonical_kernel_order() {
            let index = usize::try_from(ordinal).map_err(|_| Resource::Arithmetic)?;
            let checked = roots
                .get(index)
                .ok_or(E::Mismatch("canonical root order"))?;
            let row = middle
                .root(index)
                .ok_or(E::Mismatch("canonical root row"))?;
            for field in [
                &row.kernel_binding()[..],
                row.logical_name().as_bytes(),
                row.export_symbol().as_bytes(),
                &row.semantic_root().to_le_bytes(),
                &row.semantic_root_identity(),
                &[row.source_rank()],
                checked.middle.identity().sha256(),
                &checked.middle.identity().byte_len().to_le_bytes(),
                checked.induction.semantic_mir_sha256(),
                &checked.induction.function().to_le_bytes(),
                checked.induction.function_identity(),
                &u64::from(checked.induction.checked_additions_examined()).to_le_bytes(),
                &(checked.induction.certificates().len() as u64).to_le_bytes(),
                &checked.induction.work_units().to_le_bytes(),
            ] {
                hash_frame(&mut hash, field, budget)?;
            }
        }
        budget.charge_work(32)?;
        let identity: [u8; 32] = hash.finalize().into();
        if middle.roster_identity() != identity {
            return Err(E::Mismatch("canonical ranked roster identity"));
        }
        let retained = codec_storage
            .checked_add(source_storage.retained_storage())
            .and_then(|n| n.checked_add(root_storage))
            .and_then(|n| n.checked_add(staging_storage))
            .ok_or(Resource::Arithmetic)?;
        Ok((
            ValidatedNativeCompilerSourceProofV1 {
                source,
                middle,
                correspondence,
                verus,
                roots,
            },
            NativeCompilerSourceProofStorageV1(retained),
        ))
    }));
    if token != budget.work_ledger_identity_v1() || slot != budget as *const Budget<'_> as usize {
        drop(result);
        return Err(Resource::Accounting.into());
    }
    let Some(release) = budget.storage().checked_sub(floor) else {
        drop(result);
        return Err(Resource::Accounting.into());
    };
    if let Err(error) = budget.release_storage(release) {
        drop(result);
        return Err(error.into());
    }
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

#[cfg(test)]
#[path = "compiler_native_source_proof_v1_tests.rs"]
mod tests;
