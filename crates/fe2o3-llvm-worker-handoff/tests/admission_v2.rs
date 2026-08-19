//! Hostile and deterministic tests for inert typed V2 worker admission.

use fe2o3_llvm_handoff::{
    BasicBlockV2, BlockIdV2, CallingConventionV2, EvidenceV2, ExecutableModuleV2,
    FunctionAttributeV1, FunctionAttributeV2, FunctionIdV2, FunctionKindV2, FunctionV2,
    Gfx942HandoffInputV1, Gfx942HandoffV1, Gfx942HandoffV2, Gfx942TargetPolicyV1, IdentityV1,
    KernelEntryV1, ModuleFlagV1, ModuleMetadataV1, ObligationKindV1, ObligationV1, OriginKindV1,
    OriginV1, ReturnTypeV2, StageIdentitiesV1, TerminatorV2, WorkgroupSizeRangeV1,
};
use fe2o3_llvm_worker_handoff::{
    EXACT_LLD_BUILD_IDENTITY_V1, EXACT_LLD_VERSION_V1, EXACT_LLVM_BUILD_IDENTITY_V1,
    MAX_WORKER_ADMISSION_REQUEST_BYTES_V2, MeasuredLlvmLldBuildV1, WorkerAdmissionErrorV1,
    WorkerAdmissionErrorV2, WorkerAdmissionRequestV2, WorkerBuildFieldV1,
};

fn identity(byte: u8) -> IdentityV1 {
    IdentityV1::new([byte; 32]).unwrap()
}

fn fixture(source_byte: u8) -> Gfx942HandoffV2 {
    let origin = OriginV1::new(OriginKindV1::AmdgcnIr, identity(source_byte), None);
    let attributes =
        FunctionAttributeV1::gfx942_kernel_defaults(WorkgroupSizeRangeV1::new(1, 64).unwrap());
    let kernel = KernelEntryV1::new(
        "admission_v2_kernel",
        vec![],
        attributes.clone(),
        origin.identity(),
    )
    .unwrap();
    let obligation = ObligationV1::new(
        ObligationKindV1::PreserveKernelAbi,
        identity(0x42),
        origin.identity(),
    );
    let base = Gfx942HandoffV1::new(Gfx942HandoffInputV1 {
        stage_identities: StageIdentitiesV1::new([1; 32], [2; 32], [3; 32]).unwrap(),
        target: Gfx942TargetPolicyV1::canonical(),
        kernels: vec![kernel],
        module: ModuleMetadataV1::new(
            vec![ModuleFlagV1::CodeObjectVersion6, ModuleFlagV1::PicLevel2],
            vec![],
            vec![],
        )
        .unwrap(),
        origins: vec![origin],
        obligations: vec![obligation],
    })
    .unwrap();
    let evidence = EvidenceV2::new(
        base.origins()[0].identity(),
        base.obligations()
            .iter()
            .map(|value| value.identity())
            .collect(),
    )
    .unwrap();
    let function = FunctionV2::new(
        FunctionIdV2::new(1),
        "admission_v2_kernel",
        FunctionKindV2::Kernel,
        CallingConventionV2::AmdGpuKernel,
        ReturnTypeV2::Void,
        vec![],
        attributes
            .into_iter()
            .map(FunctionAttributeV2::from)
            .collect(),
        BlockIdV2::new(0),
        vec![BasicBlockV2::new(
            BlockIdV2::new(0),
            vec![],
            TerminatorV2::Return(None),
        )],
        evidence,
    )
    .unwrap();
    let module = ExecutableModuleV2::new(
        base.module().flags().to_vec(),
        base.module().named_metadata().to_vec(),
        vec![],
        vec![],
        vec![function],
    )
    .unwrap();
    Gfx942HandoffV2::new(base, module).unwrap()
}

fn admit(
    handoff: &Gfx942HandoffV2,
    build: MeasuredLlvmLldBuildV1<'_>,
) -> Result<fe2o3_llvm_worker_handoff::AdmittedWorkerRequestV2, WorkerAdmissionErrorV2> {
    let bytes = handoff.encode_canonical();
    WorkerAdmissionRequestV2::new(bytes.as_bytes(), *handoff.identity().as_bytes(), build).admit()
}

#[test]
fn exact_v2_request_retains_graph_and_grants_no_authority() {
    let handoff = fixture(0x31);
    let admitted = admit(&handoff, MeasuredLlvmLldBuildV1::exact()).unwrap();

    assert_eq!(admitted.handoff(), &handoff);
    assert_eq!(admitted.handoff_identity(), handoff.identity());
    assert_ne!(admitted.admission_identity().as_bytes(), &[0; 32]);
    assert_eq!(admitted.admission_identity().to_string().len(), 64);
    assert!(!admitted.authenticates_worker_measurement());
    assert!(!admitted.grants_object_authority());
    assert!(!admitted.grants_link_authority());
    assert!(!admitted.grants_publication_authority());
    assert!(!admitted.grants_load_authority());
    assert!(!admitted.grants_launch_authority());
}

#[test]
fn v2_bound_zero_claim_and_identity_substitution_fail_closed() {
    let oversized = vec![0; MAX_WORKER_ADMISSION_REQUEST_BYTES_V2 + 1];
    assert_eq!(
        WorkerAdmissionRequestV2::new(&oversized, [1; 32], MeasuredLlvmLldBuildV1::exact()).admit(),
        Err(WorkerAdmissionErrorV2::RequestTooLong {
            observed: MAX_WORKER_ADMISSION_REQUEST_BYTES_V2 + 1,
            maximum: MAX_WORKER_ADMISSION_REQUEST_BYTES_V2,
        })
    );

    let handoff = fixture(0x31);
    let bytes = handoff.encode_canonical();
    assert_eq!(
        WorkerAdmissionRequestV2::new(bytes.as_bytes(), [0; 32], MeasuredLlvmLldBuildV1::exact())
            .admit(),
        Err(WorkerAdmissionErrorV2::ZeroHandoffIdentity)
    );
    assert_eq!(
        WorkerAdmissionRequestV2::new(
            bytes.as_bytes(),
            [0x99; 32],
            MeasuredLlvmLldBuildV1::exact()
        )
        .admit(),
        Err(WorkerAdmissionErrorV2::HandoffIdentityMismatch)
    );
}

#[test]
fn truncated_and_trailing_v2_encodings_are_rejected_by_canonical_decode() {
    let handoff = fixture(0x31);
    let bytes = handoff.encode_canonical();
    for length in [0, 1, bytes.len() - 1] {
        assert!(matches!(
            WorkerAdmissionRequestV2::new(
                &bytes.as_bytes()[..length],
                *handoff.identity().as_bytes(),
                MeasuredLlvmLldBuildV1::exact(),
            )
            .admit(),
            Err(WorkerAdmissionErrorV2::Decode(_))
        ));
    }

    let mut trailing = bytes.as_bytes().to_vec();
    trailing.push(0);
    assert!(matches!(
        WorkerAdmissionRequestV2::new(
            &trailing,
            *handoff.identity().as_bytes(),
            MeasuredLlvmLldBuildV1::exact(),
        )
        .admit(),
        Err(WorkerAdmissionErrorV2::Decode(_))
    ));
}

#[test]
fn build_policy_admission_is_not_measurement() {
    let handoff = fixture(0x31);
    let substituted = MeasuredLlvmLldBuildV1::new(
        "22.1.9",
        EXACT_LLVM_BUILD_IDENTITY_V1,
        EXACT_LLD_VERSION_V1,
        EXACT_LLD_BUILD_IDENTITY_V1,
        true,
    );
    assert_eq!(
        admit(&handoff, substituted),
        Err(WorkerAdmissionErrorV2::Policy(
            WorkerAdmissionErrorV1::BuildIdentitySubstitution(WorkerBuildFieldV1::LlvmVersion)
        ))
    );
}

#[test]
fn source_graph_identity_changes_v2_admission_identity() {
    let first = admit(&fixture(0x31), MeasuredLlvmLldBuildV1::exact()).unwrap();
    let second = admit(&fixture(0x32), MeasuredLlvmLldBuildV1::exact()).unwrap();

    assert_ne!(first.handoff_identity(), second.handoff_identity());
    assert_ne!(first.admission_identity(), second.admission_identity());
}
