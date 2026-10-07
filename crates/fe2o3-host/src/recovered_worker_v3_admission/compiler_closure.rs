//! Immutable compiler evidence validation, separate from live publication custody.

use super::*;

pub(super) struct CompilerClosureValidation {
    pub(super) finalizer_derivation: RevalidatedProtectedWorkerV3FinalizerDerivationV1,
    pub(super) compiler_execution_subject: InertCompilerExecutionSubjectV1,
    pub(super) outer_handoff: InertSemanticCompilerModuleHandoffV3,
    pub(super) inspection: FinalizedDescriptorInspection,
}

pub(super) fn validate(
    wire: &WorkerV3LoadEnvelopeWireV2,
    finalized: &[u8],
) -> Result<CompilerClosureValidation, RecoveredWorkerV3AdmissionErrorV1> {
    let replay = wire.replay();
    let finalizer_derivation = revalidate_protected_worker_v3_finalizer_derivation_v1(
        replay.publication_intent_record().attempt(),
        replay.outer_handoff(),
        replay.external_provider_payloads(),
        replay.transcript(),
        finalized,
    )
    .map_err(RecoveredWorkerV3AdmissionErrorV1::FinalizerDerivation)?;
    validate_finalizer_derivation_association(wire, finalized, &finalizer_derivation)?;
    let inspection = validate_finalized_identity(replay.publication_intent_record(), finalized)?;
    let outer_handoff = InertSemanticCompilerModuleHandoffV3::decode(replay.outer_handoff())
        .map_err(RecoveredWorkerV3AdmissionErrorV1::OuterHandoff)?;
    let compiler_execution_subject = wire
        .reconstructed_compiler_execution_subject_v1()
        .map_err(RecoveredWorkerV3AdmissionErrorV1::Envelope)?;
    validate_compiler_source_and_exports(&outer_handoff, &inspection)?;
    validate_target_and_code_object(&outer_handoff, &inspection)?;
    Ok(CompilerClosureValidation {
        finalizer_derivation,
        compiler_execution_subject,
        outer_handoff,
        inspection,
    })
}

/// Independently checks the complete immutable V2 replay and one exact kernel selection.
///
/// This operation performs no filesystem access, publication recovery or lock acquisition.
/// Callers may pass bytes borrowed from an already-held current token, or transferred copies.
/// Both paths establish byte consistency only: this cannot authenticate the original compiler
/// or application occurrence, transfer proof custody, or establish publication currentness.
pub fn check_worker_v3_compiler_closure_v1<'evidence>(
    exact_canonical_envelope: &'evidence [u8],
    finalized_hsaco: &'evidence [u8],
    kernel_id: KernelId,
) -> Result<CheckedWorkerV3CompilerClosureV1<'evidence>, RecoveredWorkerV3AdmissionErrorV1> {
    let wire = WorkerV3LoadEnvelopeWireV2::decode_canonical(exact_canonical_envelope)
        .map_err(RecoveredWorkerV3AdmissionErrorV1::Envelope)?;
    // Unlike a recovered lease, a public byte slice has no admitted artifact size bound.
    if finalized_hsaco.len() != wire.replay().publication_intent_record().output_length() {
        return Err(RecoveredWorkerV3AdmissionErrorV1::FinalizedLengthMismatch);
    }
    let validation = validate(&wire, finalized_hsaco)?;
    let entrypoint = select_entrypoint_from_evidence(
        &wire,
        &validation.outer_handoff,
        &validation.inspection,
        &validation.compiler_execution_subject,
        &validation.finalizer_derivation,
        kernel_id,
    )?;
    Ok(CheckedWorkerV3CompilerClosureV1 {
        exact_canonical_envelope,
        finalized_hsaco,
        wire,
        validation,
        entrypoint,
    })
}

/// A checked immutable compiler closure, not a recovered publication or executable.
///
/// Retains both exact input borrows and independently reconstructed evidence. It does not
/// retain a publication lock or authenticate the provenance of copied bytes. There is no
/// conversion to a recovered admission, proof owner, or load/launch authority.
///
/// ```compile_fail
/// use fe2o3_host::CheckedWorkerV3CompilerClosureV1;
/// fn escape<'a>(closure: CheckedWorkerV3CompilerClosureV1<'a>)
///     -> CheckedWorkerV3CompilerClosureV1<'static> { closure }
/// ```
///
/// ```compile_fail
/// use fe2o3_host::{CheckedWorkerV3CompilerClosureV1, RecoveredWorkerV3PinnedDescriptorV1};
/// fn promote(closure: CheckedWorkerV3CompilerClosureV1<'_>)
///     -> RecoveredWorkerV3PinnedDescriptorV1 { closure }
/// ```
#[must_use]
pub struct CheckedWorkerV3CompilerClosureV1<'evidence> {
    exact_canonical_envelope: &'evidence [u8],
    finalized_hsaco: &'evidence [u8],
    wire: WorkerV3LoadEnvelopeWireV2,
    validation: CompilerClosureValidation,
    entrypoint: RecoveredWorkerV3EntrypointV1,
}

impl fmt::Debug for CheckedWorkerV3CompilerClosureV1<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CheckedWorkerV3CompilerClosureV1")
            .field("lineage", &self.lineage_identity())
            .field("authority", &"none")
            .finish_non_exhaustive()
    }
}

impl CheckedWorkerV3CompilerClosureV1<'_> {
    pub const fn exact_canonical_envelope_bytes(&self) -> &[u8] {
        self.exact_canonical_envelope
    }

    pub const fn finalized_hsaco_bytes(&self) -> &[u8] {
        self.finalized_hsaco
    }

    pub const fn finalizer_replay(&self) -> &fe2o3_runtime_protocol::WorkerV3LoadEnvelopeWireV1 {
        self.wire.replay()
    }

    pub const fn finalizer_derivation(&self) -> &RevalidatedProtectedWorkerV3FinalizerDerivationV1 {
        &self.validation.finalizer_derivation
    }

    pub const fn compiler_execution_subject(&self) -> &InertCompilerExecutionSubjectV1 {
        &self.validation.compiler_execution_subject
    }

    pub const fn compiler_execution_receipt_carriage(&self) -> &CompilerExecutionReceiptCarriageV1 {
        self.wire.compiler_execution_receipt()
    }

    pub const fn semantic_compiler_handoff(&self) -> &InertSemanticCompilerModuleHandoffV3 {
        &self.validation.outer_handoff
    }

    pub const fn lineage_identity(&self) -> WorkerV3HostLineageIdentityV1 {
        self.entrypoint.lineage_identity()
    }

    pub fn descriptor_table(&self) -> &DeviceDescriptorTableV1 {
        self.validation.inspection.descriptor_table()
    }

    pub fn descriptor(&self) -> &KernelDescriptorV1 {
        &self.descriptor_table().kernels()[self.entrypoint.descriptor_index]
    }

    pub fn physical_kernel(&self) -> &InspectedKernel {
        &self.validation.inspection.hsaco().kernels()[self.entrypoint.physical_kernel_index]
    }

    pub fn descriptor_binding(&self) -> KernelDescriptorBinding {
        self.validation.inspection.kernel_bindings().bindings()
            [self.entrypoint.physical_kernel_index]
    }

    pub fn target(&self) -> AmdTargetId {
        self.validation.inspection.hsaco().target()
    }

    pub fn code_object_version(&self) -> CodeObjectVersion {
        self.validation.inspection.hsaco().code_object_version()
    }

    pub const fn grants_currentness_authority(&self) -> bool {
        false
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}
