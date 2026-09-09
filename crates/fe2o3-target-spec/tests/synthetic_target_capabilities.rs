use fe2o3_target_spec::*;

const PROFILE: TargetProfileSpecV1 = SYNTHETIC_CONFORMANCE_TARGET_PROFILE_V1;
const MODEL: TargetCapabilityModelIdentityV1 = SYNTHETIC_CONFORMANCE_TARGET_MODEL_V1;
const U16: TargetScalarTypeV1 = TargetScalarTypeV1::new(TargetScalarKindV1::UnsignedInteger, 16);
const F32: TargetScalarTypeV1 = TargetScalarTypeV1::new(TargetScalarKindV1::Float, 32);

fn outcome(requirement: TargetCapabilityRequirementV1) -> TargetCapabilityDecisionOutcomeV1 {
    let decision = query_target_capability_v1(&SyntheticConformanceTargetV1, requirement)
        .expect("synthetic target and test requirements are valid");
    let replay = query_target_capability_v1(&SyntheticConformanceTargetV1, requirement)
        .expect("synthetic target query replay must remain valid");
    assert_eq!(decision, replay);
    assert_eq!(decision.model(), MODEL);
    assert_eq!(decision.requirement(), requirement);
    decision.outcome()
}

fn assert_supported(requirement: TargetCapabilityRequirementV1) {
    assert_eq!(
        outcome(requirement),
        TargetCapabilityDecisionOutcomeV1::Supported,
        "expected support for {requirement}"
    );
}

fn assert_unsupported(requirement: TargetCapabilityRequirementV1) {
    assert_eq!(
        outcome(requirement),
        TargetCapabilityDecisionOutcomeV1::Unsupported,
        "expected rejection for {requirement}"
    );
}

fn atomic(bit_width: u16) -> TargetCapabilityRequirementV1 {
    TargetCapabilityRequirementV1::Atomic(TargetAtomicRequirementV1::new(
        TargetScalarTypeV1::new(TargetScalarKindV1::UnsignedInteger, bit_width),
        TargetAtomicOperationV1::Add,
        TargetMemoryOrderingV1::Relaxed,
        TargetMemoryScopeV1::Device,
        TargetAddressSpaceV1::Global,
    ))
}

fn reviewed_matrix() -> TargetCapabilityRequirementV1 {
    TargetCapabilityRequirementV1::Matrix(TargetMatrixRequirementV1::with_complete_contract(
        TargetMatrixOperationV1::TensorContraction,
        TargetMatrixShapeV1::new(8, 4, 2),
        F32,
        F32,
        F32,
        TargetMatrixLayoutsV1::new(
            TargetMatrixLayoutV1::RowMajor,
            TargetMatrixLayoutV1::RowMajor,
            TargetMatrixLayoutV1::ColumnMajor,
        ),
        TargetNumericalModeV1::AllowContraction,
        16,
        8,
    ))
}

#[test]
fn synthetic_target_accepts_its_reviewed_contract() {
    assert_supported(TargetCapabilityRequirementV1::SubgroupSize(8));
    assert_supported(TargetCapabilityRequirementV1::SubgroupSize(16));
    assert_supported(TargetCapabilityRequirementV1::ScalarType(U16));
    assert_supported(TargetCapabilityRequirementV1::ScalarType(F32));
    assert_supported(TargetCapabilityRequirementV1::AddressSpace(
        TargetAddressSpaceV1::Global,
        TargetMemoryAccessV1::ReadWrite,
    ));
    assert_supported(TargetCapabilityRequirementV1::AddressSpace(
        TargetAddressSpaceV1::Constant,
        TargetMemoryAccessV1::Read,
    ));
    assert_supported(atomic(16));
    assert_supported(TargetCapabilityRequirementV1::Barrier(
        TargetBarrierRequirementV1::new(
            TargetExecutionScopeV1::Workgroup,
            TargetMemoryScopeV1::Workgroup,
            TargetAddressSpaceV1::Workgroup,
            TargetMemoryOrderingV1::AcquireRelease,
            TargetBarrierParticipationV1::Uniform,
        ),
    ));
    assert_supported(TargetCapabilityRequirementV1::Fence(
        TargetFenceRequirementV1::new(
            TargetMemoryScopeV1::Workgroup,
            TargetAddressSpaceV1::Workgroup,
            TargetMemoryOrderingV1::AcquireRelease,
        ),
    ));
    assert_supported(TargetCapabilityRequirementV1::Collective(
        TargetCollectiveRequirementV1::new(
            TargetExecutionScopeV1::Subgroup,
            TargetCollectiveOperationV1::ReduceAdd,
            U16,
            16,
            TargetCollectiveParticipationV1::Full,
            None,
        ),
    ));
    assert_supported(reviewed_matrix());
    assert_supported(TargetCapabilityRequirementV1::AsyncCopy(
        TargetAsyncCopyRequirementV1::new(
            TargetAddressSpaceV1::Global,
            TargetAddressSpaceV1::Workgroup,
            12,
            4,
        ),
    ));
    assert_supported(TargetCapabilityRequirementV1::AsyncWait(
        TargetAsyncWaitRequirementV1::new(
            TargetExecutionScopeV1::Workgroup,
            TargetMemoryScopeV1::Workgroup,
            TargetMemoryOrderingV1::Acquire,
            1,
        ),
    ));
    assert_supported(TargetCapabilityRequirementV1::Numerical(
        TargetNumericalRequirementV1::new(F32, TargetNumericalModeV1::IeeeStrict),
    ));
    assert_supported(TargetCapabilityRequirementV1::Resource(
        TargetResourceRequirementV1::WorkgroupInvocationsAtMost(192),
    ));
    assert_supported(TargetCapabilityRequirementV1::Resource(
        TargetResourceRequirementV1::StaticSharedMemoryBytesAtMost(24 * 1024),
    ));
    assert_supported(TargetCapabilityRequirementV1::Resource(
        TargetResourceRequirementV1::WorkgroupDimensions { x: 96, y: 2, z: 1 },
    ));
    assert_supported(TargetCapabilityRequirementV1::Abi(
        TargetAbiConstraintV1::PointerWidth(32),
    ));
    assert_supported(TargetCapabilityRequirementV1::Abi(
        TargetAbiConstraintV1::Endianness(TargetEndiannessV1::Big),
    ));
    assert_supported(TargetCapabilityRequirementV1::Object(
        TargetObjectConstraintV1::Format(TargetObjectFormatV1::PortableModule),
    ));
    assert_supported(TargetCapabilityRequirementV1::Object(
        TargetObjectConstraintV1::Relocatable(false),
    ));
}

#[test]
fn every_unsupported_axis_is_rejected_independently() {
    assert_unsupported(TargetCapabilityRequirementV1::SubgroupSize(32));
    assert_unsupported(TargetCapabilityRequirementV1::AddressSpace(
        TargetAddressSpaceV1::Generic,
        TargetMemoryAccessV1::Read,
    ));
    assert_unsupported(TargetCapabilityRequirementV1::AddressSpace(
        TargetAddressSpaceV1::Constant,
        TargetMemoryAccessV1::Write,
    ));
    assert_unsupported(atomic(32));
    assert_unsupported(TargetCapabilityRequirementV1::Barrier(
        TargetBarrierRequirementV1::new(
            TargetExecutionScopeV1::Grid,
            TargetMemoryScopeV1::Workgroup,
            TargetAddressSpaceV1::Workgroup,
            TargetMemoryOrderingV1::AcquireRelease,
            TargetBarrierParticipationV1::Uniform,
        ),
    ));
    assert_unsupported(TargetCapabilityRequirementV1::Fence(
        TargetFenceRequirementV1::new(
            TargetMemoryScopeV1::Device,
            TargetAddressSpaceV1::Global,
            TargetMemoryOrderingV1::AcquireRelease,
        ),
    ));
    assert_unsupported(TargetCapabilityRequirementV1::Collective(
        TargetCollectiveRequirementV1::new(
            TargetExecutionScopeV1::Subgroup,
            TargetCollectiveOperationV1::ReduceAdd,
            TargetScalarTypeV1::new(TargetScalarKindV1::Float, 32),
            64,
            TargetCollectiveParticipationV1::Full,
            Some(TargetNumericalModeV1::IeeeStrict),
        ),
    ));
    assert_unsupported(TargetCapabilityRequirementV1::Matrix(
        TargetMatrixRequirementV1::new(
            TargetMatrixShapeV1::new(16, 16, 16),
            TargetScalarTypeV1::new(TargetScalarKindV1::Float, 16),
            TargetScalarTypeV1::new(TargetScalarKindV1::Float, 32),
            TargetMatrixLayoutsV1::new(
                TargetMatrixLayoutV1::RowMajor,
                TargetMatrixLayoutV1::ColumnMajor,
                TargetMatrixLayoutV1::RowMajor,
            ),
            TargetNumericalModeV1::IeeeStrict,
        ),
    ));
    let TargetCapabilityRequirementV1::Matrix(matrix) = reviewed_matrix() else {
        unreachable!()
    };
    assert_unsupported(TargetCapabilityRequirementV1::Matrix(
        TargetMatrixRequirementV1::with_complete_contract(
            matrix.operation(),
            TargetMatrixShapeV1::new(matrix.m(), matrix.n(), matrix.k()),
            matrix.lhs_type(),
            matrix.rhs_type(),
            matrix.accumulator_type(),
            TargetMatrixLayoutsV1::new(
                matrix.lhs_layout(),
                matrix.rhs_layout(),
                matrix.output_layout(),
            ),
            matrix.numerical_mode(),
            matrix.subgroup_size(),
            16,
        ),
    ));
    assert_unsupported(TargetCapabilityRequirementV1::AsyncCopy(
        TargetAsyncCopyRequirementV1::new(
            TargetAddressSpaceV1::Global,
            TargetAddressSpaceV1::Workgroup,
            16,
            4,
        ),
    ));
    assert_unsupported(TargetCapabilityRequirementV1::AsyncWait(
        TargetAsyncWaitRequirementV1::new(
            TargetExecutionScopeV1::Workgroup,
            TargetMemoryScopeV1::Workgroup,
            TargetMemoryOrderingV1::Acquire,
            0,
        ),
    ));
    assert_unsupported(TargetCapabilityRequirementV1::Numerical(
        TargetNumericalRequirementV1::new(
            TargetScalarTypeV1::new(TargetScalarKindV1::Float, 32),
            TargetNumericalModeV1::AllowApproximation,
        ),
    ));
    assert_unsupported(TargetCapabilityRequirementV1::Resource(
        TargetResourceRequirementV1::WorkgroupInvocationsAtMost(193),
    ));
    assert_unsupported(TargetCapabilityRequirementV1::Resource(
        TargetResourceRequirementV1::StaticSharedMemoryBytesAtMost(24 * 1024 + 1),
    ));
    assert_unsupported(TargetCapabilityRequirementV1::Resource(
        TargetResourceRequirementV1::DynamicSharedMemoryBytesAtMost(12 * 1024 + 1),
    ));
    assert_unsupported(TargetCapabilityRequirementV1::Abi(
        TargetAbiConstraintV1::PointerWidth(64),
    ));
    assert_unsupported(TargetCapabilityRequirementV1::Abi(
        TargetAbiConstraintV1::Endianness(TargetEndiannessV1::Little),
    ));
    assert_unsupported(TargetCapabilityRequirementV1::Object(
        TargetObjectConstraintV1::Format(TargetObjectFormatV1::LoadableExecutable),
    ));
    assert_unsupported(TargetCapabilityRequirementV1::Object(
        TargetObjectConstraintV1::Relocatable(true),
    ));
}

#[test]
fn nonfinal_outcomes_are_not_collapsed_into_support() {
    assert_eq!(
        outcome(TargetCapabilityRequirementV1::Resource(
            TargetResourceRequirementV1::DynamicSharedMemoryBytesAtMost(1024),
        )),
        TargetCapabilityDecisionOutcomeV1::DynamicLaunchEvidenceRequired(
            TargetLaunchEvidenceKindV1::DynamicSharedMemoryBytes,
        )
    );
    assert_eq!(
        outcome(TargetCapabilityRequirementV1::Resource(
            TargetResourceRequirementV1::PrivateMemoryBytesPerInvocationAtMost(64),
        )),
        TargetCapabilityDecisionOutcomeV1::Incomplete
    );
    assert_eq!(
        outcome(TargetCapabilityRequirementV1::Abi(
            TargetAbiConstraintV1::KernelArgumentSegmentBytesAtMost(4096),
        )),
        TargetCapabilityDecisionOutcomeV1::Unreviewed
    );
}

#[test]
fn model_and_decision_identities_are_canonical_and_deterministic() {
    assert_eq!(PROFILE.vendor(), TargetVendorV1::Other);
    assert_eq!(
        PROFILE.architecture_family(),
        TargetArchitectureFamilyV1::Other
    );
    assert_eq!(PROFILE.rustc_target(), None);
    assert_eq!(PROFILE.llvm_target(), None);
    assert_eq!(PROFILE.data_layout(), None);
    assert_eq!(PROFILE.artifact_format(), TargetArtifactFormatV1::Unknown);
    assert_eq!(MODEL.validate(), Ok(()));
    assert_eq!(
        TargetCapabilityModelIdentityV1::new(PROFILE, "synthetic-conformance-v1"),
        Ok(MODEL)
    );
    assert_eq!(SyntheticConformanceTargetV1.model_identity(), MODEL);
    assert_eq!(
        MODEL.to_string(),
        "fe2o3.target-capability-model.v1;profile-fingerprint=ca74f09fe4905e7c66e6bfafe4ee4ad23df167bac81f0007f325be6273f41411;revision-fingerprint=b447385124f03b7e4c5bae18c5df0c3460aa07f540247a8244ae20208fd83945"
    );

    let requirement = atomic(16);
    let first = query_target_capability_v1(&SyntheticConformanceTargetV1, requirement)
        .expect("valid query")
        .to_string();
    let second = query_target_capability_v1(&SyntheticConformanceTargetV1, requirement)
        .expect("valid query")
        .to_string();
    assert_eq!(first, second);
    assert!(first.starts_with("fe2o3.target-capability-decision.v1;"));
    assert!(first.contains(
        "requirement=[atomic:unsigned-integer:16:twos-complement:add:relaxed:<none>:device:global]"
    ));
    assert!(first.ends_with(";outcome=supported"));
}

#[derive(Clone, Copy)]
struct InvalidIdentityTarget;

impl TargetCapabilityQueryV1 for InvalidIdentityTarget {
    fn model_identity(&self) -> TargetCapabilityModelIdentityV1 {
        TargetCapabilityModelIdentityV1::new_unchecked(PROFILE, "NOT-CANONICAL")
    }

    fn query_outcome(
        &self,
        _requirement: TargetCapabilityRequirementV1,
    ) -> TargetCapabilityDecisionOutcomeV1 {
        panic!("an invalid model must be rejected before its query implementation runs")
    }
}

#[test]
fn invalid_models_and_requirements_cannot_mint_decisions() {
    assert!(matches!(
        query_target_capability_v1(
            &InvalidIdentityTarget,
            TargetCapabilityRequirementV1::SubgroupSize(32)
        ),
        Err(TargetCapabilityQueryErrorV1::InvalidModel(
            TargetCapabilityModelIdentityErrorV1::NonCanonicalRevision
        ))
    ));

    let invalid_requirements = [
        TargetCapabilityRequirementV1::ScalarType(TargetScalarTypeV1::new(
            TargetScalarKindV1::Float,
            0,
        )),
        TargetCapabilityRequirementV1::SubgroupSize(0),
        TargetCapabilityRequirementV1::Atomic(TargetAtomicRequirementV1::new(
            TargetScalarTypeV1::new(TargetScalarKindV1::UnsignedInteger, 32),
            TargetAtomicOperationV1::CompareExchange,
            TargetMemoryOrderingV1::AcquireRelease,
            TargetMemoryScopeV1::Device,
            TargetAddressSpaceV1::Global,
        )),
        TargetCapabilityRequirementV1::Atomic(TargetAtomicRequirementV1::compare_exchange(
            TargetScalarTypeV1::new(TargetScalarKindV1::UnsignedInteger, 32),
            TargetMemoryOrderingV1::Release,
            TargetMemoryOrderingV1::Acquire,
            TargetMemoryScopeV1::Device,
            TargetAddressSpaceV1::Global,
        )),
        TargetCapabilityRequirementV1::Fence(TargetFenceRequirementV1::new(
            TargetMemoryScopeV1::Device,
            TargetAddressSpaceV1::Global,
            TargetMemoryOrderingV1::Relaxed,
        )),
        TargetCapabilityRequirementV1::Collective(TargetCollectiveRequirementV1::new(
            TargetExecutionScopeV1::Subgroup,
            TargetCollectiveOperationV1::ReduceAdd,
            TargetScalarTypeV1::new(TargetScalarKindV1::Float, 32),
            0,
            TargetCollectiveParticipationV1::Full,
            Some(TargetNumericalModeV1::IeeeStrict),
        )),
        TargetCapabilityRequirementV1::AsyncCopy(TargetAsyncCopyRequirementV1::new(
            TargetAddressSpaceV1::Global,
            TargetAddressSpaceV1::Workgroup,
            16,
            3,
        )),
        TargetCapabilityRequirementV1::Resource(
            TargetResourceRequirementV1::WorkgroupInvocationsAtMost(0),
        ),
        TargetCapabilityRequirementV1::Abi(TargetAbiConstraintV1::KernelArgumentAlignmentAtMost(3)),
    ];

    for requirement in invalid_requirements {
        assert!(matches!(
            query_target_capability_v1(&SyntheticConformanceTargetV1, requirement),
            Err(TargetCapabilityQueryErrorV1::InvalidRequirement(_))
        ));
    }
}

const CLOSURE_SCALAR: TargetCapabilityRequirementV1 =
    TargetCapabilityRequirementV1::ScalarType(U16);
const CLOSURE_SUBGROUP: TargetCapabilityRequirementV1 =
    TargetCapabilityRequirementV1::SubgroupSize(16);
const CLOSURE_COLLECTIVE: TargetCapabilityRequirementV1 =
    TargetCapabilityRequirementV1::Collective(TargetCollectiveRequirementV1::new(
        TargetExecutionScopeV1::Subgroup,
        TargetCollectiveOperationV1::ReduceAdd,
        U16,
        16,
        TargetCollectiveParticipationV1::Full,
        None,
    ));
const CLOSURE_DEPENDENCIES: &[TargetCapabilityRequirementV1] = &[CLOSURE_SCALAR, CLOSURE_SUBGROUP];
const CLOSURE_ROOTS: &[TargetCapabilityRequirementV1] = &[CLOSURE_COLLECTIVE];
const CLOSURE_NODES: &[TargetCapabilityClosureNodeV1<'static>] = &[
    TargetCapabilityClosureNodeV1::new(CLOSURE_SCALAR, &[]),
    TargetCapabilityClosureNodeV1::new(CLOSURE_SUBGROUP, &[]),
    TargetCapabilityClosureNodeV1::new(CLOSURE_COLLECTIVE, CLOSURE_DEPENDENCIES),
];

fn complete_closure_spec() -> TargetCapabilityClosureSpecV1<'static> {
    TargetCapabilityClosureSpecV1::new(CLOSURE_ROOTS, CLOSURE_NODES)
}

#[test]
fn exact_closure_is_static_no_alloc_and_deterministic() {
    let first = admit_static_target_capability_closure_v1(
        &SyntheticConformanceTargetV1,
        complete_closure_spec(),
    )
    .expect("synthetic closure must be statically admitted");
    let second = admit_static_target_capability_closure_v1(
        &SyntheticConformanceTargetV1,
        complete_closure_spec(),
    )
    .expect("replayed synthetic closure must be admitted");

    assert_eq!(first, second);
    assert_eq!(first.len(), 3);
    assert!(!first.is_empty());
    assert_eq!(first.model(), MODEL);
    assert_eq!(first.spec(), complete_closure_spec());
    for index in 0..first.len() {
        assert_eq!(
            first.decision(index).unwrap().outcome(),
            TargetCapabilityDecisionOutcomeV1::Supported
        );
    }
    assert_eq!(first.decision(first.len()), None);
    assert_eq!(first.first_dynamic_requirement(), None);

    let canonical = first.to_string();
    assert_eq!(canonical, second.to_string());
    assert!(canonical.starts_with("fe2o3.target-capability-closure.v1;"));
    assert!(canonical.contains(
        "collective:subgroup:reduce-add:unsigned-integer:16:twos-complement:16:full:<none>"
    ));
    for forbidden in ["amd", "gfx", "hip", "hsa", "cuda", "spir-v", "spirv"] {
        assert!(
            !canonical.to_ascii_lowercase().contains(forbidden),
            "{forbidden}"
        );
    }
}

#[test]
fn neutral_capability_source_and_records_have_no_backend_vocabulary_or_space_numbers() {
    let neutral_sources = [
        (
            "target capability contract",
            include_str!("../src/capability_v1.rs"),
        ),
        (
            "synthetic conformance model",
            include_str!("../src/synthetic_conformance_v1.rs"),
        ),
        (
            "source memory capabilities",
            include_str!("../../fe2o3-device/src/capability_memory.rs"),
        ),
        (
            "source execution capabilities",
            include_str!("../../fe2o3-device/src/execution.rs"),
        ),
        (
            "source numerical capabilities",
            include_str!("../../fe2o3-device/src/numerical.rs"),
        ),
        (
            "canonical execution capability wrapper",
            include_str!("../../fe2o3-kernel-ir/src/execution_capability_v1.rs"),
        ),
        (
            "canonical KIR V13 owner",
            include_str!("../../fe2o3-kernel-ir/src/canonical_kir_v13.rs"),
        ),
        (
            "target-neutral optimization report",
            include_str!("../../fe2o3-kernel-opt/src/optimization_v6.rs"),
        ),
        (
            "target-neutral structural replay",
            include_str!("../../fe2o3-kernel-opt/src/structural_replay_admission_v6.rs"),
        ),
        (
            "target-neutral cost model",
            include_str!("../../fe2o3-kernel-opt/src/target_neutral_cost_v1.rs"),
        ),
        (
            "target-neutral module transform records",
            include_str!("../../fe2o3-kernel-opt/src/target_neutral_module_transforms_v1.rs"),
        ),
        (
            "transformation preservation records",
            include_str!("../../fe2o3-kernel-opt/src/transformation_preservation_v1.rs"),
        ),
        (
            "canonical capability evidence",
            include_str!("../../fe2o3-proof-contracts/src/capability.rs"),
        ),
    ];
    let forbidden_identifiers = [
        "amd",
        "amdgcn",
        "amdhsa",
        "hip",
        "hsa",
        "cuda",
        "spirv",
        "wave",
        "wavefront",
        "warp",
        "mfma",
        "s_barrier",
        "buffer_load",
        "buffer_store",
    ];
    let contains_identifier = |source: &str, identifier: &str| {
        source.match_indices(identifier).any(|(start, _)| {
            let is_identifier_byte = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
            let end = start + identifier.len();
            !source[..start]
                .bytes()
                .next_back()
                .is_some_and(is_identifier_byte)
                && !source[end..].bytes().next().is_some_and(is_identifier_byte)
        })
    };
    let contains_gfx_profile = |source: &str| {
        source.match_indices("gfx").any(|(start, _)| {
            source[start + 3..]
                .bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_digit())
        })
    };
    let forbidden_space_numbers = [
        "addrspace(1)",
        "addrspace(3)",
        "addrspace(4)",
        "addrspace(5)",
        "address space 1",
        "address space 3",
        "address space 4",
        "address space 5",
    ];

    for (layer, source) in neutral_sources {
        let source = source.to_ascii_lowercase();
        for forbidden in forbidden_identifiers {
            assert!(
                !contains_identifier(&source, forbidden),
                "{layer} contains {forbidden}"
            );
        }
        assert!(
            !contains_gfx_profile(&source),
            "{layer} contains a gfx profile"
        );
        assert!(!source.contains("spir-v"), "{layer} contains spir-v");
        for forbidden in forbidden_space_numbers {
            assert!(!source.contains(forbidden), "{layer} contains {forbidden}");
        }
    }

    let requirements = [
        TargetCapabilityRequirementV1::ScalarType(F32),
        TargetCapabilityRequirementV1::SubgroupSize(16),
        TargetCapabilityRequirementV1::AddressSpace(
            TargetAddressSpaceV1::Constant,
            TargetMemoryAccessV1::Read,
        ),
        atomic(16),
        TargetCapabilityRequirementV1::Barrier(TargetBarrierRequirementV1::new(
            TargetExecutionScopeV1::Workgroup,
            TargetMemoryScopeV1::Workgroup,
            TargetAddressSpaceV1::Workgroup,
            TargetMemoryOrderingV1::AcquireRelease,
            TargetBarrierParticipationV1::Uniform,
        )),
        TargetCapabilityRequirementV1::Fence(TargetFenceRequirementV1::new(
            TargetMemoryScopeV1::Workgroup,
            TargetAddressSpaceV1::Workgroup,
            TargetMemoryOrderingV1::AcquireRelease,
        )),
        TargetCapabilityRequirementV1::Collective(TargetCollectiveRequirementV1::new(
            TargetExecutionScopeV1::Subgroup,
            TargetCollectiveOperationV1::ReduceAdd,
            U16,
            16,
            TargetCollectiveParticipationV1::Full,
            None,
        )),
        reviewed_matrix(),
        TargetCapabilityRequirementV1::AsyncCopy(TargetAsyncCopyRequirementV1::new(
            TargetAddressSpaceV1::Global,
            TargetAddressSpaceV1::Workgroup,
            12,
            4,
        )),
        TargetCapabilityRequirementV1::AsyncWait(TargetAsyncWaitRequirementV1::new(
            TargetExecutionScopeV1::Workgroup,
            TargetMemoryScopeV1::Workgroup,
            TargetMemoryOrderingV1::Acquire,
            1,
        )),
        TargetCapabilityRequirementV1::Numerical(TargetNumericalRequirementV1::new(
            F32,
            TargetNumericalModeV1::IeeeStrict,
        )),
        TargetCapabilityRequirementV1::Resource(
            TargetResourceRequirementV1::WorkgroupInvocationsAtMost(192),
        ),
        TargetCapabilityRequirementV1::Abi(TargetAbiConstraintV1::PointerWidth(32)),
        TargetCapabilityRequirementV1::Object(TargetObjectConstraintV1::Format(
            TargetObjectFormatV1::PortableModule,
        )),
    ];
    for requirement in requirements {
        let record = requirement.to_string().to_ascii_lowercase();
        for forbidden in forbidden_identifiers {
            assert!(
                !contains_identifier(&record, forbidden),
                "{record} contains {forbidden}"
            );
        }
        assert!(
            !contains_gfx_profile(&record),
            "{record} contains a gfx profile"
        );
        assert!(!record.contains("spir-v"), "{record} contains spir-v");
        for forbidden in forbidden_space_numbers {
            assert!(!record.contains(forbidden), "{record} contains {forbidden}");
        }
    }

    for space in [
        TargetAddressSpaceV1::Global,
        TargetAddressSpaceV1::Workgroup,
        TargetAddressSpaceV1::Private,
        TargetAddressSpaceV1::Constant,
        TargetAddressSpaceV1::Generic,
    ] {
        let record = TargetCapabilityRequirementV1::AddressSpace(space, TargetMemoryAccessV1::Read)
            .to_string();
        let suffix = record.strip_prefix("address-space:").unwrap();
        assert!(!suffix.as_bytes()[0].is_ascii_digit(), "{record}");
    }
}

#[derive(Clone, Copy)]
struct OmittedTarget;

impl TargetCapabilityQueryV1 for OmittedTarget {
    fn model_identity(&self) -> TargetCapabilityModelIdentityV1 {
        MODEL
    }

    fn query_outcome(
        &self,
        requirement: TargetCapabilityRequirementV1,
    ) -> TargetCapabilityDecisionOutcomeV1 {
        SyntheticConformanceTargetV1.query_outcome(requirement)
    }

    fn query_outcome_checked(
        &self,
        requirement: TargetCapabilityRequirementV1,
    ) -> Option<TargetCapabilityDecisionOutcomeV1> {
        if requirement == CLOSURE_SUBGROUP {
            None
        } else {
            Some(Self::query_outcome(self, requirement))
        }
    }
}

struct NondeterministicTarget(core::cell::Cell<bool>);

impl TargetCapabilityQueryV1 for NondeterministicTarget {
    fn model_identity(&self) -> TargetCapabilityModelIdentityV1 {
        MODEL
    }

    fn query_outcome(
        &self,
        _requirement: TargetCapabilityRequirementV1,
    ) -> TargetCapabilityDecisionOutcomeV1 {
        let prior = self.0.replace(!self.0.get());
        if prior {
            TargetCapabilityDecisionOutcomeV1::Supported
        } else {
            TargetCapabilityDecisionOutcomeV1::Unsupported
        }
    }
}

fn singleton_spec(
    requirement: &'static [TargetCapabilityRequirementV1; 1],
    nodes: &'static [TargetCapabilityClosureNodeV1<'static>; 1],
) -> TargetCapabilityClosureSpecV1<'static> {
    TargetCapabilityClosureSpecV1::new(requirement, nodes)
}

#[test]
fn closure_rejects_omitted_nondeterministic_and_nonfinal_answers() {
    assert!(matches!(
        query_target_capability_closure_v1(&OmittedTarget, complete_closure_spec()),
        Err(TargetCapabilityClosureErrorV1::Query {
            requirement: CLOSURE_SUBGROUP,
            source: TargetCapabilityQueryErrorV1::OmittedAnswer(CLOSURE_SUBGROUP),
        })
    ));

    let target = NondeterministicTarget(core::cell::Cell::new(false));
    assert_eq!(
        query_target_capability_closure_v1(&target, complete_closure_spec()),
        Err(TargetCapabilityClosureErrorV1::NondeterministicAnswer(
            CLOSURE_SCALAR
        ))
    );

    const INCOMPLETE: [TargetCapabilityRequirementV1; 1] =
        [TargetCapabilityRequirementV1::Resource(
            TargetResourceRequirementV1::PrivateMemoryBytesPerInvocationAtMost(1),
        )];
    const INCOMPLETE_NODES: [TargetCapabilityClosureNodeV1<'static>; 1] =
        [TargetCapabilityClosureNodeV1::new(INCOMPLETE[0], &[])];
    assert_eq!(
        query_target_capability_closure_v1(
            &SyntheticConformanceTargetV1,
            singleton_spec(&INCOMPLETE, &INCOMPLETE_NODES),
        ),
        Err(TargetCapabilityClosureErrorV1::Incomplete(INCOMPLETE[0]))
    );

    const UNREVIEWED: [TargetCapabilityRequirementV1; 1] = [TargetCapabilityRequirementV1::Abi(
        TargetAbiConstraintV1::KernelArgumentSegmentBytesAtMost(1),
    )];
    const UNREVIEWED_NODES: [TargetCapabilityClosureNodeV1<'static>; 1] =
        [TargetCapabilityClosureNodeV1::new(UNREVIEWED[0], &[])];
    assert_eq!(
        query_target_capability_closure_v1(
            &SyntheticConformanceTargetV1,
            singleton_spec(&UNREVIEWED, &UNREVIEWED_NODES),
        ),
        Err(TargetCapabilityClosureErrorV1::Unreviewed(UNREVIEWED[0]))
    );

    const UNSUPPORTED: [TargetCapabilityRequirementV1; 1] =
        [TargetCapabilityRequirementV1::SubgroupSize(64)];
    const UNSUPPORTED_NODES: [TargetCapabilityClosureNodeV1<'static>; 1] =
        [TargetCapabilityClosureNodeV1::new(UNSUPPORTED[0], &[])];
    assert_eq!(
        query_target_capability_closure_v1(
            &SyntheticConformanceTargetV1,
            singleton_spec(&UNSUPPORTED, &UNSUPPORTED_NODES),
        ),
        Err(TargetCapabilityClosureErrorV1::Unsupported(UNSUPPORTED[0]))
    );
}

#[test]
fn launch_dependent_closure_is_resolved_but_not_statically_admitted() {
    const DYNAMIC: [TargetCapabilityRequirementV1; 1] = [TargetCapabilityRequirementV1::Resource(
        TargetResourceRequirementV1::DynamicSharedMemoryBytesAtMost(1024),
    )];
    const DYNAMIC_NODES: [TargetCapabilityClosureNodeV1<'static>; 1] =
        [TargetCapabilityClosureNodeV1::new(DYNAMIC[0], &[])];
    let spec = singleton_spec(&DYNAMIC, &DYNAMIC_NODES);
    let closure = query_target_capability_closure_v1(&SyntheticConformanceTargetV1, spec).unwrap();
    assert_eq!(
        closure.first_dynamic_requirement(),
        Some((
            DYNAMIC[0],
            TargetLaunchEvidenceKindV1::DynamicSharedMemoryBytes
        ))
    );
    assert_eq!(
        admit_static_target_capability_closure_v1(&SyntheticConformanceTargetV1, spec),
        Err(
            TargetCapabilityClosureErrorV1::DynamicLaunchEvidenceRequired {
                requirement: DYNAMIC[0],
                kind: TargetLaunchEvidenceKindV1::DynamicSharedMemoryBytes,
            }
        )
    );
}

#[test]
fn closure_graph_validation_rejects_cycles_omissions_and_noncanonical_graphs() {
    const CYCLE_ROOTS: [TargetCapabilityRequirementV1; 1] = [CLOSURE_SCALAR];
    const SCALAR_TO_SUBGROUP: [TargetCapabilityRequirementV1; 1] = [CLOSURE_SUBGROUP];
    const SUBGROUP_TO_SCALAR: [TargetCapabilityRequirementV1; 1] = [CLOSURE_SCALAR];
    const CYCLE_NODES: [TargetCapabilityClosureNodeV1<'static>; 2] = [
        TargetCapabilityClosureNodeV1::new(CLOSURE_SCALAR, &SCALAR_TO_SUBGROUP),
        TargetCapabilityClosureNodeV1::new(CLOSURE_SUBGROUP, &SUBGROUP_TO_SCALAR),
    ];
    assert!(matches!(
        query_target_capability_closure_v1(
            &SyntheticConformanceTargetV1,
            TargetCapabilityClosureSpecV1::new(&CYCLE_ROOTS, &CYCLE_NODES),
        ),
        Err(TargetCapabilityClosureErrorV1::DependencyCycle(_))
    ));

    const MISSING_ROOTS: [TargetCapabilityRequirementV1; 1] = [CLOSURE_COLLECTIVE];
    const MISSING_NODES: [TargetCapabilityClosureNodeV1<'static>; 1] =
        [TargetCapabilityClosureNodeV1::new(
            CLOSURE_COLLECTIVE,
            &[CLOSURE_SCALAR],
        )];
    assert_eq!(
        query_target_capability_closure_v1(
            &SyntheticConformanceTargetV1,
            TargetCapabilityClosureSpecV1::new(&MISSING_ROOTS, &MISSING_NODES),
        ),
        Err(TargetCapabilityClosureErrorV1::MissingDependency {
            requirement: CLOSURE_COLLECTIVE,
            dependency: CLOSURE_SCALAR,
        })
    );

    const EXTRA_ROOTS: [TargetCapabilityRequirementV1; 1] = [CLOSURE_SCALAR];
    const EXTRA_NODES: [TargetCapabilityClosureNodeV1<'static>; 2] = [
        TargetCapabilityClosureNodeV1::new(CLOSURE_SCALAR, &[]),
        TargetCapabilityClosureNodeV1::new(CLOSURE_SUBGROUP, &[]),
    ];
    assert_eq!(
        query_target_capability_closure_v1(
            &SyntheticConformanceTargetV1,
            TargetCapabilityClosureSpecV1::new(&EXTRA_ROOTS, &EXTRA_NODES),
        ),
        Err(TargetCapabilityClosureErrorV1::UnreachableNode(
            CLOSURE_SUBGROUP
        ))
    );

    const UNSORTED_ROOTS: [TargetCapabilityRequirementV1; 2] = [CLOSURE_SUBGROUP, CLOSURE_SCALAR];
    assert_eq!(
        query_target_capability_closure_v1(
            &SyntheticConformanceTargetV1,
            TargetCapabilityClosureSpecV1::new(&UNSORTED_ROOTS, CLOSURE_NODES),
        ),
        Err(TargetCapabilityClosureErrorV1::NonCanonicalRootOrder)
    );

    let too_many = vec![
        TargetCapabilityClosureNodeV1::new(CLOSURE_SCALAR, &[]);
        MAX_TARGET_CAPABILITY_CLOSURE_REQUIREMENTS_V1 + 1
    ];
    assert_eq!(
        query_target_capability_closure_v1(
            &SyntheticConformanceTargetV1,
            TargetCapabilityClosureSpecV1::new(&EXTRA_ROOTS, &too_many),
        ),
        Err(TargetCapabilityClosureErrorV1::TooManyRequirements {
            count: MAX_TARGET_CAPABILITY_CLOSURE_REQUIREMENTS_V1 + 1,
            maximum: MAX_TARGET_CAPABILITY_CLOSURE_REQUIREMENTS_V1,
        })
    );
}

#[test]
fn query_is_deterministic_across_a_bounded_requirement_property_matrix() {
    for width in 1..=128 {
        let requirement = TargetCapabilityRequirementV1::SubgroupSize(width);
        let first = query_target_capability_v1(&SyntheticConformanceTargetV1, requirement).unwrap();
        let second =
            query_target_capability_v1(&SyntheticConformanceTargetV1, requirement).unwrap();
        assert_eq!(first, second, "subgroup width {width}");
    }
    for bits in [8, 16, 32, 64, 128] {
        for scope in [
            TargetMemoryScopeV1::Subgroup,
            TargetMemoryScopeV1::Workgroup,
            TargetMemoryScopeV1::Device,
            TargetMemoryScopeV1::System,
        ] {
            let requirement =
                TargetCapabilityRequirementV1::Atomic(TargetAtomicRequirementV1::new(
                    TargetScalarTypeV1::new(TargetScalarKindV1::UnsignedInteger, bits),
                    TargetAtomicOperationV1::Add,
                    TargetMemoryOrderingV1::Relaxed,
                    scope,
                    TargetAddressSpaceV1::Global,
                ));
            assert_eq!(
                query_target_capability_v1(&SyntheticConformanceTargetV1, requirement),
                query_target_capability_v1(&SyntheticConformanceTargetV1, requirement)
            );
        }
    }
}
