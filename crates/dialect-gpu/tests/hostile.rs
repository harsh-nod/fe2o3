use dialect_gpu::{
    AddressSpaceAttr, BarrierOp, CanonicalKirOperationAttr, CanonicalKirTerminatorAttr,
    ExecutionDomainAttr, ExecutionExtentAttr, ExecutionLayoutOp, FenceOp, GridIdentityAttr,
    HierarchyAttr, HierarchyIdOp, HierarchyIndexType, MemoryOrderAttr, MemoryScopeAttr,
    MemorySpaceOp, MemorySpaceType, RegistrationError, RegistrationOutcome, SubgroupSizeAttr,
    SynchronizationOpInterface, TargetNeutralGpuOpInterface, barrier_op_attr_names,
    optimization_v1::{
        AccessModeAttr, BFloat16Attr, BFloat16Type, IndexAttr, IndexType, MemoryAlignmentAttr,
        PointerType, PreservedOperationKindAttr, PreservedOperationOp, PreservedTerminatorOp,
        SliceType, VolatileAttr,
    },
    register_dialect,
};
use fe2o3_kernel_ir::{
    IntrinsicOperation, Operation as KirOperation, OperationKind, Terminator, Type as KirType,
    ValueDef, ValueId,
};
use pliron::{
    attribute::{AttrObj, verify_attr},
    builtin::{
        attributes::{BytesAttr, UnitAttr},
        op_interfaces::SingleBlockRegionInterface,
        ops::ModuleOp,
        types::UnitType,
    },
    combine::{Parser, eof},
    context::Context,
    identifier::Identifier,
    op::{Op, op_cast, verify_op},
    operation::{Operation, OperationParserConfig, verify_operation},
    parsable::{Parsable, parse_from_str},
    printable::Printable,
    r#type::{TypeHandle, verify_type},
};

fn round_trip_attribute(source: AttrObj) {
    let mut source_context = Context::new();
    register_dialect(&mut source_context).expect("source gpu registration");
    let text = source.disp(&source_context).to_string();

    for _ in 0..2 {
        let mut fresh = Context::new();
        register_dialect(&mut fresh).expect("fresh gpu registration");
        let parsed = parse_from_str(AttrObj::parser(()).skip(eof()), &mut fresh, &text)
            .expect("registered capability attribute must parse");
        verify_attr(&*parsed, &fresh).expect("parsed capability attribute must verify");
        assert_eq!(parsed.disp(&fresh).to_string(), text);
    }
}

fn round_trip_type(build: impl Fn(&Context) -> TypeHandle) {
    let mut source = Context::new();
    register_dialect(&mut source).expect("source gpu registration");
    let text = build(&source).disp(&source).to_string();

    for _ in 0..2 {
        let mut fresh = Context::new();
        register_dialect(&mut fresh).expect("fresh gpu registration");
        let parsed = parse_from_str(TypeHandle::parser(()).skip(eof()), &mut fresh, &text)
            .expect("registered capability type must parse");
        verify_type(&*parsed.deref(&fresh), &fresh).expect("parsed capability type must verify");
        assert_eq!(parsed.disp(&fresh).to_string(), text);
    }
}

#[test]
fn registration_is_real_duplicate_safe_and_round_trips_entities() {
    let mut context = Context::new();
    assert_eq!(
        register_dialect(&mut context),
        Ok(RegistrationOutcome::Registered)
    );
    assert_eq!(
        register_dialect(&mut context),
        Ok(RegistrationOutcome::AlreadyRegistered)
    );

    let attribute: AttrObj = Box::new(HierarchyAttr::Workgroup);
    let attribute_text = attribute.disp(&context).to_string();
    let parsed_attribute = parse_from_str(
        AttrObj::parser(()).skip(eof()),
        &mut context,
        &attribute_text,
    )
    .expect("registered gpu attribute must parse");
    assert_eq!(
        parsed_attribute.downcast_ref::<HierarchyAttr>(),
        Some(&HierarchyAttr::Workgroup)
    );

    let ty: TypeHandle = HierarchyIndexType::get(&context, HierarchyAttr::Subgroup).into();
    let type_text = ty.disp(&context).to_string();
    let parsed_type = parse_from_str(TypeHandle::parser(()).skip(eof()), &mut context, &type_text)
        .expect("registered gpu type must parse");
    assert_eq!(
        parsed_type
            .deref(&context)
            .downcast_ref::<HierarchyIndexType>()
            .expect("typed hierarchy index")
            .hierarchy(),
        HierarchyAttr::Subgroup
    );

    let barrier = BarrierOp::new(
        &mut context,
        HierarchyAttr::Workgroup,
        MemoryScopeAttr::Device,
        AddressSpaceAttr::Global,
        MemoryOrderAttr::AcquireRelease,
    );
    let operation_text = barrier.disp(&context).to_string();
    let parsed_operation = parse_from_str(
        Operation::parser(OperationParserConfig {
            look_for_outlined_attrs: false,
        })
        .skip(eof()),
        &mut context,
        &operation_text,
    )
    .expect("registered gpu operation must parse");
    let parsed = Operation::get_op_dyn(parsed_operation, &context);
    assert!(parsed.is::<BarrierOp>());
    verify_op(&*parsed, &context).expect("parsed barrier must verify");

    for operation in [
        HierarchyIdOp::new(&mut context, HierarchyAttr::Subgroup).get_operation(),
        MemorySpaceOp::new(&mut context, AddressSpaceAttr::Workgroup).get_operation(),
    ] {
        let module = ModuleOp::new(
            &mut context,
            "gpu_roundtrip".try_into().expect("valid name"),
        );
        module.append_operation(&mut context, operation, 0);
        let printed = module.get_operation().disp(&context).to_string();
        let parsed = parse_from_str(Operation::top_level_parser(), &mut context, &printed)
            .expect("registered result operation must parse");
        verify_operation(parsed, &context).expect("parsed result operation must verify");
    }
}

#[test]
fn every_capability_attribute_and_type_family_round_trips_in_fresh_contexts() {
    let operation = KirOperation::effect_free(
        ValueDef::new(ValueId(0), KirType::INDEX),
        OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
    );
    let attributes: Vec<AttrObj> = vec![
        Box::new(HierarchyAttr::Workgroup),
        Box::new(AddressSpaceAttr::Workgroup),
        Box::new(MemoryScopeAttr::Device),
        Box::new(MemoryOrderAttr::AcquireRelease),
        Box::new(GridIdentityAttr(17)),
        Box::new(ExecutionExtentAttr(257)),
        Box::new(ExecutionDomainAttr::FullPhysicalWorkgroups),
        Box::new(SubgroupSizeAttr(64)),
        Box::new(AccessModeAttr::ReadWrite),
        Box::new(MemoryAlignmentAttr(16)),
        Box::new(VolatileAttr(true)),
        Box::new(IndexAttr(31)),
        Box::new(BFloat16Attr(0x3f80)),
        Box::new(CanonicalKirOperationAttr::new(&operation).expect("canonical operation")),
        Box::new(
            CanonicalKirTerminatorAttr::new(&Terminator::Unreachable)
                .expect("canonical terminator"),
        ),
    ];
    for attribute in attributes {
        round_trip_attribute(attribute);
    }

    round_trip_type(|context| HierarchyIndexType::get(context, HierarchyAttr::Subgroup).into());
    round_trip_type(|context| MemorySpaceType::get(context, AddressSpaceAttr::Global).into());
    round_trip_type(|context| IndexType::get(context).into());
    round_trip_type(|context| BFloat16Type::get(context).into());
    round_trip_type(|context| {
        PointerType::get(
            context,
            UnitType::get(context).into(),
            AddressSpaceAttr::Global,
            AccessModeAttr::ReadOnly,
        )
        .into()
    });
    round_trip_type(|context| {
        SliceType::get(
            context,
            UnitType::get(context).into(),
            AddressSpaceAttr::Workgroup,
            AccessModeAttr::ReadWrite,
        )
        .into()
    });
}

#[test]
fn every_target_neutral_capability_shell_op_round_trips_in_a_fresh_context() {
    let text = {
        let mut source = Context::new();
        register_dialect(&mut source).expect("source gpu registration");
        let module = ModuleOp::new(
            &mut source,
            Identifier::try_from("gpu_capability_shell").unwrap(),
        );
        for operation in [
            HierarchyIdOp::new(&mut source, HierarchyAttr::Lane).get_operation(),
            ExecutionLayoutOp::new(&mut source, 19, [256, 1, 1], [64, 1, 1], 64).get_operation(),
            MemorySpaceOp::new(&mut source, AddressSpaceAttr::Workgroup).get_operation(),
            BarrierOp::new(
                &mut source,
                HierarchyAttr::Workgroup,
                MemoryScopeAttr::Workgroup,
                AddressSpaceAttr::Workgroup,
                MemoryOrderAttr::AcquireRelease,
            )
            .get_operation(),
            FenceOp::new(
                &mut source,
                MemoryScopeAttr::Device,
                AddressSpaceAttr::Global,
                MemoryOrderAttr::Release,
            )
            .get_operation(),
        ] {
            module.append_operation(&mut source, operation, 0);
        }
        module.get_operation().disp(&source).to_string()
    };

    let mut canonical_text = None;
    for _ in 0..2 {
        let mut fresh = Context::new();
        register_dialect(&mut fresh).expect("fresh gpu registration");
        let parsed = parse_from_str(Operation::top_level_parser(), &mut fresh, &text)
            .expect("registered capability shell must parse");
        verify_operation(parsed, &fresh).expect("parsed capability shell must verify");
        let parsed_text = parsed.disp(&fresh).to_string();
        if let Some(canonical_text) = &canonical_text {
            assert_eq!(&parsed_text, canonical_text);
        } else {
            canonical_text = Some(parsed_text);
        }
    }
}

#[test]
fn hostile_registration_marker_is_rejected() {
    let mut context = Context::new();
    let key =
        Identifier::try_from("fe2o3_dialect_gpu_explicit_registration").expect("valid marker key");
    let hostile = context.aux_data.insert(Box::new(17_u32));
    context.aux_data_map.insert(key.clone(), hostile);
    assert_eq!(
        register_dialect(&mut context),
        Err(RegistrationError::MarkerCollision)
    );
    context.aux_data.remove(hostile);
    assert_eq!(
        register_dialect(&mut context),
        Err(RegistrationError::CorruptMarker)
    );
}

#[test]
fn analysis_incomplete_legacy_ops_stay_rejected_after_fresh_context_round_trip() {
    let families = [
        PreservedOperationKindAttr::Intrinsic,
        PreservedOperationKindAttr::MemoryIntrinsic,
        PreservedOperationKindAttr::Alloca,
        PreservedOperationKindAttr::GuardedLoad,
        PreservedOperationKindAttr::GuardedStore,
        PreservedOperationKindAttr::Barrier,
        PreservedOperationKindAttr::Atomic,
        PreservedOperationKindAttr::Fence,
        PreservedOperationKindAttr::WorkgroupBarrier,
        PreservedOperationKindAttr::WorkgroupMemory,
        PreservedOperationKindAttr::Matrix,
        PreservedOperationKindAttr::Gfx950LdsTranspose,
        PreservedOperationKindAttr::Wave,
        PreservedOperationKindAttr::InlineAssembly,
    ];

    for family in families {
        let text = {
            let mut source = Context::new();
            register_dialect(&mut source).expect("source gpu registration");
            let module = ModuleOp::new(
                &mut source,
                Identifier::try_from("legacy_operation").unwrap(),
            );
            let operation = PreservedOperationOp::new(&mut source, family, vec![], vec![]);
            module.append_operation(&mut source, operation.get_operation(), 0);
            module.get_operation().disp(&source).to_string()
        };
        let mut fresh = Context::new();
        register_dialect(&mut fresh).expect("fresh gpu registration");
        let decoded = parse_from_str(Operation::top_level_parser(), &mut fresh, &text)
            .expect("legacy syntax remains bounded and parseable");
        assert!(
            verify_operation(decoded, &fresh).is_err(),
            "analysis-incomplete family {family:?} entered a verified fresh graph"
        );
    }

    let text = {
        let mut source = Context::new();
        register_dialect(&mut source).expect("source gpu registration");
        let module = ModuleOp::new(
            &mut source,
            Identifier::try_from("legacy_terminator").unwrap(),
        );
        let operation = PreservedTerminatorOp::new_unreachable(&mut source);
        module.append_operation(&mut source, operation.get_operation(), 0);
        module.get_operation().disp(&source).to_string()
    };
    let mut fresh = Context::new();
    register_dialect(&mut fresh).expect("fresh gpu registration");
    let decoded = parse_from_str(Operation::top_level_parser(), &mut fresh, &text)
        .expect("legacy terminator syntax remains parseable");
    assert!(verify_operation(decoded, &fresh).is_err());
}

#[test]
fn valid_operations_are_target_neutral_and_non_authoritative() {
    let mut context = Context::new();
    register_dialect(&mut context).expect("gpu registration");

    let hierarchy = HierarchyIdOp::new(&mut context, HierarchyAttr::Lane);
    let memory = MemorySpaceOp::new(&mut context, AddressSpaceAttr::Workgroup);
    let barrier = BarrierOp::new(
        &mut context,
        HierarchyAttr::Subgroup,
        MemoryScopeAttr::Subgroup,
        AddressSpaceAttr::Global,
        MemoryOrderAttr::SequentiallyConsistent,
    );
    let fence = FenceOp::new(
        &mut context,
        MemoryScopeAttr::System,
        AddressSpaceAttr::Global,
        MemoryOrderAttr::Release,
    );

    for op in [&hierarchy as &dyn Op, &memory, &barrier, &fence] {
        verify_op(op, &context).expect("valid gpu shell operation");
        let interface = op_cast::<dyn TargetNeutralGpuOpInterface>(op)
            .expect("target-neutral interface must be registered");
        assert!(interface.is_target_neutral());
        assert!(!interface.grants_runtime_authority());
    }
    let sync = op_cast::<dyn SynchronizationOpInterface>(&barrier)
        .expect("barrier synchronization interface");
    assert!(sync.is_synchronization());
}

#[test]
fn verifier_rejects_mismatched_result_types_and_attributes() {
    let mut context = Context::new();
    register_dialect(&mut context).expect("gpu registration");

    let wrong_type = MemorySpaceType::get(&context, AddressSpaceAttr::Global).into();
    let operation = Operation::new(
        &mut context,
        HierarchyIdOp::get_concrete_op_info(),
        vec![wrong_type],
        vec![],
        vec![],
        0,
    );
    let malformed = HierarchyIdOp::from_operation(operation);
    malformed.set_attr_gpu_hierarchy_id_hierarchy(&context, HierarchyAttr::Grid);
    assert!(verify_op(&malformed, &context).is_err());

    let hierarchy = HierarchyIdOp::new(&mut context, HierarchyAttr::Workgroup);
    hierarchy
        .get_operation()
        .deref_mut(&context)
        .attributes
        .set(
            dialect_gpu::hierarchy_id_op_attr_names::ATTR_KEY_GPU_HIERARCHY_ID_HIERARCHY.clone(),
            UnitAttr,
        );
    assert!(verify_op(&hierarchy, &context).is_err());

    let wrong_result = HierarchyIndexType::get(&context, HierarchyAttr::Lane).into();
    let operation = Operation::new(
        &mut context,
        MemorySpaceOp::get_concrete_op_info(),
        vec![wrong_result],
        vec![],
        vec![],
        0,
    );
    let malformed = MemorySpaceOp::from_operation(operation);
    malformed.set_attr_gpu_memory_space_address_space(&context, AddressSpaceAttr::Private);
    assert!(verify_op(&malformed, &context).is_err());
}

#[test]
fn execution_layout_distinguishes_dynamic_global_axes_from_physical_workgroups() {
    let mut context = Context::new();
    register_dialect(&mut context).expect("gpu registration");

    let dynamic = ExecutionLayoutOp::new(&mut context, 9, [0, 128, 1], [8, 8, 1], 64);
    verify_op(&dynamic, &context).expect("dynamic global extent is retained explicitly");
    assert_eq!(dynamic.global_extents(&context), Some([0, 128, 1]));
    assert_eq!(dynamic.workgroup_extents(&context), Some([8, 8, 1]));
    assert_eq!(dynamic.subgroup_size(&context), Some(64));
    assert_eq!(
        dynamic.execution_domain(&context),
        ExecutionDomainAttr::PotentiallyPartial
    );

    let authenticated_dynamic = ExecutionLayoutOp::new_with_domain(
        &mut context,
        9,
        [0, 128, 1],
        [8, 8, 1],
        64,
        ExecutionDomainAttr::FullPhysicalWorkgroups,
    );
    verify_op(&authenticated_dynamic, &context)
        .expect("dynamic full-workgroup provenance is structurally consistent");

    let contradictory = ExecutionLayoutOp::new_with_domain(
        &mut context,
        9,
        [65, 1, 1],
        [64, 1, 1],
        64,
        ExecutionDomainAttr::FullPhysicalWorkgroups,
    );
    assert!(verify_op(&contradictory, &context).is_err());

    let zero_workgroup = ExecutionLayoutOp::new(&mut context, 9, [64, 1, 1], [0, 1, 1], 1);
    assert!(verify_op(&zero_workgroup, &context).is_err());
    let partial_final_subgroup = ExecutionLayoutOp::new(&mut context, 9, [64, 1, 1], [8, 8, 1], 48);
    verify_op(&partial_final_subgroup, &context)
        .expect("an exact workgroup may end in a partial physical subgroup");
    let zero_subgroup = ExecutionLayoutOp::new(&mut context, 9, [64, 1, 1], [8, 8, 1], 0);
    assert!(verify_op(&zero_subgroup, &context).is_err());
}

#[test]
fn verifier_rejects_hostile_synchronization_combinations() {
    let mut context = Context::new();
    register_dialect(&mut context).expect("gpu registration");

    let private = BarrierOp::new(
        &mut context,
        HierarchyAttr::Workgroup,
        MemoryScopeAttr::Workgroup,
        AddressSpaceAttr::Private,
        MemoryOrderAttr::AcquireRelease,
    );
    assert!(verify_op(&private, &context).is_err());

    let narrow = BarrierOp::new(
        &mut context,
        HierarchyAttr::Workgroup,
        MemoryScopeAttr::Subgroup,
        AddressSpaceAttr::Global,
        MemoryOrderAttr::AcquireRelease,
    );
    assert!(verify_op(&narrow, &context).is_err());

    let weak = BarrierOp::new(
        &mut context,
        HierarchyAttr::Subgroup,
        MemoryScopeAttr::Subgroup,
        AddressSpaceAttr::Global,
        MemoryOrderAttr::Acquire,
    );
    assert!(verify_op(&weak, &context).is_err());

    let constant_release = FenceOp::new(
        &mut context,
        MemoryScopeAttr::Device,
        AddressSpaceAttr::Constant,
        MemoryOrderAttr::Release,
    );
    assert!(verify_op(&constant_release, &context).is_err());
}

#[test]
fn verifier_rejects_missing_extra_and_structural_payloads() {
    let mut context = Context::new();
    register_dialect(&mut context).expect("gpu registration");

    let missing = BarrierOp::new(
        &mut context,
        HierarchyAttr::Workgroup,
        MemoryScopeAttr::Workgroup,
        AddressSpaceAttr::Global,
        MemoryOrderAttr::AcquireRelease,
    );
    missing
        .get_operation()
        .deref_mut(&context)
        .attributes
        .0
        .remove(&*barrier_op_attr_names::ATTR_KEY_GPU_BARRIER_ORDER);
    assert!(verify_op(&missing, &context).is_err());

    let extra_hierarchy = HierarchyIdOp::new(&mut context, HierarchyAttr::Grid);
    let extra_memory = MemorySpaceOp::new(&mut context, AddressSpaceAttr::Global);
    let extra_barrier = BarrierOp::new(
        &mut context,
        HierarchyAttr::Workgroup,
        MemoryScopeAttr::Device,
        AddressSpaceAttr::Global,
        MemoryOrderAttr::SequentiallyConsistent,
    );
    let extra_fence = FenceOp::new(
        &mut context,
        MemoryScopeAttr::Device,
        AddressSpaceAttr::Global,
        MemoryOrderAttr::Acquire,
    );
    for op in [
        &extra_hierarchy as &dyn Op,
        &extra_memory,
        &extra_barrier,
        &extra_fence,
    ] {
        op.get_operation().deref_mut(&context).attributes.set(
            Identifier::try_from("gpu_hostile_extra").expect("valid key"),
            BytesAttr::new(vec![0xde, 0xad, 0xbe, 0xef]),
        );
        assert!(
            verify_op(op, &context).is_err(),
            "{} accepted an undeclared byte payload",
            op.get_opid()
        );
    }

    let unit = UnitType::get(&context).into();
    let operation = Operation::new(
        &mut context,
        FenceOp::get_concrete_op_info(),
        vec![unit],
        vec![],
        vec![],
        0,
    );
    let malformed = FenceOp::from_operation(operation);
    malformed.set_attr_gpu_fence_memory_scope(&context, MemoryScopeAttr::Device);
    malformed.set_attr_gpu_fence_address_space(&context, AddressSpaceAttr::Global);
    malformed.set_attr_gpu_fence_order(&context, MemoryOrderAttr::Acquire);
    assert!(verify_op(&malformed, &context).is_err());
}
