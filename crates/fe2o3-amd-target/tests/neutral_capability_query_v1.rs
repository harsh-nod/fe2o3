use fe2o3_amd_target::{
    PRODUCTION_AMD_NEUTRAL_CAPABILITY_MODEL_REVISION_V1, ProductionAmdCapabilityOwnerV1,
    ProductionAmdTargetCapabilityModelV1, ProductionAmdTargetProfileV1,
};
use fe2o3_target_spec::*;

use TargetCapabilityDecisionOutcomeV1::{
    DynamicLaunchEvidenceRequired, Incomplete, Supported, Unsupported,
};

const PROFILES: [ProductionAmdTargetProfileV1; 2] = [
    ProductionAmdTargetProfileV1::Gfx942,
    ProductionAmdTargetProfileV1::Gfx950,
];

fn model(profile: ProductionAmdTargetProfileV1) -> ProductionAmdTargetCapabilityModelV1 {
    profile.capability_model().unwrap()
}

fn outcome(
    model: &ProductionAmdTargetCapabilityModelV1,
    requirement: TargetCapabilityRequirementV1,
) -> TargetCapabilityDecisionOutcomeV1 {
    let decision = query_target_capability_v1(model, requirement).unwrap();
    assert_eq!(decision.model(), model.model_identity());
    assert_eq!(decision.requirement(), requirement);
    decision.outcome()
}

const fn integer(kind: TargetScalarKindV1, bits: u16) -> TargetScalarTypeV1 {
    TargetScalarTypeV1::new(kind, bits)
}

const fn f32_type() -> TargetScalarTypeV1 {
    TargetScalarTypeV1::new(TargetScalarKindV1::Float, 32)
}

const fn fp4_type() -> TargetScalarTypeV1 {
    TargetScalarTypeV1::with_encoding(
        TargetScalarKindV1::Float,
        4,
        TargetScalarEncodingV1::Float4E2M1Ocp,
    )
}

fn atomic(
    value_type: TargetScalarTypeV1,
    operation: TargetAtomicOperationV1,
    ordering: TargetMemoryOrderingV1,
    scope: TargetMemoryScopeV1,
    address_space: TargetAddressSpaceV1,
) -> TargetCapabilityRequirementV1 {
    TargetCapabilityRequirementV1::Atomic(TargetAtomicRequirementV1::new(
        value_type,
        operation,
        ordering,
        scope,
        address_space,
    ))
}

fn barrier(participation: TargetBarrierParticipationV1) -> TargetCapabilityRequirementV1 {
    TargetCapabilityRequirementV1::Barrier(TargetBarrierRequirementV1::new(
        TargetExecutionScopeV1::Workgroup,
        TargetMemoryScopeV1::Workgroup,
        TargetAddressSpaceV1::Workgroup,
        TargetMemoryOrderingV1::AcquireRelease,
        participation,
    ))
}

fn collective(
    operation: TargetCollectiveOperationV1,
    value_type: TargetScalarTypeV1,
    participants: u32,
) -> TargetCapabilityRequirementV1 {
    TargetCapabilityRequirementV1::Collective(TargetCollectiveRequirementV1::new(
        TargetExecutionScopeV1::Subgroup,
        operation,
        value_type,
        participants,
        TargetCollectiveParticipationV1::Full,
        (value_type.kind() == TargetScalarKindV1::Float)
            .then_some(TargetNumericalModeV1::IeeeStrict),
    ))
}

const fn cooperative_layouts() -> TargetMatrixLayoutsV1 {
    TargetMatrixLayoutsV1::new(
        TargetMatrixLayoutV1::CooperativeFragment,
        TargetMatrixLayoutV1::CooperativeFragment,
        TargetMatrixLayoutV1::CooperativeFragment,
    )
}

fn bf16_matrix() -> TargetCapabilityRequirementV1 {
    TargetCapabilityRequirementV1::Matrix(TargetMatrixRequirementV1::new(
        TargetMatrixShapeV1::new(16, 16, 16),
        TargetScalarTypeV1::bfloat16(),
        f32_type(),
        cooperative_layouts(),
        TargetNumericalModeV1::AllowContraction,
    ))
}

fn scaled_matrix() -> TargetCapabilityRequirementV1 {
    TargetCapabilityRequirementV1::Matrix(TargetMatrixRequirementV1::with_complete_contract(
        TargetMatrixOperationV1::ScaledMatrixMultiplyAccumulate,
        TargetMatrixShapeV1::new(16, 16, 128),
        fp4_type(),
        fp4_type(),
        f32_type(),
        cooperative_layouts(),
        TargetNumericalModeV1::AllowApproximation,
        64,
        64,
    ))
}

#[test]
fn exact_models_are_stable_and_profile_bound() {
    assert_ne!(
        model(PROFILES[0]).model_identity(),
        model(PROFILES[1]).model_identity()
    );
    for profile in PROFILES {
        let model = model(profile);
        assert_eq!(model.model_identity().validate(), Ok(()));
        assert_eq!(
            model.model_identity(),
            TargetCapabilityModelIdentityV1::new(
                profile.target_profile_spec(),
                PRODUCTION_AMD_NEUTRAL_CAPABILITY_MODEL_REVISION_V1,
            )
            .unwrap()
        );
    }
}

#[test]
fn scalar_address_and_subgroup_matrix_is_exact() {
    for profile in PROFILES {
        let model = model(profile);
        for scalar in [
            TargetScalarTypeV1::new(TargetScalarKindV1::Boolean, 1),
            integer(TargetScalarKindV1::SignedInteger, 8),
            integer(TargetScalarKindV1::UnsignedInteger, 16),
            integer(TargetScalarKindV1::SignedInteger, 32),
            integer(TargetScalarKindV1::UnsignedInteger, 64),
            TargetScalarTypeV1::new(TargetScalarKindV1::Float, 16),
            TargetScalarTypeV1::bfloat16(),
            f32_type(),
        ] {
            assert_eq!(
                outcome(&model, TargetCapabilityRequirementV1::ScalarType(scalar)),
                Supported
            );
        }
        assert_eq!(
            outcome(
                &model,
                TargetCapabilityRequirementV1::ScalarType(TargetScalarTypeV1::new(
                    TargetScalarKindV1::Float,
                    64,
                )),
            ),
            Unsupported
        );
        assert_eq!(
            outcome(
                &model,
                TargetCapabilityRequirementV1::ScalarType(fp4_type())
            ),
            Incomplete
        );
        for space in [
            TargetAddressSpaceV1::Global,
            TargetAddressSpaceV1::Workgroup,
            TargetAddressSpaceV1::Private,
        ] {
            assert_eq!(
                outcome(
                    &model,
                    TargetCapabilityRequirementV1::AddressSpace(
                        space,
                        TargetMemoryAccessV1::ReadWrite
                    ),
                ),
                Supported
            );
        }
        for space in [
            TargetAddressSpaceV1::Constant,
            TargetAddressSpaceV1::Generic,
        ] {
            assert_eq!(
                outcome(
                    &model,
                    TargetCapabilityRequirementV1::AddressSpace(space, TargetMemoryAccessV1::Read),
                ),
                Unsupported
            );
        }
        assert_eq!(
            outcome(&model, TargetCapabilityRequirementV1::SubgroupSize(64)),
            Supported
        );
        assert_eq!(
            outcome(&model, TargetCapabilityRequirementV1::SubgroupSize(32)),
            Unsupported
        );
    }
}

#[test]
fn atomic_matrix_covers_operations_types_scopes_and_failure_order() {
    let operations = [
        TargetAtomicOperationV1::Load,
        TargetAtomicOperationV1::Store,
        TargetAtomicOperationV1::Exchange,
        TargetAtomicOperationV1::Add,
        TargetAtomicOperationV1::Sub,
        TargetAtomicOperationV1::Min,
        TargetAtomicOperationV1::Max,
        TargetAtomicOperationV1::And,
        TargetAtomicOperationV1::Or,
        TargetAtomicOperationV1::Xor,
    ];
    for profile in PROFILES {
        let model = model(profile);
        for bits in [32, 64] {
            for operation in operations {
                let ordering = match operation {
                    TargetAtomicOperationV1::Load => TargetMemoryOrderingV1::Acquire,
                    TargetAtomicOperationV1::Store => TargetMemoryOrderingV1::Release,
                    _ => TargetMemoryOrderingV1::AcquireRelease,
                };
                assert_eq!(
                    outcome(
                        &model,
                        atomic(
                            integer(TargetScalarKindV1::UnsignedInteger, bits),
                            operation,
                            ordering,
                            TargetMemoryScopeV1::Device,
                            TargetAddressSpaceV1::Global,
                        ),
                    ),
                    Supported
                );
            }
        }
        let compare_exchange =
            TargetCapabilityRequirementV1::Atomic(TargetAtomicRequirementV1::compare_exchange(
                integer(TargetScalarKindV1::UnsignedInteger, 32),
                TargetMemoryOrderingV1::AcquireRelease,
                TargetMemoryOrderingV1::Acquire,
                TargetMemoryScopeV1::Device,
                TargetAddressSpaceV1::Global,
            ));
        assert_eq!(outcome(&model, compare_exchange), Supported);
        for substitution in [
            atomic(
                integer(TargetScalarKindV1::UnsignedInteger, 16),
                TargetAtomicOperationV1::Add,
                TargetMemoryOrderingV1::AcquireRelease,
                TargetMemoryScopeV1::Device,
                TargetAddressSpaceV1::Global,
            ),
            atomic(
                integer(TargetScalarKindV1::UnsignedInteger, 32),
                TargetAtomicOperationV1::Nand,
                TargetMemoryOrderingV1::AcquireRelease,
                TargetMemoryScopeV1::Device,
                TargetAddressSpaceV1::Global,
            ),
            atomic(
                integer(TargetScalarKindV1::UnsignedInteger, 32),
                TargetAtomicOperationV1::Add,
                TargetMemoryOrderingV1::AcquireRelease,
                TargetMemoryScopeV1::Device,
                TargetAddressSpaceV1::Private,
            ),
        ] {
            assert_eq!(outcome(&model, substitution), Unsupported);
        }
        let system = atomic(
            integer(TargetScalarKindV1::UnsignedInteger, 32),
            TargetAtomicOperationV1::Add,
            TargetMemoryOrderingV1::SequentiallyConsistent,
            TargetMemoryScopeV1::System,
            TargetAddressSpaceV1::Global,
        );
        assert_eq!(
            outcome(&model, system),
            DynamicLaunchEvidenceRequired(
                TargetLaunchEvidenceKindV1::SystemAtomicMemoryEligibility
            )
        );
        assert_eq!(
            model.capability_owner(system),
            Some(ProductionAmdCapabilityOwnerV1::AtomicLowering)
        );
    }
}

#[test]
fn synchronization_collective_and_matrix_contracts_are_exact() {
    for profile in PROFILES {
        let model = model(profile);
        assert_eq!(
            outcome(&model, barrier(TargetBarrierParticipationV1::Uniform)),
            Supported
        );
        assert_eq!(
            outcome(&model, barrier(TargetBarrierParticipationV1::DynamicMask)),
            Unsupported
        );
        assert_eq!(
            outcome(
                &model,
                TargetCapabilityRequirementV1::Fence(TargetFenceRequirementV1::new(
                    TargetMemoryScopeV1::Device,
                    TargetAddressSpaceV1::Global,
                    TargetMemoryOrderingV1::AcquireRelease,
                )),
            ),
            Supported
        );
        for operation in [
            TargetCollectiveOperationV1::Ballot,
            TargetCollectiveOperationV1::Any,
            TargetCollectiveOperationV1::All,
        ] {
            assert_eq!(
                outcome(
                    &model,
                    collective(
                        operation,
                        TargetScalarTypeV1::new(TargetScalarKindV1::Boolean, 1),
                        64,
                    ),
                ),
                Supported
            );
        }
        assert_eq!(
            outcome(
                &model,
                collective(
                    TargetCollectiveOperationV1::Broadcast,
                    integer(TargetScalarKindV1::UnsignedInteger, 32),
                    64,
                ),
            ),
            Supported
        );
        assert_eq!(
            outcome(
                &model,
                collective(TargetCollectiveOperationV1::ReduceAdd, f32_type(), 64),
            ),
            Supported
        );
        let scan = TargetCapabilityRequirementV1::Collective(TargetCollectiveRequirementV1::new(
            TargetExecutionScopeV1::Workgroup,
            TargetCollectiveOperationV1::InclusiveScanAdd,
            integer(TargetScalarKindV1::UnsignedInteger, 32),
            64,
            TargetCollectiveParticipationV1::Full,
            None,
        ));
        assert_eq!(outcome(&model, scan), Supported);
        assert_eq!(outcome(&model, bf16_matrix()), Supported);
    }
    assert_eq!(
        outcome(
            &model(ProductionAmdTargetProfileV1::Gfx950),
            scaled_matrix()
        ),
        Supported
    );
    assert_eq!(
        outcome(
            &model(ProductionAmdTargetProfileV1::Gfx942),
            scaled_matrix()
        ),
        Unsupported
    );

    let wrong_layout = TargetCapabilityRequirementV1::Matrix(TargetMatrixRequirementV1::new(
        TargetMatrixShapeV1::new(16, 16, 16),
        TargetScalarTypeV1::bfloat16(),
        f32_type(),
        TargetMatrixLayoutsV1::new(
            TargetMatrixLayoutV1::RowMajor,
            TargetMatrixLayoutV1::CooperativeFragment,
            TargetMatrixLayoutV1::CooperativeFragment,
        ),
        TargetNumericalModeV1::AllowContraction,
    ));
    for profile in PROFILES {
        assert_eq!(outcome(&model(profile), wrong_layout), Unsupported);
    }
}

#[test]
fn nonfinal_and_physical_contracts_remain_explicit() {
    for profile in PROFILES {
        let model = model(profile);
        let async_copy =
            TargetCapabilityRequirementV1::AsyncCopy(TargetAsyncCopyRequirementV1::new(
                TargetAddressSpaceV1::Global,
                TargetAddressSpaceV1::Workgroup,
                16,
                16,
            ));
        assert_eq!(outcome(&model, async_copy), Supported);
        assert_eq!(
            model.capability_owner(async_copy),
            Some(ProductionAmdCapabilityOwnerV1::AsyncCopyLowering)
        );
        for scalar in [
            TargetScalarTypeV1::new(TargetScalarKindV1::Float, 16),
            TargetScalarTypeV1::bfloat16(),
        ] {
            assert_eq!(
                outcome(
                    &model,
                    TargetCapabilityRequirementV1::Numerical(TargetNumericalRequirementV1::new(
                        scalar,
                        TargetNumericalModeV1::IeeeStrict,
                    )),
                ),
                Incomplete
            );
        }
        let private = TargetCapabilityRequirementV1::Resource(
            TargetResourceRequirementV1::PrivateMemoryBytesPerInvocationAtMost(1),
        );
        assert_eq!(outcome(&model, private), Supported);
        assert_eq!(
            model.capability_owner(private),
            Some(ProductionAmdCapabilityOwnerV1::ResourceAdmission)
        );
        assert_eq!(
            outcome(
                &model,
                TargetCapabilityRequirementV1::Resource(
                    TargetResourceRequirementV1::RegistersPerInvocationAtMost(1),
                ),
            ),
            Incomplete
        );
        let lds_limit = if profile == ProductionAmdTargetProfileV1::Gfx942 {
            64 * 1024
        } else {
            160 * 1024
        };
        for requirement in [
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::WorkgroupInvocationsAtMost(1024),
            ),
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::StaticSharedMemoryBytesAtMost(lds_limit),
            ),
            TargetCapabilityRequirementV1::Abi(TargetAbiConstraintV1::PointerWidth(64)),
            TargetCapabilityRequirementV1::Abi(TargetAbiConstraintV1::Endianness(
                TargetEndiannessV1::Little,
            )),
            TargetCapabilityRequirementV1::Abi(
                TargetAbiConstraintV1::KernelArgumentAlignmentAtMost(8),
            ),
            TargetCapabilityRequirementV1::Abi(
                TargetAbiConstraintV1::KernelArgumentSegmentBytesAtMost(1 << 20),
            ),
            TargetCapabilityRequirementV1::Object(TargetObjectConstraintV1::Format(
                TargetObjectFormatV1::LoadableExecutable,
            )),
            TargetCapabilityRequirementV1::Object(TargetObjectConstraintV1::Relocatable(false)),
        ] {
            assert_eq!(outcome(&model, requirement), Supported, "{requirement}");
            assert!(model.capability_owner(requirement).is_some());
        }
        for requirement in [
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::WorkgroupInvocationsAtMost(1025),
            ),
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::StaticSharedMemoryBytesAtMost(lds_limit + 1),
            ),
            TargetCapabilityRequirementV1::Abi(
                TargetAbiConstraintV1::KernelArgumentAlignmentAtMost(16),
            ),
            TargetCapabilityRequirementV1::Object(TargetObjectConstraintV1::Relocatable(true)),
        ] {
            assert_eq!(outcome(&model, requirement), Unsupported, "{requirement}");
        }
    }
}

#[test]
fn neutral_source_and_canonical_records_do_not_leak_backend_terms() {
    let source = include_str!("../../fe2o3-target-spec/src/capability_v1.rs").to_ascii_lowercase();
    for forbidden in ["amd", "gfx", "hip", "hsa", "cuda", "spir-v", "spirv"] {
        assert!(!source.contains(forbidden), "{forbidden}");
    }
    let requirement = bf16_matrix();
    let mut first = String::new();
    let mut second = String::new();
    requirement.encode_canonical(&mut first).unwrap();
    requirement.encode_canonical(&mut second).unwrap();
    assert_eq!(first, second);
    assert!(!first.contains("gfx"));
}

#[test]
fn neutral_conformance_graph_executes_the_same_checked_closure_query() {
    let source = TargetCapabilityRequirementV1::AddressSpace(
        TargetAddressSpaceV1::Global,
        TargetMemoryAccessV1::Read,
    );
    let destination = TargetCapabilityRequirementV1::AddressSpace(
        TargetAddressSpaceV1::Workgroup,
        TargetMemoryAccessV1::Write,
    );
    let copy = TargetCapabilityRequirementV1::AsyncCopy(TargetAsyncCopyRequirementV1::new(
        TargetAddressSpaceV1::Global,
        TargetAddressSpaceV1::Workgroup,
        16,
        16,
    ));
    let roots = [copy];
    let mut dependencies = [source, destination];
    dependencies.sort();
    let empty = [];
    let mut nodes = [
        TargetCapabilityClosureNodeV1::new(source, &empty),
        TargetCapabilityClosureNodeV1::new(destination, &empty),
        TargetCapabilityClosureNodeV1::new(copy, &dependencies),
    ];
    nodes.sort_by_key(|node| node.requirement());
    let specification = TargetCapabilityClosureSpecV1::new(&roots, &nodes);

    for profile in PROFILES {
        let target = model(profile);
        let closure = query_target_capability_closure_v1(&target, specification).unwrap();
        assert_eq!(closure.len(), 3);
        for index in 0..closure.len() {
            let decision = closure.decision(index).unwrap();
            assert_eq!(decision.model(), target.model_identity());
            assert_eq!(decision.outcome(), Supported);
        }
    }
}
