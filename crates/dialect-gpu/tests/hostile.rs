use dialect_gpu::{
    AddressSpaceAttr, BarrierOp, FenceOp, HierarchyAttr, HierarchyIdOp, HierarchyIndexType,
    MemoryOrderAttr, MemoryScopeAttr, MemorySpaceOp, MemorySpaceType, RegistrationOutcome,
    SynchronizationOpInterface, TargetNeutralGpuOpInterface, barrier_op_attr_names,
    register_dialect,
};
use pliron::{
    attribute::AttrObj,
    builtin::{attributes::UnitAttr, types::UnitType},
    combine::{Parser, eof},
    context::Context,
    identifier::Identifier,
    op::{Op, op_cast, verify_op},
    operation::{Operation, OperationParserConfig},
    parsable::{Parsable, parse_from_str},
    printable::Printable,
    r#type::TypeHandle,
};

#[test]
fn registration_is_real_duplicate_safe_and_round_trips_entities() {
    let mut context = Context::new();
    assert_eq!(
        register_dialect(&mut context),
        RegistrationOutcome::Registered
    );
    assert_eq!(
        register_dialect(&mut context),
        RegistrationOutcome::AlreadyRegistered
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
}

#[test]
fn valid_operations_are_target_neutral_and_non_authoritative() {
    let mut context = Context::new();
    register_dialect(&mut context);

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
    register_dialect(&mut context);

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
fn verifier_rejects_hostile_synchronization_combinations() {
    let mut context = Context::new();
    register_dialect(&mut context);

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
    register_dialect(&mut context);

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

    let extra = FenceOp::new(
        &mut context,
        MemoryScopeAttr::Device,
        AddressSpaceAttr::Global,
        MemoryOrderAttr::Acquire,
    );
    extra.get_operation().deref_mut(&context).attributes.set(
        Identifier::try_from("gpu_hostile_extra").expect("valid key"),
        UnitAttr,
    );
    assert!(verify_op(&extra, &context).is_err());

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
