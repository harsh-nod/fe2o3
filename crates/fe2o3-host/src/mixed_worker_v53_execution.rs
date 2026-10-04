//! Private V53 proof-to-execution contract. No approved implementation is installed.
//! Content replay and current-record signatures alone never enter this transition.
use super::*;
use crate::{
    GeneratedWorkerV3KfdExecutionError, WorkerV3CompilerCurrentRecordAuditV1,
    WorkerV3CompilerExecutionVerificationV1, WorkerV3ProtectedSemanticMachineRefinementEvidenceV1,
};
use fe2o3_amd_target::{AmdTargetId, PRODUCTION_GFX942_DEVICE_TARGET_V1};
use fe2o3_kernel_ir::{CanonicalKernelIrWorkLedgerIdentityV1, VerifiedCanonicalKernelIrModuleV18};
use fe2o3_kfd::CheckedGfx942XnackMinusDevice;
use fe2o3_runtime::{
    Gfx942AuthorizedRuntimeDispatchResultV1, WorkerV3Gfx942ExecutionAuthorityV1,
    execute_authorized_gfx942_runtime_dispatch_v1,
};

#[path = "mixed_worker_v53_native_evidence.rs"]
mod evidence;
pub(crate) use evidence::ReservedNativeEvidenceV53;

/// Fixed coordinates accompany the concrete borrowed request. Their equality is
/// a substitution check, not a theorem or approval of the claimed producer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NativeSubjectV53 {
    pub(crate) versions: [u16; 5],
    pub(crate) capsule: [u8; 32],
    pub(crate) descriptor: [u8; 32],
    pub(crate) typed_receipt: [u8; 32],
    pub(crate) source: [[u8; 32]; 2],
    pub(crate) graphs: [([u8; 32], u64); 4],
    pub(crate) compiler_subject: [u8; 32],
    pub(crate) compiler_carriage: [u8; 32],
    pub(crate) current_record: [u8; 32],
    pub(crate) artifact: ([u8; 32], u64),
    pub(crate) ordinal: usize,
    pub(crate) invocation: Gfx942RuntimeInvocationBindingV1,
    pub(crate) dispatch_contract: [u8; 32],
    pub(crate) prepared_content: [u8; 32],
    pub(crate) device_unique_id: u64,
}
const VERSIONS: [u16; 5] = [53, 50, 26, 18, 6];

/// Only created after complete roster, source receipt, final graph, native text,
/// descriptor, target, current record and generated-argument joins. The final
/// graph is the same live owner used by target selection and native replay.
pub(crate) struct NativeVerificationRequestV53<'a, R> {
    subject: NativeSubjectV53,
    lineage: MixedWorkerV53VerificationRequest<'a, R>,
    final_graph: &'a VerifiedCanonicalKernelIrModuleV18,
    compiler: &'a WorkerV3CompilerExecutionVerificationV1,
    invocation: &'a PreparedGfx942RuntimeDispatchV1,
    device: &'a CheckedGfx942XnackMinusDevice,
}
impl<R> NativeVerificationRequestV53<'_, R> {
    pub(crate) fn subject(&self) -> NativeSubjectV53 {
        self.subject
    }
    pub(crate) fn lineage(&self) -> &MixedWorkerV53VerificationRequest<'_, R> {
        &self.lineage
    }
    pub(crate) fn final_graph(&self) -> &VerifiedCanonicalKernelIrModuleV18 {
        self.final_graph
    }
    pub(crate) fn compiler(&self) -> &WorkerV3CompilerExecutionVerificationV1 {
        self.compiler
    }
    pub(crate) fn invocation(&self) -> &PreparedGfx942RuntimeDispatchV1 {
        self.invocation
    }
    pub(crate) fn device(&self) -> &CheckedGfx942XnackMinusDevice {
        self.device
    }
}

/// An extension contract, not a production verification implementation.
///
/// # Safety
/// A successful implementation must independently authenticate the approved
/// protected compiler, current-record trust policy and measured proof runtime,
/// including the exact V50 signed receipt, its source identities, prefix witness,
/// all four graphs and every root in the V53 roster. Self-signatures and successful
/// content replay do not satisfy this requirement. It must verify a theorem from
/// those exact source semantics through the supplied live V18 graph, final LLVM,
/// target ISA and finalized HSACO, including ABI, addresses, effects, control and
/// numeric behavior. LLVM is not assumed trusted by this interface.
///
/// It must instantiate that theorem for the SAME immutable prepared invocation:
/// complete kernarg template, buffer bytes/access/length, fixups, geometry, V26
/// premises, initialization, alias/race freedom, bounds and completion effects,
/// and the exact checked gfx942 device. Any concurrent state assumption must be
/// secured by retained owners through dispatch and quiescence, not sampled then
/// released. Evidence must remain valid while these owners are retained and must
/// not rely on a backend borrow surviving this call. It must reject unsupported
/// schemas, targets and missing protected authority; no V1 receipt or fallback.
///
/// The returned evidence must authenticate request.subject(), not merely echo it.
/// Its boxed payload reservation must use this same ledger. Backend computation
/// and allocations require their own finite approved policy; the host reservation
/// accounts only retained evidence, not provider RSS. No implementation is shipped.
pub(crate) unsafe trait ProtectedNativeBackendV53 {
    type Error: std::error::Error + Send + Sync + 'static;
    fn verify<R: CompilerGeneratedKernelExpectationRosterV1>(
        &mut self,
        request: NativeVerificationRequestV53<'_, R>,
        budget: &mut Budget<'_>,
    ) -> std::result::Result<ReservedNativeEvidenceV53, Self::Error>;
}

fn digest(bytes: &[u8], budget: &mut Budget<'_>) -> Result<[u8; 32]> {
    budget.charge_work(bytes.len().checked_add(1).ok_or(Resource::Arithmetic)?)?;
    Ok(Sha256::digest(bytes).into())
}

fn require_execution_target(target: AmdTargetId) -> Result<()> {
    if target != AmdTargetId::parse(PRODUCTION_GFX942_DEVICE_TARGET_V1).map_err(codec_error)? {
        return Err(AdmissionError::UnsupportedTarget);
    }
    Ok(())
}

/// Length-delimited complete immutable preparation, not just the premise digest.
fn prepared_content(
    prepared: &PreparedGfx942RuntimeDispatchV1,
    budget: &mut Budget<'_>,
) -> Result<[u8; 32]> {
    let view = prepared.inspection_v1();
    let mut hash = Sha256::new();
    hash.update(b"FE2O3/HOST/MIXED-PREPARED-CONTENT/V53\0");
    for word in [
        view.descriptor_offset,
        view.kernarg_alignment,
        u64::from(view.private_segment_size),
        u64::from(view.group_segment_size),
        u64::from(view.timeout_milliseconds),
    ] {
        hash.update(word.to_le_bytes());
    }
    for word in view.geometry.grid() {
        hash.update(word.to_le_bytes());
    }
    for word in view.geometry.workgroup() {
        hash.update(word.to_le_bytes());
    }
    hash.update(view.geometry.dimensions().to_le_bytes());
    for bytes in [view.executable_image, view.kernarg_template] {
        hash.update(
            u64::try_from(bytes.len())
                .map_err(|_| Resource::Arithmetic)?
                .to_le_bytes(),
        );
        hash.update(digest(bytes, budget)?);
    }
    hash.update(
        u64::try_from(view.buffers.len())
            .map_err(|_| Resource::Arithmetic)?
            .to_le_bytes(),
    );
    let policies = prepared.buffer_policies_v1();
    if policies.len() != view.buffers.len() {
        return Err(binding("V53 buffer policy extent"));
    }
    for (buffer, (access, length)) in view.buffers.iter().zip(policies) {
        budget.charge_work(1)?;
        let access = match access {
            fe2o3_runtime::Gfx942RuntimeBufferAccessV1::ReadOnly => 1u8,
            fe2o3_runtime::Gfx942RuntimeBufferAccessV1::WriteOnly => 2,
            fe2o3_runtime::Gfx942RuntimeBufferAccessV1::ReadWrite => 3,
        };
        if length != u64::try_from(buffer.bytes().len()).map_err(|_| Resource::Arithmetic)? {
            return Err(binding("V53 buffer policy length"));
        }
        hash.update([access]);
        hash.update(length.to_le_bytes());
        hash.update(digest(buffer.bytes(), budget)?);
    }
    hash.update(
        u64::try_from(view.pointer_fixups.len())
            .map_err(|_| Resource::Arithmetic)?
            .to_le_bytes(),
    );
    for fixup in view.pointer_fixups {
        budget.charge_work(1)?;
        for word in [
            u64::try_from(fixup.kernarg_offset()).map_err(|_| Resource::Arithmetic)?,
            u64::try_from(fixup.buffer_index()).map_err(|_| Resource::Arithmetic)?,
            u64::try_from(fixup.buffer_byte_offset()).map_err(|_| Resource::Arithmetic)?,
            fixup.required_alignment(),
        ] {
            hash.update(word.to_le_bytes());
        }
    }
    Ok(hash.finalize().into())
}

struct NativeAuthorityV53<R, K> {
    owner: RecoveredMixedWorkerV53PinnedRoster<R>,
    current: DurableCurrentLinkPublicationTokenV1,
    _compiler: WorkerV3CompilerExecutionVerificationV1,
    _evidence: WorkerV3ProtectedSemanticMachineRefinementEvidenceV1,
    subject: NativeSubjectV53,
    _marker: PhantomData<fn() -> K>,
}

// SAFETY: only the consuming transition below constructs this private owner,
// after the unsafe backend's full V53 theorem contract and exact subject join.
// The runtime request/device cannot be extracted or exchanged by callers.
unsafe impl<R, K: CompilerGeneratedKernelExpectationV1> WorkerV3Gfx942ExecutionAuthorityV1
    for NativeAuthorityV53<R, K>
{
    type CurrentnessError = AdmissionError;
    fn finalized_hsaco_sha256(&self) -> [u8; 32] {
        self.subject.artifact.0
    }
    fn finalized_hsaco_length(&self) -> u64 {
        self.subject.artifact.1
    }
    fn kernel_name(&self) -> &str {
        K::EXPORT_NAME
    }
    fn dispatch_contract_sha256(&self) -> [u8; 32] {
        self.subject.dispatch_contract
    }
    fn invocation_binding(&self) -> Gfx942RuntimeInvocationBindingV1 {
        self.subject.invocation
    }
    fn device_unique_id(&self) -> u64 {
        self.subject.device_unique_id
    }
    fn revalidate_currentness(&self) -> Result<()> {
        self.owner.check_current(&self.current)
    }
}

struct ExecutionPartsV53<'a, R, K> {
    authority: NativeAuthorityV53<R, K>,
    device: CheckedGfx942XnackMinusDevice,
    prepared: PreparedGfx942RuntimeDispatchV1,
    completion: GeneratedKfdCompletion<'a>,
}

/// Private, move-only and budget-borrowing. Drop retires evidence before refunding
/// its charge. The older preparation charge remains the caller's responsibility.
#[must_use]
pub(crate) struct AuthorizedMixedInvocationV53<'a, 'b, 'w, R, K> {
    parts: Option<ExecutionPartsV53<'a, R, K>>,
    _reservation: EvidenceStorageLease<'b, 'w>,
}
struct EvidenceStorageLease<'b, 'w> {
    budget: &'b mut Budget<'w>,
    retained: usize,
}
impl Drop for EvidenceStorageLease<'_, '_> {
    fn drop(&mut self) {
        // Exclusive budget borrow prevents any caller refund or ledger exchange.
        self.budget
            .release_storage(self.retained)
            .expect("private V53 evidence reservation");
    }
}
impl<R, K: CompilerGeneratedKernelExpectationV1> AuthorizedMixedInvocationV53<'_, '_, '_, R, K> {
    pub(crate) fn execute(
        mut self,
    ) -> std::result::Result<
        Gfx942AuthorizedRuntimeDispatchResultV1,
        GeneratedWorkerV3KfdExecutionError,
    > {
        let ExecutionPartsV53 {
            authority,
            device,
            prepared,
            completion,
        } = self.parts.take().expect("one-shot V53 owner");
        let result = execute_authorized_gfx942_runtime_dispatch_v1(authority, device, prepared)
            .map_err(GeneratedWorkerV3KfdExecutionError::Runtime)?;
        completion
            .apply(result)
            .map_err(GeneratedWorkerV3KfdExecutionError::Completion)
    }
}

impl<'a, R: CompilerGeneratedKernelExpectationRosterV1, K: CompilerGeneratedKernelExpectationV1>
    PreparedMixedWorkerV53Invocation<'a, R, K>
{
    /// No production backend is installed. This crate-private transition cannot
    /// be selected from the ordinary public preparation API or a fixture signer.
    pub(crate) fn authorize_native<'b, 'w, B: ProtectedNativeBackendV53>(
        self,
        audit: WorkerV3CompilerCurrentRecordAuditV1,
        mut device: CheckedGfx942XnackMinusDevice,
        backend: &mut B,
        budget: &'b mut Budget<'w>,
    ) -> Result<AuthorizedMixedInvocationV53<'a, 'b, 'w, R, K>> {
        require_execution_target(self.owner.descriptor.bindings.inspection().target())?;
        self.revalidate_currentness()?;
        device.check_observable_currentness().map_err(codec_error)?;
        let marker = CompilerGeneratedKernelExpectationRosterEntryV1::for_marker::<K>();
        let ordinal = R::ENTRIES
            .iter()
            .position(|entry| *entry == marker)
            .ok_or_else(|| binding("V53 execution marker absent from exact roster"))?;
        let outer = &self.owner.custody.outer_handoff;
        let table = self.owner.descriptor_table()?;
        let audit_storage = size_of::<WorkerV3CompilerCurrentRecordAuditV1>();
        // The codec envelope accounts callback/result headers separately. These
        // are the additional coexisting local views and streaming hash states.
        let scratch = audit_storage
            .checked_add(
                fe2o3_kernel_descriptor::mixed_conditional_v26::MIXED_CONTRACT_CODEC_STORAGE_V26,
            )
            .and_then(|n| n.checked_add(2 * size_of::<Sha256>()))
            .and_then(|n| n.checked_add(size_of::<NativeVerificationRequestV53<'_, R>>()))
            .and_then(|n| n.checked_add(size_of::<NativeSubjectV53>()))
            .and_then(|n| n.checked_add(size_of::<InertTypedSourceReceiptV53<'_>>()))
            .and_then(|n| n.checked_add(size_of::<fe2o3_kfd::Gfx942KfdDispatchInspectionV1<'_>>()))
            .ok_or(Resource::Arithmetic)?;
        let ((compiler, evidence, subject), retained) =
            codec_on_budget(budget, scratch, |budget| {
                let compiler = audit
                    .bind_exact_compiler_execution_v1(
                        self.owner.compiler_execution_subject(),
                        self.owner.compiler_execution_receipt(),
                    )
                    .map_err(codec_error)?;
                if !compiler.authenticates_signed_currentness_evidence() {
                    return Err(binding(
                        "V53 requires opaque signed current-record evidence",
                    ));
                }
                let receipt = self.owner.typed_receipt(budget)?;
                let contract = table
                    .contract(ordinal, &mut |n| budget.charge_work(n))
                    .map_err(codec_error)?;
                require_mixed_contract(self.prepared.invocation_binding(), *contract.identity())
                    .map_err(|_| binding("V53 exact selected contract"))?;
                if self.prepared.kernel_name() != K::EXPORT_NAME
                    || self.prepared.identity().object_sha256()
                        != digest(self.current.exact_artifact_bytes(), budget)?
                {
                    return Err(binding("V53 prepared executable substitution"));
                }
                let subject = NativeSubjectV53 {
                    versions: VERSIONS,
                    capsule: digest(outer.capsule().canonical_bytes(), budget)?,
                    descriptor: digest(table.canonical_bytes(), budget)?,
                    typed_receipt: digest(receipt.canonical_bytes(), budget)?,
                    source: receipt.source_identities(),
                    graphs: receipt.graph_identities(),
                    compiler_subject: compiler.subject_sha256(),
                    compiler_carriage: compiler.carriage_sha256(),
                    current_record: compiler.current_record_verification_sha256(),
                    artifact: (
                        self.prepared.identity().object_sha256(),
                        self.prepared.finalized_hsaco_length(),
                    ),
                    ordinal,
                    invocation: self.prepared.invocation_binding(),
                    dispatch_contract: self.prepared.dispatch_contract_sha256(),
                    prepared_content: prepared_content(&self.prepared, budget)?,
                    device_unique_id: device.observation().unique_id(),
                };
                let (evidence, evidence_storage) =
                    codec_on_budget(budget, MIXED_MIDDLE_END_WORKING_STORAGE_V50, |budget| {
                        with_validated_lineage_on_budget(
                            outer,
                            &table,
                            ProductionAmdTargetProfileV1::Gfx942,
                            budget,
                            |final_graph, budget| {
                                self.revalidate_currentness()?;
                                let request = NativeVerificationRequestV53 {
                                    subject,
                                    lineage: MixedWorkerV53VerificationRequest {
                                        owner: &self.owner,
                                        current: &self.current,
                                    },
                                    final_graph,
                                    compiler: &compiler,
                                    invocation: &self.prepared,
                                    device: &device,
                                };
                                let evidence =
                                    backend.verify(request, budget).map_err(codec_error)?;
                                evidence.into_checked(subject, budget)
                            },
                        )
                    })?;
                // Nested codec scopes returned an unreserved owner. Rejoin its actual
                // boxed backing before keeping it alive outside those scopes.
                budget.reserve_storage(evidence_storage)?;
                self.revalidate_currentness()?;
                device.check_observable_currentness().map_err(codec_error)?;
                Ok((
                    (compiler, evidence, subject),
                    audit_storage
                        .checked_add(evidence_storage)
                        .ok_or(Resource::Arithmetic)?,
                ))
            })?;
        drop(table);
        let retained = retained
            .checked_add(size_of::<AuthorizedMixedInvocationV53<'a, 'b, 'w, R, K>>())
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(retained)?;
        let Self {
            owner,
            current,
            prepared,
            _completion: completion,
            _marker: _,
        } = self;
        let authority = NativeAuthorityV53 {
            owner,
            current,
            _compiler: compiler,
            _evidence: evidence,
            subject,
            _marker: PhantomData,
        };
        Ok(AuthorizedMixedInvocationV53 {
            parts: Some(ExecutionPartsV53 {
                authority,
                device,
                prepared,
                completion,
            }),
            _reservation: EvidenceStorageLease { budget, retained },
        })
    }
}

#[cfg(test)]
#[path = "mixed_worker_v53_execution_tests.rs"]
mod tests;
