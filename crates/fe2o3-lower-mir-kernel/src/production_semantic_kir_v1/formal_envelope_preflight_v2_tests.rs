fn guarded_envelope_source_v2() -> ProductionSemanticKirOwnerV1 {
    ProductionSemanticKirOwnerV1::try_lower(
        guarded_read_source_v360(false, false),
        ProductionSemanticKirLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn full_envelope_preflight_retains_fresh_original_report_and_unchanged_v4_policy() {
    use crate::{InertCanonicalFormalMemoryAdmissionEvidenceV4, ProductionFormalMemoryOwnerV1};
    let source = guarded_envelope_source_v2();
    let extents = source.module().kernels[0]
        .domain
        .extents()
        .enumerate()
        .fold([1_u64; 3], |mut extents, (axis, extent)| {
            extents[axis] = match extent {
                LaunchExtent::Static(value) => u64::from(value),
                LaunchExtent::Dynamic => 4096,
            };
            extents
        });
    let original_identity = source.canonical_kernel_ir_identity();
    let fresh = fe2o3_kernel_ir::derive_kernel_memory_obligations_for_launch(
        source.module(),
        &source.module().kernels[0].id,
        fe2o3_kernel_ir::ExplicitLaunchExtent::Exact {
            rank: source.module().kernels[0].domain.rank(),
            extents,
        },
        FormalIndexWidth::Bits64,
    )
    .unwrap();
    let owner =
        ProductionFormalMemoryOwnerV1::try_admit_for_launch_envelopes_v2(source, &[extents])
            .unwrap();
    owner.verify_equivalence().unwrap();
    assert_eq!(
        owner.semantic_kir().canonical_kernel_ir_identity(),
        original_identity
    );
    let [envelope] = owner.launch_envelopes_v2().unwrap() else {
        panic!("exact root roster")
    };
    assert_eq!(envelope.extents(), extents);
    assert_eq!(envelope.kernel().obligations(), fresh.obligations());
    assert_eq!(
        envelope.kernel().ranked_discharged_reasons(),
        fresh.incomplete_reasons()
    );
    let old = ProductionFormalMemoryOwnerV1::try_admit(guarded_envelope_source_v2()).unwrap();
    assert!(old.launch_envelopes_v2().is_none());
    assert_eq!(owner.kernels(), old.kernels());
    let evidence = InertCanonicalFormalMemoryAdmissionEvidenceV4::from_live_owner(&owner).unwrap();
    let old_evidence =
        InertCanonicalFormalMemoryAdmissionEvidenceV4::from_live_owner(&old).unwrap();
    assert_eq!(evidence.canonical_bytes(), old_evidence.canonical_bytes());
    assert!(!owner.grants_artifact_or_launch_authority());
}

#[test]
fn full_envelope_preflight_rejects_omitted_extra_zero_and_inactive_roots() {
    use crate::{ProductionFormalMemoryErrorV1, ProductionFormalMemoryOwnerV1};
    for extents in [vec![], vec![[64, 1, 1]; 2]] {
        assert!(matches!(
            ProductionFormalMemoryOwnerV1::try_admit_for_launch_envelopes_v2(
                guarded_envelope_source_v2(),
                &extents,
            ),
            Err(ProductionFormalMemoryErrorV1::LaunchEnvelopeCount { expected: 1, .. })
        ));
    }
    for extents in [[0, 1, 1], [64, 2, 1], [64, 1, 2]] {
        assert!(
            ProductionFormalMemoryOwnerV1::try_admit_for_launch_envelopes_v2(
                guarded_envelope_source_v2(),
                &[extents],
            )
            .is_err()
        );
    }
}

#[test]
fn full_envelope_preflight_checks_every_original_static_root_without_reordering() {
    use crate::ProductionFormalMemoryOwnerV1;
    let exports = ["zeta_entry", "alpha_entry", "middle_entry"];
    let source = || {
        let receipt = ProductionRankedSemanticProjectionModuleReceiptV1::from_unvalidated_projection_roster_candidate(
            noop_semantic_owner(&exports),
            noop_ranked_roster(&exports),
        ).unwrap();
        ProductionSemanticKirOwnerV1::try_lower_after_ranked_roster_checks(
            receipt,
            ProductionSemanticKirLimitsV1::default(),
        )
        .unwrap()
    };
    let owner = ProductionFormalMemoryOwnerV1::try_admit_for_launch_envelopes_v2(
        source(),
        &[[64, 1, 1]; 3],
    )
    .unwrap();
    assert_eq!(
        owner
            .launch_envelopes_v2()
            .unwrap()
            .iter()
            .map(|row| row.kernel().obligations().kernel().as_str())
            .collect::<Vec<_>>(),
        exports,
    );
    owner.verify_equivalence().unwrap();
    for bad in 0..3 {
        let mut extents = [[64, 1, 1]; 3];
        extents[bad][0] = 63;
        assert!(
            ProductionFormalMemoryOwnerV1::try_admit_for_launch_envelopes_v2(source(), &extents)
                .is_err()
        );
    }
    let source = source();
    let identity = source.canonical_kernel_ir_identity();
    let envelopes = [[128, 1, 1], [1088, 1, 1], [65, 1, 1]];
    let owner =
        ProductionFormalMemoryOwnerV1::try_admit_for_launch_envelopes_v2(source, &envelopes)
            .unwrap();
    owner.verify_equivalence().unwrap();
    assert_eq!(
        owner.semantic_kir().canonical_kernel_ir_identity(),
        identity
    );
    for (kernel, envelope) in owner
        .semantic_kir()
        .module()
        .kernels
        .iter()
        .zip(owner.launch_envelopes_v2().unwrap())
    {
        assert_eq!(
            kernel.domain,
            fe2o3_kernel_ir::LaunchDomain::D1 {
                x: LaunchExtent::Static(64)
            }
        );
        assert_eq!(
            envelope
                .kernel()
                .obligations()
                .invocations()
                .unwrap()
                .end_exclusive(),
            envelope.extents()[0]
        );
    }
}

#[test]
fn full_envelope_census_finds_cross_access_conflict_omitted_by_two_invocations() {
    use fe2o3_kernel_ir::{
        ExplicitLaunchExtent, IntrinsicOperation, Kernel, LaunchDomain, LaunchExtent, Module,
        Signature, derive_kernel_memory_obligations_for_launch,
    };
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let operation = |id, ty, kind| Operation::effect_free(ValueDef::new(ValueId(id), ty), kind);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        operation(
            2,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        operation(3, Type::INDEX, OperationKind::Constant(Constant::Index(3))),
        operation(
            4,
            pointer.clone(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        operation(
            5,
            pointer.clone(),
            OperationKind::GetElementPointer {
                base: ValueId(4),
                offset: ValueId(2),
            },
        ),
        operation(
            6,
            pointer,
            OperationKind::GetElementPointer {
                base: ValueId(4),
                offset: ValueId(3),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(5),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
        operation(
            7,
            scalar.clone(),
            OperationKind::Load {
                pointer: ValueId(6),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("full-envelope-cross-access");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                Type::slice(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite),
                scalar,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    let analyze = |extent| {
        derive_kernel_memory_obligations_for_launch(
            &module,
            &module.kernels[0].id,
            ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [extent, 1, 1],
            },
            FormalIndexWidth::Bits64,
        )
        .unwrap()
    };
    let small = analyze(2);
    let full = analyze(4);
    assert!(small.is_complete() && full.is_complete());
    assert_eq!(small.obligations().accesses().len(), 2);
    assert_eq!(full.obligations().accesses().len(), 2);
    assert!(small.obligations().inter_invocation_conflicts().is_empty());
    assert_eq!(full.obligations().inter_invocation_conflicts().len(), 1);
    let conflict = &full.obligations().inter_invocation_conflicts()[0];
    assert_eq!(conflict.left(), full.obligations().accesses()[0].location());
    assert_eq!(
        conflict.right(),
        full.obligations().accesses()[1].location()
    );
}
