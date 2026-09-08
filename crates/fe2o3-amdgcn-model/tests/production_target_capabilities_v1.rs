use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_amdgcn_model::{
    MAX_PRODUCTION_TARGET_CAPABILITY_CLOSURE_BYTES_V1, OmittedCapabilityAxisV1,
    ProductionArtifactOnlyRequirementV1, ProductionCanonicalGraphVersionV1,
    ProductionTargetCapabilityCanonicalErrorV1, ProductionTargetCapabilityClosureV1,
    ProductionTargetCapabilityClosureV13, ProductionTargetCapabilityDiagnosticCodeV1,
    ProductionTargetCapabilityErrorV1, ProductionTargetLaunchEvidenceV1,
    ProductionTargetLaunchEvidenceV13, ProductionTargetLaunchEvidenceValueV1,
    bind_capability_legalized_production_target_v1, legalize_production_target_capabilities_v1,
    legalize_production_target_capabilities_v13, production_target_requirements_for_module_v1,
};
use fe2o3_kernel_ir::{
    AddressSpace, AtomicKind, BasicBlock, BlockId, ExecutionCapabilityOpV1,
    ExecutionCapabilityOperationV1, ExecutionCapabilityProvenanceV1,
    ExecutionCapabilityRequirementV1, ExecutionCapabilitySignatureV1, ExecutionCapabilitySourceV1,
    ExecutionElementLayoutV1, ExecutionSafetyObligationsV1, ExecutionTypeIdentityV1, Function,
    FunctionId, LaunchDomain, LaunchExtent, MemoryOrdering, Module, Operation, OperationKind,
    ResourceCapabilityRequirementV1, ScalarType, Signature, SynchronizationScope, TargetCapability,
    Terminator, VerifiedCanonicalKernelIrV12, VerifiedCanonicalKernelIrV13, WorkgroupSize,
};
use fe2o3_target_spec::{
    TargetAbiConstraintV1, TargetCapabilityDecisionOutcomeV1, TargetCapabilityRequirementV1,
    TargetLaunchEvidenceKindV1, TargetResourceRequirementV1,
};

const PROFILES: [ProductionAmdTargetProfileV1; 2] = [
    ProductionAmdTargetProfileV1::Gfx942,
    ProductionAmdTargetProfileV1::Gfx950,
];

fn neutral_module(id: &str) -> Module {
    let mut block = BasicBlock::new(BlockId(0));
    block.terminator = Some(Terminator::Return { values: vec![] });
    let function =
        Function::kernel_entry("entry", Signature::new(vec![], vec![]), vec![], vec![block]);
    let mut kernel = fe2o3_kernel_ir::Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    let mut module = Module::new(id);
    module.functions.push(function);
    module.kernels.push(kernel);
    module
}

fn system_atomic_module(id: &str) -> Module {
    let mut module = neutral_module(id);
    module
        .required_capabilities
        .insert(TargetCapability::Execution(
            ExecutionCapabilityRequirementV1::Atomic {
                value_type: ScalarType::U32,
                operation: AtomicKind::Add,
                ordering: MemoryOrdering::SequentiallyConsistent,
                failure_ordering: None,
                scope: SynchronizationScope::System,
                address_space: AddressSpace::Global,
            },
        ));
    module
}

fn dynamic_lds_module(id: &str, bytes: u64) -> Module {
    let mut module = neutral_module(id);
    module
        .required_capabilities
        .insert(TargetCapability::Execution(
            ExecutionCapabilityRequirementV1::Resource(
                ResourceCapabilityRequirementV1::DynamicWorkgroupMemoryBytesAtMost(bytes),
            ),
        ));
    module
}

fn v12(module: Module) -> VerifiedCanonicalKernelIrV12 {
    VerifiedCanonicalKernelIrV12::from_module(module).unwrap()
}

fn v13(module: Module) -> VerifiedCanonicalKernelIrV13 {
    VerifiedCanonicalKernelIrV13::from_module(module).unwrap()
}

fn execution_operation(operation: ExecutionCapabilityOperationV1) -> Operation {
    let references = operation.type_references();
    let output = references
        .last()
        .copied()
        .unwrap_or(ExecutionTypeIdentityV1::new([1; 32]));
    Operation::new(
        vec![],
        OperationKind::ExecutionCapability(ExecutionCapabilityOpV1 {
            operands: vec![],
            signature: ExecutionCapabilitySignatureV1::new(&references, output).unwrap(),
            provenance: ExecutionCapabilityProvenanceV1 {
                root: FunctionId::new("entry"),
                kernel_binding: [1; 32],
                frontend_unit: [2; 32],
                kernel_marker: [3; 32],
                target_brand: [4; 32],
                launch_brand: [5; 32],
                issuance: [6; 32],
            },
            workgroup_brand: Some([7; 32]),
            epoch_before: Some([8; 32]),
            epoch_after: None,
            obligations: ExecutionSafetyObligationsV1::from_bits(0),
            source: ExecutionCapabilitySourceV1 {
                function: [9; 32],
                operation: [10; 32],
                block: 0,
            },
            operation,
        }),
    )
}

#[test]
fn module_extraction_contains_only_graph_requirements() {
    let requirements = production_target_requirements_for_module_v1(&neutral_module("extract"))
        .expect("neutral module requirements");
    assert_eq!(
        requirements,
        [
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::WorkgroupInvocationsAtMost(64),
            ),
            TargetCapabilityRequirementV1::Resource(
                TargetResourceRequirementV1::WorkgroupDimensions { x: 64, y: 1, z: 1 },
            ),
        ]
        .into_iter()
        .collect()
    );
    assert!(!requirements.iter().any(|requirement| matches!(
        requirement,
        TargetCapabilityRequirementV1::SubgroupSize(_)
            | TargetCapabilityRequirementV1::Abi(_)
            | TargetCapabilityRequirementV1::Object(_)
    )));
}

#[test]
fn static_resource_requirements_are_aggregated_over_real_allocations() {
    let mut module = neutral_module("aggregate-static-resources");
    let identity = |byte| ExecutionTypeIdentityV1::new([byte; 32]);
    let block = &mut module.functions[0].body.as_mut().unwrap().blocks[0];
    block.operations.extend([
        execution_operation(ExecutionCapabilityOperationV1::LdsAllocate {
            workgroup: identity(1),
            lds: identity(2),
            element: identity(3),
            layout: ExecutionElementLayoutV1 {
                byte_size: 4,
                byte_alignment: 4,
            },
            elements: 3,
        }),
        execution_operation(ExecutionCapabilityOperationV1::WorkgroupMemoryAllocate {
            workgroup: identity(4),
            view: identity(5),
            element: identity(6),
            layout: ExecutionElementLayoutV1 {
                byte_size: 8,
                byte_alignment: 8,
            },
            elements: 5,
            index_space: identity(7),
        }),
        execution_operation(ExecutionCapabilityOperationV1::PrivateMemoryAllocate {
            context: identity(8),
            view: identity(9),
            element: identity(10),
            layout: ExecutionElementLayoutV1 {
                byte_size: 2,
                byte_alignment: 2,
            },
            elements: 7,
        }),
    ]);
    let requirements = production_target_requirements_for_module_v1(&module).unwrap();
    let resources = requirements
        .iter()
        .filter_map(|requirement| match requirement {
            TargetCapabilityRequirementV1::Resource(resource) => Some(*resource),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(
        resources.contains(&TargetResourceRequirementV1::StaticSharedMemoryBytesAtMost(
            52
        ))
    );
    assert!(
        resources.contains(&TargetResourceRequirementV1::PrivateMemoryBytesPerInvocationAtMost(14))
    );
    assert!(
        resources.contains(&TargetResourceRequirementV1::StaticSharedMemoryBytesAtMost(
            12
        ))
    );
    assert!(
        resources.contains(&TargetResourceRequirementV1::StaticSharedMemoryBytesAtMost(
            40
        ))
    );
}

#[test]
fn overflowing_execution_footprint_is_rejected_instead_of_omitted() {
    let mut module = neutral_module("overflowing-static-resource");
    let identity = |byte| ExecutionTypeIdentityV1::new([byte; 32]);
    module.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .push(execution_operation(
            ExecutionCapabilityOperationV1::PrivateMemoryAllocate {
                context: identity(1),
                view: identity(2),
                element: identity(3),
                layout: ExecutionElementLayoutV1 {
                    byte_size: u32::MAX,
                    byte_alignment: 1,
                },
                elements: u64::MAX,
            },
        ));
    assert!(matches!(
        production_target_requirements_for_module_v1(&module),
        Err(ProductionTargetCapabilityErrorV1::UnknownStaticResourceFootprint { .. })
    ));
}

#[test]
fn v12_and_v13_checked_adapters_share_decision_rules() {
    for profile in PROFILES {
        let owner12 = v12(neutral_module("adapter-parity"));
        let evidence12 =
            ProductionTargetLaunchEvidenceV1::for_static_launches(&owner12, 7).unwrap();
        let closure12 =
            legalize_production_target_capabilities_v1(&owner12, 7, &evidence12, profile).unwrap();

        let owner13 = v13(neutral_module("adapter-parity"));
        let evidence13 =
            ProductionTargetLaunchEvidenceV13::for_static_launches(&owner13, 7).unwrap();
        let closure13 =
            legalize_production_target_capabilities_v13(&owner13, 7, &evidence13, profile).unwrap();

        let decisions12 = closure12
            .decisions()
            .iter()
            .map(|decision| (decision.requirement(), decision.outcome()))
            .collect::<Vec<_>>();
        let decisions13 = closure13
            .decisions()
            .iter()
            .map(|decision| (decision.requirement(), decision.outcome()))
            .collect::<Vec<_>>();
        assert_eq!(decisions12, decisions13);
        assert_eq!(closure12.capability_owners(), closure13.capability_owners());
        assert_eq!(
            closure13.subject().version(),
            ProductionCanonicalGraphVersionV1::V13
        );
        assert_ne!(closure12.identity(), closure13.identity());
        assert!(!closure12.decisions().iter().any(|decision| matches!(
            decision.requirement(),
            TargetCapabilityRequirementV1::SubgroupSize(_)
        )));
        let required = TargetCapabilityRequirementV1::Abi(TargetAbiConstraintV1::PointerWidth(64));
        assert!(closure12.decisions().iter().any(|decision| {
            decision.requirement() == required
                && decision.outcome() == TargetCapabilityDecisionOutcomeV1::Supported
        }));
        assert!(!closure12.decisions().iter().any(|decision| matches!(
            decision.requirement(),
            TargetCapabilityRequirementV1::Object(_)
                | TargetCapabilityRequirementV1::Abi(
                    TargetAbiConstraintV1::KernelArgumentAlignmentAtMost(_)
                        | TargetAbiConstraintV1::KernelArgumentSegmentBytesAtMost(_)
                )
                | TargetCapabilityRequirementV1::Resource(
                    TargetResourceRequirementV1::RegistersPerInvocationAtMost(_)
                )
        )));
        assert_eq!(
            closure12.artifact_only_requirements(),
            &[
                ProductionArtifactOnlyRequirementV1::ExactKernelArgumentLayout,
                ProductionArtifactOnlyRequirementV1::ExactRegistersPerInvocation,
                ProductionArtifactOnlyRequirementV1::ExactPrivateSegmentBytes,
                ProductionArtifactOnlyRequirementV1::ExactScratchBytes,
                ProductionArtifactOnlyRequirementV1::ExactLoadableObject,
            ]
        );
        let decoded12 = ProductionTargetCapabilityClosureV1::decode_canonical(
            closure12.canonical_bytes(),
            &evidence12,
        )
        .unwrap();
        assert_eq!(decoded12, closure12);
        let decoded13 = ProductionTargetCapabilityClosureV13::decode_canonical(
            closure13.canonical_bytes(),
            &evidence13,
        )
        .unwrap();
        assert_eq!(decoded13, closure13);
    }
}

#[test]
fn v12_binding_retains_compatibility_custody() {
    for profile in PROFILES {
        let owner = v12(neutral_module("binding"));
        let identity = *owner.identity();
        let evidence = ProductionTargetLaunchEvidenceV1::for_static_launches(&owner, 11).unwrap();
        let bound =
            bind_capability_legalized_production_target_v1(owner, 11, &evidence, profile).unwrap();
        assert_eq!(bound.capability_closure().neutral_kernel_ir(), identity);
        assert_eq!(bound.input_epoch(), 11);
        assert_eq!(bound.output_epoch(), 12);
        assert_eq!(
            bound.capability_closure().decisions().len(),
            bound.capability_closure().capability_owners().len()
        );
        assert!(!bound.capability_closure().grants_launch_authority());
    }
}

#[test]
fn system_atomic_evidence_is_exact_identity_bound_and_required() {
    for profile in PROFILES {
        let owner = v12(system_atomic_module("system-atomic"));
        let missing = ProductionTargetLaunchEvidenceV1::for_static_launches(&owner, 5).unwrap();
        assert!(matches!(
            legalize_production_target_capabilities_v1(&owner, 5, &missing, profile),
            Err(ProductionTargetCapabilityErrorV1::MissingLaunchEvidence {
                kind: TargetLaunchEvidenceKindV1::SystemAtomicMemoryEligibility,
                ..
            })
        ));

        let false_evidence = ProductionTargetLaunchEvidenceV1::from_exact_values(
            &owner,
            5,
            [ProductionTargetLaunchEvidenceValueV1::SystemAtomicMemoryEligibility(false)],
        )
        .unwrap();
        assert!(matches!(
            legalize_production_target_capabilities_v1(&owner, 5, &false_evidence, profile),
            Err(
                ProductionTargetCapabilityErrorV1::LaunchEvidenceValueMismatch {
                    observed: ProductionTargetLaunchEvidenceValueV1::SystemAtomicMemoryEligibility(
                        false
                    ),
                    ..
                }
            )
        ));

        let evidence = ProductionTargetLaunchEvidenceV1::with_system_atomic_memory_eligibility(
            &owner, 5, true,
        )
        .unwrap();
        let closure =
            legalize_production_target_capabilities_v1(&owner, 5, &evidence, profile).unwrap();
        assert!(closure.decisions().iter().any(|decision| matches!(
            (decision.requirement(), decision.outcome()),
            (
                TargetCapabilityRequirementV1::Atomic(atomic),
                TargetCapabilityDecisionOutcomeV1::DynamicLaunchEvidenceRequired(
                    TargetLaunchEvidenceKindV1::SystemAtomicMemoryEligibility
                )
            ) if atomic.scope() == fe2o3_target_spec::TargetMemoryScopeV1::System
        )));
    }
}

#[test]
fn launch_evidence_rejects_unused_duplicate_and_subject_substitution() {
    let owner = v12(neutral_module("unused"));
    let unused =
        ProductionTargetLaunchEvidenceV1::with_system_atomic_memory_eligibility(&owner, 3, true)
            .unwrap();
    assert!(matches!(
        legalize_production_target_capabilities_v1(
            &owner,
            3,
            &unused,
            ProductionAmdTargetProfileV1::Gfx942,
        ),
        Err(ProductionTargetCapabilityErrorV1::UnusedLaunchEvidence {
            kind: TargetLaunchEvidenceKindV1::SystemAtomicMemoryEligibility,
        })
    ));

    let system = v12(system_atomic_module("duplicate"));
    assert!(matches!(
        ProductionTargetLaunchEvidenceV1::from_exact_values(
            &system,
            3,
            [
                ProductionTargetLaunchEvidenceValueV1::SystemAtomicMemoryEligibility(true),
                ProductionTargetLaunchEvidenceValueV1::SystemAtomicMemoryEligibility(false),
            ],
        ),
        Err(ProductionTargetCapabilityErrorV1::DuplicateLaunchEvidence {
            kind: TargetLaunchEvidenceKindV1::SystemAtomicMemoryEligibility,
        })
    ));

    let first = v12(system_atomic_module("first-subject"));
    let second = v12(system_atomic_module("second-subject"));
    let substituted =
        ProductionTargetLaunchEvidenceV1::with_system_atomic_memory_eligibility(&first, 3, true)
            .unwrap();
    assert!(matches!(
        legalize_production_target_capabilities_v1(
            &second,
            3,
            &substituted,
            ProductionAmdTargetProfileV1::Gfx942,
        ),
        Err(ProductionTargetCapabilityErrorV1::LaunchEvidenceSubjectMismatch)
    ));
}

#[test]
fn dynamic_lds_evidence_still_uses_exact_value_matching() {
    let owner = v12(dynamic_lds_module("dynamic-lds", 512));
    for profile in PROFILES {
        let missing = ProductionTargetLaunchEvidenceV1::for_static_launches(&owner, 9).unwrap();
        assert!(matches!(
            legalize_production_target_capabilities_v1(&owner, 9, &missing, profile),
            Err(ProductionTargetCapabilityErrorV1::MissingLaunchEvidence {
                kind: TargetLaunchEvidenceKindV1::DynamicSharedMemoryBytes,
                ..
            })
        ));
        let mismatch =
            ProductionTargetLaunchEvidenceV1::with_dynamic_shared_memory_bytes(&owner, 9, 256)
                .unwrap();
        assert!(matches!(
            legalize_production_target_capabilities_v1(&owner, 9, &mismatch, profile),
            Err(ProductionTargetCapabilityErrorV1::LaunchEvidenceValueMismatch { .. })
        ));
        let exact =
            ProductionTargetLaunchEvidenceV1::with_dynamic_shared_memory_bytes(&owner, 9, 512)
                .unwrap();
        legalize_production_target_capabilities_v1(&owner, 9, &exact, profile).unwrap();
    }
}

#[test]
fn v12_coarse_participation_axes_remain_fail_closed() {
    let mut module = neutral_module("coarse-collective");
    module
        .required_capabilities
        .insert(TargetCapability::Execution(
            ExecutionCapabilityRequirementV1::Collective {
                execution_scope: SynchronizationScope::Subgroup,
                operation: fe2o3_kernel_ir::CollectiveCapabilityOperationV1::ReduceAdd,
                value_type: ScalarType::F32,
                participants: 64,
            },
        ));
    let owner = v12(module);
    let evidence = ProductionTargetLaunchEvidenceV1::for_static_launches(&owner, 1).unwrap();
    let error = legalize_production_target_capabilities_v1(
        &owner,
        1,
        &evidence,
        ProductionAmdTargetProfileV1::Gfx950,
    )
    .unwrap_err();
    assert!(matches!(
        &error,
        ProductionTargetCapabilityErrorV1::OmittedAxis {
            axis: OmittedCapabilityAxisV1::CollectiveParticipation,
            ..
        }
    ));
    assert_eq!(
        error.diagnostic_code(),
        ProductionTargetCapabilityDiagnosticCodeV1::OmittedAxis
    );
}

#[test]
fn malformed_launch_shape_is_rejected_before_target_query() {
    let mut module = neutral_module("missing-workgroup");
    module.kernels[0].workgroup_size = None;
    let owner = v12(module);
    let evidence = ProductionTargetLaunchEvidenceV1::for_static_launches(&owner, 0).unwrap();
    assert!(matches!(
        legalize_production_target_capabilities_v1(
            &owner,
            0,
            &evidence,
            ProductionAmdTargetProfileV1::Gfx942,
        ),
        Err(ProductionTargetCapabilityErrorV1::MissingWorkgroupSize { .. })
    ));
}

#[test]
fn canonical_closure_codec_rejects_hostile_envelopes_and_cross_version_replay() {
    let owner = v13(neutral_module("canonical-codec"));
    let evidence = ProductionTargetLaunchEvidenceV13::for_static_launches(&owner, 17).unwrap();
    let closure = legalize_production_target_capabilities_v13(
        &owner,
        17,
        &evidence,
        ProductionAmdTargetProfileV1::Gfx950,
    )
    .unwrap();
    let canonical = closure.canonical_bytes().to_vec();

    let mut trailing = canonical.clone();
    trailing.push(0);
    assert!(matches!(
        ProductionTargetCapabilityClosureV13::decode_canonical(&trailing, &evidence),
        Err(ProductionTargetCapabilityCanonicalErrorV1::NonCanonical)
    ));

    let mut unknown_magic = canonical.clone();
    unknown_magic[0] ^= 0xff;
    assert!(matches!(
        ProductionTargetCapabilityClosureV13::decode_canonical(&unknown_magic, &evidence),
        Err(ProductionTargetCapabilityCanonicalErrorV1::InvalidMagic)
    ));

    let mut unknown_version = canonical.clone();
    unknown_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert!(matches!(
        ProductionTargetCapabilityClosureV13::decode_canonical(&unknown_version, &evidence),
        Err(ProductionTargetCapabilityCanonicalErrorV1::UnsupportedWireVersion(2))
    ));

    let v12_owner = v12(neutral_module("canonical-codec-v12"));
    let v12_evidence =
        ProductionTargetLaunchEvidenceV1::for_static_launches(&v12_owner, 17).unwrap();
    assert!(matches!(
        ProductionTargetCapabilityClosureV1::decode_canonical(&canonical, &v12_evidence),
        Err(ProductionTargetCapabilityCanonicalErrorV1::GraphVersionMismatch)
            | Err(ProductionTargetCapabilityCanonicalErrorV1::LaunchEvidenceMismatch)
    ));

    let oversized = vec![0; MAX_PRODUCTION_TARGET_CAPABILITY_CLOSURE_BYTES_V1 + 1];
    assert!(matches!(
        ProductionTargetCapabilityClosureV13::decode_canonical(&oversized, &evidence),
        Err(ProductionTargetCapabilityCanonicalErrorV1::Oversized { .. })
    ));

    for index in 0..canonical.len() {
        let mut mutated = canonical.clone();
        mutated[index] ^= 1;
        if let Ok(decoded) =
            ProductionTargetCapabilityClosureV13::decode_canonical(&mutated, &evidence)
        {
            assert_eq!(decoded.canonical_bytes(), mutated);
            assert_ne!(decoded.identity(), closure.identity());
        }
    }
}

#[test]
fn legalization_records_bind_exact_dependencies_queries_answers_and_owners() {
    let owner = v12(system_atomic_module("dependency-edges"));
    let evidence =
        ProductionTargetLaunchEvidenceV1::with_system_atomic_memory_eligibility(&owner, 19, true)
            .unwrap();
    let closure = legalize_production_target_capabilities_v1(
        &owner,
        19,
        &evidence,
        ProductionAmdTargetProfileV1::Gfx942,
    )
    .unwrap();
    let atomic = closure
        .legalization_records()
        .iter()
        .find(|record| {
            matches!(
                record.decision().requirement(),
                TargetCapabilityRequirementV1::Atomic(_)
            )
        })
        .unwrap();
    let TargetCapabilityRequirementV1::Atomic(atomic_requirement) = atomic.decision().requirement()
    else {
        unreachable!()
    };
    assert_eq!(
        atomic.dependencies(),
        &[TargetCapabilityRequirementV1::AddressSpace(
            atomic_requirement.address_space(),
            fe2o3_target_spec::TargetMemoryAccessV1::ReadWrite,
        )]
    );
    assert_eq!(atomic.decision().model(), closure.target_model());
    assert_eq!(
        atomic.decision().outcome(),
        TargetCapabilityDecisionOutcomeV1::DynamicLaunchEvidenceRequired(
            TargetLaunchEvidenceKindV1::SystemAtomicMemoryEligibility,
        )
    );
    assert_eq!(
        closure.legalization_records().len(),
        closure.decisions().len()
    );
}
