//! Hostile and deterministic tests for worker handoff admission V1.

use fe2o3_llvm_handoff::{
    DecodeHandoffErrorV1, DeviceLibraryInputV1, DeviceLibraryKindV1, FunctionAttributeV1,
    Gfx942HandoffInputV1, Gfx942HandoffV1, Gfx942TargetPolicyV1, HandoffDiagnosticV1, IdentityV1,
    KernelEntryV1, ModuleFlagV1, ModuleMetadataV1, ObligationKindV1, ObligationV1, OriginKindV1,
    OriginV1, StageIdentitiesV1, WireSectionV1, WorkgroupSizeRangeV1,
};
use fe2o3_llvm_worker_handoff::{
    EXACT_LLD_BUILD_IDENTITY_V1, EXACT_LLD_VERSION_V1, EXACT_LLVM_BUILD_IDENTITY_V1,
    EXACT_LLVM_VERSION_V1, MAX_WORKER_ADMISSION_REQUEST_BYTES_V1,
    MAX_WORKER_BUILD_VERSION_BYTES_V1, MAX_WORKER_DEVICE_LIBRARY_BYTES_V1, MeasuredLlvmLldBuildV1,
    WorkerAdmissionErrorV1, WorkerAdmissionRequestV1, WorkerBuildFieldV1,
};

fn identity(byte: u8) -> IdentityV1 {
    IdentityV1::new([byte; 32]).unwrap()
}

fn library(kind: DeviceLibraryKindV1, byte: u8, byte_len: u64) -> DeviceLibraryInputV1 {
    DeviceLibraryInputV1::new(kind, [byte; 32], byte_len).unwrap()
}

fn supported_libraries() -> Vec<DeviceLibraryInputV1> {
    vec![
        library(DeviceLibraryKindV1::Ocml, 0x41, 1_024),
        library(DeviceLibraryKindV1::OclcIsaVersion942, 0x42, 2_048),
        library(DeviceLibraryKindV1::OclcFiniteOnlyOff, 0x43, 512),
        library(DeviceLibraryKindV1::OclcUnsafeMathOff, 0x44, 512),
    ]
}

fn handoff_with_libraries(libraries: Vec<DeviceLibraryInputV1>) -> Gfx942HandoffV1 {
    let origin = OriginV1::new(OriginKindV1::AmdgcnIr, identity(0x11), None);
    let kernel = KernelEntryV1::new(
        "admission_kernel",
        vec![],
        FunctionAttributeV1::gfx942_kernel_defaults(WorkgroupSizeRangeV1::new(64, 256).unwrap()),
        origin.identity(),
    )
    .unwrap();
    let obligation = ObligationV1::new(
        ObligationKindV1::PreserveTargetFeatures,
        identity(0x12),
        origin.identity(),
    );
    Gfx942HandoffV1::new(Gfx942HandoffInputV1 {
        stage_identities: StageIdentitiesV1::new([1; 32], [2; 32], [3; 32]).unwrap(),
        target: Gfx942TargetPolicyV1::canonical(),
        kernels: vec![kernel],
        module: ModuleMetadataV1::new(
            vec![ModuleFlagV1::CodeObjectVersion6, ModuleFlagV1::PicLevel2],
            vec![],
            libraries,
        )
        .unwrap(),
        origins: vec![origin],
        obligations: vec![obligation],
    })
    .unwrap()
}

fn handoff() -> Gfx942HandoffV1 {
    handoff_with_libraries(vec![])
}

fn admit_with_build(
    handoff: &Gfx942HandoffV1,
    build: MeasuredLlvmLldBuildV1<'_>,
) -> Result<fe2o3_llvm_worker_handoff::AdmittedWorkerRequestV1, WorkerAdmissionErrorV1> {
    let bytes = handoff.encode_canonical();
    WorkerAdmissionRequestV1::new(bytes.as_bytes(), *handoff.identity().as_bytes(), build).admit()
}

fn admit_bytes(
    bytes: &[u8],
    claimed_identity: [u8; 32],
) -> Result<fe2o3_llvm_worker_handoff::AdmittedWorkerRequestV1, WorkerAdmissionErrorV1> {
    WorkerAdmissionRequestV1::new(bytes, claimed_identity, MeasuredLlvmLldBuildV1::exact()).admit()
}

#[test]
fn exact_request_is_inert_and_preserves_canonical_identity() {
    let handoff = handoff();
    let admitted = admit_with_build(&handoff, MeasuredLlvmLldBuildV1::exact()).unwrap();

    assert_eq!(admitted.handoff(), &handoff);
    assert_eq!(admitted.handoff_identity(), handoff.identity());
    assert_eq!(
        admitted.handoff().encode_canonical(),
        handoff.encode_canonical()
    );
    assert_eq!(
        admitted.build_identity().llvm_version(),
        EXACT_LLVM_VERSION_V1
    );
    assert_eq!(
        admitted.build_identity().llvm_build_identity(),
        EXACT_LLVM_BUILD_IDENTITY_V1
    );
    assert_eq!(
        admitted.build_identity().lld_version(),
        EXACT_LLD_VERSION_V1
    );
    assert_eq!(
        admitted.build_identity().lld_build_identity(),
        EXACT_LLD_BUILD_IDENTITY_V1
    );
    assert!(admitted.build_identity().in_process_lld());
    assert!(!admitted.grants_object_authority());
    assert!(!admitted.grants_link_authority());
    assert!(!admitted.grants_publication_authority());
}

#[test]
fn admission_identity_is_deterministic_and_order_independent() {
    let handoff = handoff_with_libraries(supported_libraries());
    let mut reversed = supported_libraries();
    reversed.reverse();
    let reordered = handoff_with_libraries(reversed);
    assert_eq!(handoff, reordered);

    let first = admit_with_build(&handoff, MeasuredLlvmLldBuildV1::exact()).unwrap();
    let second = admit_with_build(&reordered, MeasuredLlvmLldBuildV1::exact()).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.admission_identity(), second.admission_identity());
    assert_ne!(first.admission_identity().as_bytes(), &[0; 32]);
    assert_eq!(first.admission_identity().to_string().len(), 64);
}

#[test]
fn request_bound_and_claimed_identity_fail_closed_before_decode() {
    let oversized = vec![0; MAX_WORKER_ADMISSION_REQUEST_BYTES_V1 + 1];
    assert_eq!(
        admit_bytes(&oversized, [1; 32]),
        Err(WorkerAdmissionErrorV1::RequestTooLong {
            observed: MAX_WORKER_ADMISSION_REQUEST_BYTES_V1 + 1,
            maximum: MAX_WORKER_ADMISSION_REQUEST_BYTES_V1,
        })
    );

    let canonical = handoff().encode_canonical();
    assert_eq!(
        admit_bytes(canonical.as_bytes(), [0; 32]),
        Err(WorkerAdmissionErrorV1::ZeroHandoffIdentity)
    );
}

#[test]
fn unknown_and_substituted_target_wire_families_fail_closed() {
    let handoff = handoff();
    let canonical = handoff.encode_canonical();
    let cases = [
        (16, WireSectionV1::TargetTriple),
        (17, WireSectionV1::DataLayout),
        (18, WireSectionV1::Cpu),
        (20, WireSectionV1::TargetFeature),
        (26, WireSectionV1::CodeObjectPolicy),
        (27, WireSectionV1::OptimizationLevel),
        (28, WireSectionV1::RelocationModel),
        (29, WireSectionV1::CodeModel),
    ];
    for (offset, section) in cases {
        let mut hostile = canonical.as_bytes().to_vec();
        hostile[offset] = 0xff;
        assert_eq!(
            admit_bytes(&hostile, *handoff.identity().as_bytes()),
            Err(WorkerAdmissionErrorV1::Decode(
                DecodeHandoffErrorV1::UnknownTag { section, tag: 0xff }
            )),
            "mutation at offset {offset} did not fail in {section:?}"
        );
    }

    let mut substituted_feature = canonical.as_bytes().to_vec();
    substituted_feature[21] = 1;
    assert_eq!(
        admit_bytes(&substituted_feature, *handoff.identity().as_bytes()),
        Err(WorkerAdmissionErrorV1::Decode(
            DecodeHandoffErrorV1::InvalidModel(HandoffDiagnosticV1::UnsupportedTargetPolicy)
        ))
    );

    let mut conflicting_feature = canonical.as_bytes().to_vec();
    conflicting_feature[22] = 1;
    assert_eq!(
        admit_bytes(&conflicting_feature, *handoff.identity().as_bytes()),
        Err(WorkerAdmissionErrorV1::Decode(
            DecodeHandoffErrorV1::InvalidModel(HandoffDiagnosticV1::ConflictingTargetFeature)
        ))
    );
}

#[test]
fn zero_and_mismatched_canonical_identities_fail_closed() {
    let handoff = handoff();
    let canonical = handoff.encode_canonical();

    let mut zero_semantic_identity = canonical.as_bytes().to_vec();
    zero_semantic_identity[30..62].fill(0);
    assert_eq!(
        admit_bytes(&zero_semantic_identity, *handoff.identity().as_bytes()),
        Err(WorkerAdmissionErrorV1::Decode(
            DecodeHandoffErrorV1::InvalidModel(HandoffDiagnosticV1::ZeroIdentity("semantic"))
        ))
    );

    assert_eq!(
        admit_bytes(canonical.as_bytes(), [0x99; 32]),
        Err(WorkerAdmissionErrorV1::HandoffIdentityMismatch)
    );

    let mut changed_semantic_identity = canonical.as_bytes().to_vec();
    changed_semantic_identity[30] ^= 1;
    assert_eq!(
        admit_bytes(&changed_semantic_identity, *handoff.identity().as_bytes()),
        Err(WorkerAdmissionErrorV1::HandoffIdentityMismatch)
    );
}

#[test]
fn every_build_policy_substitution_is_rejected() {
    let handoff = handoff();
    let cases = [
        (
            MeasuredLlvmLldBuildV1::new(
                "22.1.9",
                EXACT_LLVM_BUILD_IDENTITY_V1,
                EXACT_LLD_VERSION_V1,
                EXACT_LLD_BUILD_IDENTITY_V1,
                true,
            ),
            WorkerBuildFieldV1::LlvmVersion,
        ),
        (
            MeasuredLlvmLldBuildV1::new(
                EXACT_LLVM_VERSION_V1,
                "upstream-llvmorg-22.1.8-substituted",
                EXACT_LLD_VERSION_V1,
                EXACT_LLD_BUILD_IDENTITY_V1,
                true,
            ),
            WorkerBuildFieldV1::LlvmBuildIdentity,
        ),
        (
            MeasuredLlvmLldBuildV1::new(
                EXACT_LLVM_VERSION_V1,
                EXACT_LLVM_BUILD_IDENTITY_V1,
                "22.1.9",
                EXACT_LLD_BUILD_IDENTITY_V1,
                true,
            ),
            WorkerBuildFieldV1::LldVersion,
        ),
        (
            MeasuredLlvmLldBuildV1::new(
                EXACT_LLVM_VERSION_V1,
                EXACT_LLVM_BUILD_IDENTITY_V1,
                EXACT_LLD_VERSION_V1,
                "upstream-llvmorg-22.1.8-substituted",
                true,
            ),
            WorkerBuildFieldV1::LldBuildIdentity,
        ),
        (
            MeasuredLlvmLldBuildV1::new(
                EXACT_LLVM_VERSION_V1,
                EXACT_LLVM_BUILD_IDENTITY_V1,
                EXACT_LLD_VERSION_V1,
                EXACT_LLD_BUILD_IDENTITY_V1,
                false,
            ),
            WorkerBuildFieldV1::InProcessLld,
        ),
    ];
    for (build, field) in cases {
        assert_eq!(
            admit_with_build(&handoff, build),
            Err(WorkerAdmissionErrorV1::BuildIdentitySubstitution(field))
        );
    }
}

#[test]
fn build_diagnostics_are_typed_and_bounded() {
    let handoff = handoff();
    let oversized = "v".repeat(MAX_WORKER_BUILD_VERSION_BYTES_V1 + 1);
    let build = MeasuredLlvmLldBuildV1::new(
        &oversized,
        EXACT_LLVM_BUILD_IDENTITY_V1,
        EXACT_LLD_VERSION_V1,
        EXACT_LLD_BUILD_IDENTITY_V1,
        true,
    );
    assert_eq!(
        admit_with_build(&handoff, build),
        Err(WorkerAdmissionErrorV1::BuildFieldTooLong {
            field: WorkerBuildFieldV1::LlvmVersion,
            observed: MAX_WORKER_BUILD_VERSION_BYTES_V1 + 1,
            maximum: MAX_WORKER_BUILD_VERSION_BYTES_V1,
        })
    );

    let build = MeasuredLlvmLldBuildV1::new(
        "22.1.8\n",
        EXACT_LLVM_BUILD_IDENTITY_V1,
        EXACT_LLD_VERSION_V1,
        EXACT_LLD_BUILD_IDENTITY_V1,
        true,
    );
    assert_eq!(
        admit_with_build(&handoff, build),
        Err(WorkerAdmissionErrorV1::InvalidBuildField(
            WorkerBuildFieldV1::LlvmVersion
        ))
    );
}

#[test]
fn unsupported_incomplete_and_oversized_device_libraries_are_rejected() {
    let unsupported = handoff_with_libraries(vec![library(DeviceLibraryKindV1::Ockl, 0x51, 1_024)]);
    assert_eq!(
        admit_with_build(&unsupported, MeasuredLlvmLldBuildV1::exact()),
        Err(WorkerAdmissionErrorV1::UnsupportedDeviceLibrary(
            DeviceLibraryKindV1::Ockl
        ))
    );

    let incomplete = handoff_with_libraries(vec![library(DeviceLibraryKindV1::Ocml, 0x52, 1_024)]);
    assert_eq!(
        admit_with_build(&incomplete, MeasuredLlvmLldBuildV1::exact()),
        Err(WorkerAdmissionErrorV1::IncompleteDeviceLibraryClosure {
            observed: 1,
            required: 4,
        })
    );

    let mut oversized = supported_libraries();
    oversized[0] = library(
        DeviceLibraryKindV1::Ocml,
        0x53,
        MAX_WORKER_DEVICE_LIBRARY_BYTES_V1 + 1,
    );
    let oversized = handoff_with_libraries(oversized);
    assert_eq!(
        admit_with_build(&oversized, MeasuredLlvmLldBuildV1::exact()),
        Err(WorkerAdmissionErrorV1::DeviceLibraryTooLong {
            kind: DeviceLibraryKindV1::Ocml,
            observed: MAX_WORKER_DEVICE_LIBRARY_BYTES_V1 + 1,
            maximum: MAX_WORKER_DEVICE_LIBRARY_BYTES_V1,
        })
    );
}
