//! Component oracles are not compiler-source or signed-owner qualification.
use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, Operation, OperationKind};

#[path = "production_nominal_diagnostic_requirements_v3_tests.rs"]
mod diagnostic_requirements;

const LIMIT: usize = 32 * 1024 * 1024;

fn run<T>(f: impl FnOnce(&mut Budget<'_>) -> R<T>) -> T {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.charge_work(17).unwrap();
    budget.reserve_storage(53).unwrap();
    let result = scoped(&mut budget, f).unwrap();
    assert_eq!(budget.storage(), 53);
    result
}

fn argument(kind: DescriptorArgumentKindV1) -> TypedDescriptorArgumentV1 {
    let (size, alignment, access) = match kind {
        DescriptorArgumentKindV1::Scalar(s) => (
            u64::from(s.size_bytes()),
            u32::from(s.alignment_bytes()),
            AccessMode::ByValue,
        ),
        DescriptorArgumentKindV1::SharedSlice(_) => (16, 8, AccessMode::ReadOnly),
        DescriptorArgumentKindV1::DisjointSlice(_) => (16, 8, AccessMode::WriteOnly),
        DescriptorArgumentKindV1::GlobalMutPointer(_) => (8, 8, AccessMode::ReadWrite),
        _ => (8, 8, AccessMode::ByValue),
    };
    TypedDescriptorArgumentV1 {
        name: "argument".to_owned(),
        kind,
        access,
        offset: 24,
        layout: None,
        source_size: size,
        source_alignment: alignment,
        rustc_abi_class: if size == 16 {
            RustcAbiClassV1::ScalarPair
        } else {
            RustcAbiClassV1::Scalar
        },
        semantic_type_identity: SemanticTypeIdentityV1::from_sha256([3; 32]),
    }
}

#[test]
fn nominal_descriptor_source_and_physical_identities_are_distinct() {
    run(|budget| {
        const CALLER: usize = size_of::<SourceTypeRecordV3>() * 2
            + size_of::<DeviceLayoutRecordV1>() * 2
            + size_of::<fe2o3_artifacts::RustNominalScalarEvidenceV3>();
        budget.reserve_storage(DESCRIPTOR_QUERY_STORAGE_V3 + CALLER)?;
        for (nominal, fixed) in [
            (
                DescriptorArgumentKindV1::CompilerLaidOutUsize,
                ScalarTypeV1::U64,
            ),
            (
                DescriptorArgumentKindV1::CompilerLaidOutIsize,
                ScalarTypeV1::I64,
            ),
        ] {
            let (n, nl) = records(nominal, budget)?;
            let (f, fl) = records(DescriptorArgumentKindV1::Scalar(fixed), budget)?;
            assert_ne!(n.identity(), f.identity());
            assert_eq!(nl, fl);
            let artifact = fe2o3_artifacts::RustNominalScalarEvidenceV3::new(
                if fixed == ScalarTypeV1::U64 {
                    fe2o3_artifacts::RustNominalScalarKindV3::Usize
                } else {
                    fe2o3_artifacts::RustNominalScalarKindV3::Isize
                },
                fe2o3_artifacts::PointerWidth::Bits64,
            )
            .unwrap();
            assert_eq!(artifact.size(), 8);
            assert_eq!(artifact.abi_alignment(), 8);
            assert_eq!(artifact.abi_class(), RustcAbiClassV1::Scalar);
            assert_eq!(n.descriptor().physical_scalar(), fixed);
        }
        Ok(())
    });
}

#[test]
fn nominal_descriptor_fixed_records_preserve_existing_source_and_layout_ids() {
    run(|budget| {
        const CALLER: usize = size_of::<SourceTypeRecordV3>()
            + size_of::<DeviceLayoutRecordV1>() * 2
            + size_of::<fe2o3_kernel_descriptor::SourceTypeRecordV1>();
        budget.reserve_storage(DESCRIPTOR_QUERY_STORAGE_V3 + CALLER)?;
        for scalar in [
            ScalarTypeV1::I8,
            ScalarTypeV1::U8,
            ScalarTypeV1::I16,
            ScalarTypeV1::U16,
            ScalarTypeV1::I32,
            ScalarTypeV1::U32,
            ScalarTypeV1::I64,
            ScalarTypeV1::U64,
            ScalarTypeV1::F16,
            ScalarTypeV1::F32,
            ScalarTypeV1::F64,
        ] {
            for kind in [
                DescriptorArgumentKindV1::Scalar(scalar),
                DescriptorArgumentKindV1::SharedSlice(scalar),
                DescriptorArgumentKindV1::DisjointSlice(scalar),
                DescriptorArgumentKindV1::GlobalMutPointer(scalar),
            ] {
                let (source, layout) = records(kind, budget)?;
                let (old_source, old_layout) = super::super::descriptor_records(kind);
                assert_eq!(source.identity(), old_source.identity());
                assert_eq!(layout, old_layout);
            }
        }
        Ok(())
    });
}

#[test]
fn nominal_descriptor_components_cover_fixed_nominal_and_slice_shapes() {
    for kind in [
        DescriptorArgumentKindV1::Scalar(ScalarTypeV1::U8),
        DescriptorArgumentKindV1::Scalar(ScalarTypeV1::U64),
        DescriptorArgumentKindV1::Scalar(ScalarTypeV1::I64),
        DescriptorArgumentKindV1::CompilerLaidOutUsize,
        DescriptorArgumentKindV1::CompilerLaidOutIsize,
        DescriptorArgumentKindV1::SharedSlice(ScalarTypeV1::U32),
        DescriptorArgumentKindV1::DisjointSlice(ScalarTypeV1::U32),
        DescriptorArgumentKindV1::GlobalMutPointer(ScalarTypeV1::U32),
    ] {
        let a = argument(kind);
        let (rows, count, ownership, alias) = components(&a).unwrap();
        assert_eq!(rows[0].offset, 24);
        assert_eq!(rows[0].access, a.access);
        assert_eq!(rows[0].alias, alias);
        let slice = matches!(
            kind,
            DescriptorArgumentKindV1::SharedSlice(_) | DescriptorArgumentKindV1::DisjointSlice(_)
        );
        assert_eq!(count, if slice { 2 } else { 1 });
        if slice {
            assert_eq!(rows[1].offset, 32);
            assert_eq!(rows[1].kind, PhysicalAbiComponentKind::SliceLengthU64);
            assert_eq!(rows[1].access, AccessMode::ByValue);
            assert_eq!(rows[1].alias, AliasSemantics::Value);
        }
        if matches!(
            kind,
            DescriptorArgumentKindV1::CompilerLaidOutUsize
                | DescriptorArgumentKindV1::CompilerLaidOutIsize
        ) {
            assert_eq!(ownership, OwnershipSemantics::ByValue);
            assert_eq!(alias, AliasSemantics::Value);
            assert_eq!((rows[0].size, rows[0].alignment), (8, 8));
        }
    }
}

#[test]
fn nominal_descriptor_same_width_kind_substitutions_are_refused() {
    use SemanticRustTypeKindV1 as K;
    assert!(nominal_kind_matches(
        DescriptorArgumentKindV1::CompilerLaidOutUsize,
        K::Usize
    ));
    assert!(nominal_kind_matches(
        DescriptorArgumentKindV1::CompilerLaidOutIsize,
        K::Isize
    ));
    assert!(!nominal_kind_matches(
        DescriptorArgumentKindV1::CompilerLaidOutUsize,
        K::Isize
    ));
    assert!(!nominal_kind_matches(
        DescriptorArgumentKindV1::CompilerLaidOutIsize,
        K::Usize
    ));
    assert!(!nominal_kind_matches(
        DescriptorArgumentKindV1::Scalar(ScalarTypeV1::U64),
        K::Usize
    ));
    assert!(!nominal_kind_matches(
        DescriptorArgumentKindV1::Scalar(ScalarTypeV1::I64),
        K::Isize
    ));
    assert!(!nominal_kind_matches(
        DescriptorArgumentKindV1::CompilerLaidOutByValue,
        K::Usize
    ));
}

#[test]
fn nominal_descriptor_aggregate_is_an_exact_typed_refusal() {
    run(|budget| {
        assert!(matches!(
            records(DescriptorArgumentKindV1::CompilerLaidOutByValue, budget),
            Err(E::Mismatch("aggregate packing remains deferred"))
        ));
        Ok(())
    });
}

#[test]
fn nominal_descriptor_declared_target_sensitive_requirements_are_not_dropped() {
    for capability in [
        TargetCapability::Subgroups,
        TargetCapability::WorkgroupMemory,
        TargetCapability::Extension {
            namespace: fe2o3_kernel_ir::MATRIX_CAPABILITY_NAMESPACE.to_owned(),
            name: fe2o3_kernel_ir::BF16_F32_M16N16K16_CAPABILITY.to_owned(),
        },
        TargetCapability::Atomic {
            width_bits: 32,
            address_space: fe2o3_kernel_ir::AddressSpace::Global,
            max_scope: fe2o3_kernel_ir::SynchronizationScope::Device,
        },
    ] {
        run(|budget| {
            let mut module = Module::new("requirements");
            module.required_capabilities.insert(capability);
            assert!(matches!(
                inert_capabilities(&module, ProductionAmdTargetProfileV1::Gfx942, budget),
                Err(E::UnsupportedRequirements)
            ));
            Ok(())
        });
    }
}

#[test]
fn nominal_descriptor_operation_requirement_is_not_hidden_by_empty_declarations() {
    run(|budget| {
        let mut module = Module::new("operation_requirement");
        module
            .functions
            .push(fe2o3_kernel_ir::Function::internal_helper(
                "helper",
                fe2o3_kernel_ir::Signature::new(vec![], vec![]),
                vec![],
                vec![fe2o3_kernel_ir::BasicBlock {
                    id: fe2o3_kernel_ir::BlockId(0),
                    parameters: vec![],
                    terminator: Some(fe2o3_kernel_ir::Terminator::Return { values: vec![] }),
                    operations: vec![Operation {
                        results: vec![],
                        kind: OperationKind::Alloca {
                            element: fe2o3_kernel_ir::Type::Scalar(
                                fe2o3_kernel_ir::ScalarType::U32,
                            ),
                            count: None,
                            address_space: fe2o3_kernel_ir::AddressSpace::Workgroup,
                            alignment: 4,
                        },
                    }],
                }],
            ));
        assert!(matches!(
            inert_capabilities(&module, ProductionAmdTargetProfileV1::Gfx942, budget),
            Err(E::UnsupportedRequirements)
        ));
        Ok(())
    });
}

#[test]
fn nominal_descriptor_zero_lds_exact_source_launch_and_wave64_are_inert() {
    run(|budget| {
        let source = LaunchContract::new(
            1,
            fe2o3_artifacts::BlockSize::Exact(fe2o3_artifacts::Dimensions::new(64, 1, 1).unwrap()),
            fe2o3_artifacts::Dimensions::new(128, 1, 1).unwrap(),
            0,
            0,
        )
        .unwrap();
        let derived = launch(&source, budget)?;
        assert_eq!(derived.rank(), 1);
        assert_eq!(derived.max_grid().x(), 128);
        let mut module = Module::new("scalar");
        module.required_capabilities.insert(TargetCapability::Int64);
        assert_eq!(
            inert_capabilities(&module, ProductionAmdTargetProfileV1::Gfx942, budget)?,
            [CapabilityV1::AmdWave]
        );
        for (static_bytes, dynamic_bytes) in [(4, 0), (0, 4)] {
            let changed = LaunchContract::new(
                source.rank(),
                source.block_size(),
                source.max_grid(),
                static_bytes,
                dynamic_bytes,
            )
            .unwrap();
            assert!(matches!(
                launch(&changed, budget),
                Err(E::UnsupportedRequirements)
            ));
        }
        Ok(())
    });
}

#[test]
fn nominal_descriptor_vector_first_work_and_storage_denials_preserve_floor() {
    let header = size_of::<Vec<u64>>();
    let requested = 3 * size_of::<u64>();
    let mut work = Work::new(17);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.charge_work(17).unwrap();
    budget.reserve_storage(53).unwrap();
    assert!(matches!(
        scoped(&mut budget, |b| vector::<u64>(3, b)),
        Err(E::Resource(Resource::Work(_)))
    ));
    assert_eq!(budget.storage(), 53);
    assert_eq!(budget.work(), 17);
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, 53 + header + requested - 1);
    budget.charge_work(17).unwrap();
    budget.reserve_storage(53).unwrap();
    let error = scoped(&mut budget, |b| vector::<u64>(3, b)).unwrap_err();
    let E::Resource(Resource::Storage(error)) = error else {
        panic!("exact first reserve")
    };
    assert_eq!(error.actual(), 53 + header + requested);
    assert_eq!(error.limit(), 53 + header + requested - 1);
    assert_eq!(budget.storage(), 53);
    assert_eq!(budget.peak_storage(), 53);
    assert_eq!(budget.work(), 17);
}

#[test]
fn nominal_descriptor_paid_backing_and_unwind_restore_sibling_floor() {
    run(|budget| {
        let floor = budget.storage();
        let bytes = scoped(budget, |budget| {
            let mut value = vector::<u8>(7, budget)?;
            budget.charge_work(7)?;
            value.extend_from_slice(b"nominal");
            Ok(value)
        })?;
        assert_eq!(budget.storage(), floor);
        budget.reserve_storage(size_of::<Vec<u8>>() + bytes.capacity())?;
        let paid = budget.storage();
        let result: R<()> = scoped(budget, |budget| {
            let _backing = vector::<usize>(5, budget)?;
            panic!("component unwind after paid backing");
        });
        assert!(matches!(result, Err(E::Panicked)));
        assert_eq!(budget.storage(), paid);
        let extent = size_of::<Vec<u8>>() + bytes.capacity();
        drop(bytes);
        budget.release_storage(extent)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    });
}

#[test]
fn nominal_descriptor_panic_payload_destructor_runs_after_floor_restoration() {
    struct Payload;
    impl Drop for Payload {
        fn drop(&mut self) {
            panic!("payload destructor");
        }
    }
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.charge_work(17).unwrap();
    budget.reserve_storage(53).unwrap();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let _: R<()> = scoped(&mut budget, |budget| {
            let _backing = vector::<usize>(5, budget)?;
            std::panic::panic_any(Payload);
        });
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), 53);
    assert_eq!(budget.work(), 18);
}

#[test]
fn nominal_descriptor_content_hash_prepays_exact_framing_and_finalization() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.charge_work(17).unwrap();
    budget.reserve_storage(53).unwrap();
    let domain = b"component-domain\0";
    let binding = [7; 32];
    let payload = b"actual source bytes";
    let expected = domain.len() + 8 + 8 + 32 + 8 + payload.len() + 128;
    let value = scoped(&mut budget, |budget| {
        budget.reserve_storage(
            size_of::<Sha256>()
                + size_of::<[&[u8]; 2]>()
                + size_of::<[u8; 32]>()
                + size_of::<BuildEvidenceV1>(),
        )?;
        evidence(domain, &binding, payload, budget)
    })
    .unwrap();
    assert_eq!(budget.work(), 17 + expected);
    assert_eq!(budget.storage(), 53);
    let digest = domain_hash(domain, &[&binding, payload]);
    assert_eq!(
        value,
        BuildEvidenceV1::new(
            EvidenceIdentity::from_opaque_bytes(digest),
            EvidenceDigest::from_sha256_bytes(digest)
        )
    );
    let mut work = Work::new(17 + expected - 1);
    let mut budget = Budget::new(&mut work, LIMIT);
    budget.charge_work(17).unwrap();
    budget.reserve_storage(53).unwrap();
    assert!(matches!(
        scoped(&mut budget, |budget| {
            budget.reserve_storage(
                size_of::<Sha256>()
                    + size_of::<[&[u8]; 2]>()
                    + size_of::<[u8; 32]>()
                    + size_of::<BuildEvidenceV1>(),
            )?;
            evidence(domain, &binding, payload, budget)
        }),
        Err(E::Resource(Resource::Work(_)))
    ));
    assert_eq!(budget.work(), 17);
    assert_eq!(budget.storage(), 53);
}
