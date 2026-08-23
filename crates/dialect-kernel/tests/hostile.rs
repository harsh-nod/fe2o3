use dialect_kernel::{
    AccessKindAttr, AlgorithmOp, AlgorithmType, AtomicOrderingAttr, AtomicScopeAttr,
    CheckedTiledIndex2DOp, DIALECT_NAME, DYNAMIC_EXTENT, DimensionAttr, DimensionOp,
    ITERATION_DOMAIN_ATTR_KEY, IndexConstantOp, IndexType, IndexValueAttr, IterationDomainAttr,
    KernelError, MAX_ITERATION_RANK, MAX_RANKED_MEMORY_RANK, RankedAccessOp, RankedMemoryError,
    RankedViewOp, RankedViewType, RegistrationError, RegistrationOutcome, SemanticOwner,
    StructuredAlgorithmOp, register_dialect,
};
use pliron::{
    attribute::Attribute,
    builtin::{
        attributes::BytesAttr, op_interfaces::SingleBlockRegionInterface, ops::ModuleOp,
        types::UnitType,
    },
    common_traits::Verify,
    context::Context,
    dialect::DialectName,
    identifier::Identifier,
    op::{Op, op_cast, verify_op},
    operation::{Operation, verify_operation},
    parsable::{Parsable, parse_from_str},
    printable::Printable,
    r#type::{TypeHandle, Typed},
};

fn kernel_name() -> DialectName {
    DialectName::try_new(DIALECT_NAME).expect("valid dialect")
}

#[test]
fn registration_is_real_idempotent_and_collision_safe() {
    let context = &mut Context::new();
    assert_eq!(
        register_dialect(context, &kernel_name()),
        Ok(RegistrationOutcome::Registered)
    );
    assert_eq!(
        register_dialect(context, &kernel_name()),
        Ok(RegistrationOutcome::AlreadyRegistered)
    );
    assert_eq!(
        register_dialect(
            context,
            &DialectName::try_new("schedule").expect("valid dialect")
        ),
        Err(RegistrationError::WrongDialect)
    );

    let parsed = parse_from_str(TypeHandle::parser(()), context, "kernel.algorithm <2>")
        .expect("registered type parses");
    assert!(parsed.verify(context).is_ok());
    let parsed = parse_from_str(
        <Box<dyn Attribute>>::parser(()),
        context,
        "kernel.iteration_domain <2>",
    )
    .expect("registered attribute parses");
    assert!(parsed.verify(context).is_ok());

    let operation = AlgorithmOp::new(context, 2).expect("bounded algorithm");
    let module = ModuleOp::new(context, "registration".try_into().expect("valid name"));
    module.append_operation(context, operation.get_operation(), 0);
    let printed = module.get_operation().disp(context).to_string();
    let parsed = parse_from_str(Operation::top_level_parser(), context, &printed)
        .expect("registered operation parses");
    verify_operation(parsed, context).expect("parsed operation verifies");
}

#[test]
fn hostile_registration_marker_is_rejected() {
    let context = &mut Context::new();
    let key: Identifier = "fe2o3_dialect_kernel_registration_v1"
        .try_into()
        .expect("valid key");
    let hostile = context.aux_data.insert(Box::new(17_u32));
    context.aux_data_map.insert(key, hostile);
    assert_eq!(
        register_dialect(context, &kernel_name()),
        Err(RegistrationError::MarkerCollision)
    );
    context.aux_data.remove(hostile);
    assert_eq!(
        register_dialect(context, &kernel_name()),
        Err(RegistrationError::CorruptMarker)
    );
}

#[test]
fn constructors_and_parsed_values_enforce_rank_bounds() {
    let context = &mut Context::new();
    assert_eq!(
        AlgorithmType::new(context, 0).unwrap_err(),
        KernelError::IterationRankOutOfBounds(0)
    );
    assert_eq!(
        IterationDomainAttr::new(MAX_ITERATION_RANK + 1).unwrap_err(),
        KernelError::IterationRankOutOfBounds(MAX_ITERATION_RANK + 1)
    );

    let parsed = parse_from_str(TypeHandle::parser(()), context, "kernel.algorithm <0>")
        .expect("syntax is valid before semantic verification");
    assert!(parsed.verify(context).is_err());
    let parsed = parse_from_str(
        <Box<dyn Attribute>>::parser(()),
        context,
        "kernel.iteration_domain <4294967295>",
    )
    .expect("bounded scalar syntax parses");
    assert!(parsed.verify(context).is_err());

    let parsed = parse_from_str(
        TypeHandle::parser(()),
        context,
        "kernel.ranked_view <32, false, [0, 64]>",
    )
    .expect("ranked dynamic type parses");
    assert!(parsed.verify(context).is_ok());
}

#[test]
fn ranked_view_constructors_enforce_rank_width_and_dynamic_extent_count() {
    let context = &mut Context::new();
    register_dialect(context, &kernel_name()).unwrap();
    assert_eq!(
        RankedViewType::new(context, 32, false, vec![]).unwrap_err(),
        RankedMemoryError::RankOutOfBounds(0),
    );
    assert_eq!(
        RankedViewType::new(context, 32, false, vec![1; MAX_RANKED_MEMORY_RANK + 1]).unwrap_err(),
        RankedMemoryError::RankOutOfBounds(MAX_RANKED_MEMORY_RANK + 1),
    );
    assert_eq!(
        RankedViewType::new(context, 24, false, vec![16]).unwrap_err(),
        RankedMemoryError::UnsupportedElementWidth(24),
    );
    assert!(
        RankedViewType::new(context, 128, true, vec![1; MAX_RANKED_MEMORY_RANK]).is_ok(),
        "the documented maximum rank must remain admitted",
    );

    let first_extent = IndexConstantOp::new(context, 17);
    let second_extent = IndexConstantOp::new(context, 19);
    let dynamic =
        RankedViewType::new(context, 32, false, vec![DYNAMIC_EXTENT, 64, DYNAMIC_EXTENT]).unwrap();
    assert_eq!(
        RankedViewOp::new(context, dynamic, vec![]).err().unwrap(),
        RankedMemoryError::DynamicExtentCountMismatch {
            expected: 2,
            actual: 0,
        },
    );
    let dynamic =
        RankedViewType::new(context, 32, false, vec![DYNAMIC_EXTENT, 64, DYNAMIC_EXTENT]).unwrap();
    let view = RankedViewOp::new(
        context,
        dynamic,
        vec![first_extent.result(context), second_extent.result(context)],
    )
    .unwrap();
    assert_eq!(
        view.dynamic_extent(context, 0),
        Some(first_extent.result(context))
    );
    assert_eq!(view.dynamic_extent(context, 1), None);
    assert_eq!(
        view.dynamic_extent(context, 2),
        Some(second_extent.result(context))
    );
    assert_eq!(view.dynamic_extent(context, 3), None);

    let foreign = AlgorithmOp::new(context, 1).unwrap();
    let foreign_result = foreign.get_operation().deref(context).get_result(0);
    let foreign_extent_type =
        RankedViewType::new(context, 32, false, vec![DYNAMIC_EXTENT]).unwrap();
    let foreign_extent =
        RankedViewOp::new(context, foreign_extent_type, vec![foreign_result]).unwrap();
    assert!(verify_op(&foreign_extent, context).is_err());
}

#[test]
fn ranked_memory_local_verifiers_reject_foreign_indices_rank_mismatch_and_writes() {
    let context = &mut Context::new();
    register_dialect(context, &kernel_name()).unwrap();
    let extent = IndexConstantOp::new(context, 32);
    verify_op(&extent, context).unwrap();
    let view_type = RankedViewType::new(context, 32, false, vec![DYNAMIC_EXTENT, 64]).unwrap();
    let view = RankedViewOp::new(context, view_type, vec![extent.result(context)]).unwrap();
    verify_op(&view, context).unwrap();

    let index = IndexConstantOp::new(context, 4);
    let column = IndexConstantOp::new(context, 7);
    let read = RankedAccessOp::new(
        context,
        AccessKindAttr::Read,
        view.result(context),
        vec![index.result(context), column.result(context)],
    )
    .unwrap();
    verify_op(&read, context).unwrap();
    assert_eq!(
        RankedAccessOp::new(
            context,
            AccessKindAttr::AtomicRead,
            view.result(context),
            vec![index.result(context), column.result(context)],
        )
        .err()
        .unwrap(),
        RankedMemoryError::MissingAtomicContract,
    );
    let atomic_read = RankedAccessOp::new_atomic(
        context,
        AccessKindAttr::AtomicRead,
        AtomicOrderingAttr::Acquire,
        AtomicScopeAttr::Device,
        view.result(context),
        vec![index.result(context), column.result(context)],
    )
    .unwrap();
    verify_op(&atomic_read, context).unwrap();
    assert_eq!(
        RankedAccessOp::new(
            context,
            AccessKindAttr::Read,
            view.result(context),
            vec![index.result(context)],
        )
        .err()
        .unwrap(),
        RankedMemoryError::OperandCountMismatch {
            expected: 2,
            actual: 1,
        },
    );
    assert_eq!(
        RankedAccessOp::new(
            context,
            AccessKindAttr::Write,
            view.result(context),
            vec![index.result(context), column.result(context)],
        )
        .err()
        .unwrap(),
        RankedMemoryError::WriteThroughReadOnlyView,
    );

    let foreign = AlgorithmOp::new(context, 1).unwrap();
    let foreign_result = foreign.get_operation().deref(context).get_result(0);
    let raw = Operation::new(
        context,
        RankedAccessOp::get_concrete_op_info(),
        vec![],
        vec![view.result(context), foreign_result, column.result(context)],
        vec![],
        0,
    );
    let hostile = RankedAccessOp::from_operation(raw);
    hostile.set_attr_kernel_access_kind(context, AccessKindAttr::Read);
    assert!(verify_op(&hostile, context).is_err());
}

#[test]
fn dimension_verifier_binds_selector_to_the_same_view_rank() {
    let context = &mut Context::new();
    register_dialect(context, &kernel_name()).unwrap();
    let view_type = RankedViewType::new(context, 16, true, vec![8, 16]).unwrap();
    let view = RankedViewOp::new(context, view_type, vec![]).unwrap();
    let dimension = DimensionOp::new(context, view.result(context), 1).unwrap();
    verify_op(&dimension, context).unwrap();
    assert_eq!(
        DimensionOp::new(context, view.result(context), 2)
            .err()
            .unwrap(),
        RankedMemoryError::DimensionOutOfBounds {
            dimension: 2,
            rank: 2,
        },
    );

    dimension.set_attr_kernel_dimension(context, DimensionAttr(2));
    assert!(verify_op(&dimension, context).is_err());
    assert!(
        dimension
            .result(context)
            .get_type(context)
            .deref(context)
            .is::<IndexType>()
    );
}

#[test]
fn checked_tiled_index_verifier_rejects_malformed_geometry_and_payload() {
    let context = &mut Context::new();
    register_dialect(context, &kernel_name()).unwrap();
    let values = (0..5)
        .map(|value| IndexConstantOp::new(context, value))
        .collect::<Vec<_>>();
    let valid = CheckedTiledIndex2DOp::new(
        context,
        values[0].result(context),
        values[1].result(context),
        values[2].result(context),
        values[3].result(context),
        values[4].result(context),
        [64, 16, 16, 4],
    );
    verify_op(&valid, context).unwrap();

    valid.set_attr_kernel_tile_rows(context, IndexValueAttr(15));
    assert!(verify_op(&valid, context).is_err());

    let raw = Operation::new(
        context,
        CheckedTiledIndex2DOp::get_concrete_op_info(),
        vec![IndexType::get(context).into()],
        values[..4]
            .iter()
            .map(|value| value.result(context))
            .collect(),
        vec![],
        0,
    );
    let missing_operand = CheckedTiledIndex2DOp::from_operation(raw);
    missing_operand.set_attr_kernel_lanes_per_tile(context, IndexValueAttr(64));
    missing_operand.set_attr_kernel_tile_rows(context, IndexValueAttr(16));
    missing_operand.set_attr_kernel_tile_columns(context, IndexValueAttr(16));
    missing_operand.set_attr_kernel_elements_per_lane(context, IndexValueAttr(4));
    assert!(verify_op(&missing_operand, context).is_err());
}

#[test]
fn op_interface_reports_only_target_neutral_kernel_ownership() {
    let context = &mut Context::new();
    let algorithm = AlgorithmOp::new(context, 3).expect("bounded algorithm");
    verify_op(&algorithm, context).expect("valid algorithm");

    let interface = op_cast::<dyn StructuredAlgorithmOp>(&algorithm).expect("interface present");
    assert_eq!(interface.semantic_owner(), SemanticOwner::Kernel);
    assert!(interface.is_target_neutral());
}

#[test]
fn hostile_operation_shapes_and_metadata_fail_verification() {
    let context = &mut Context::new();

    let wrong_type = UnitType::get(context);
    let raw = Operation::new(
        context,
        AlgorithmOp::get_concrete_op_info(),
        vec![wrong_type.into()],
        vec![],
        vec![],
        0,
    );
    let wrong_type_op = AlgorithmOp::from_operation(raw);
    wrong_type_op.set_iteration_domain(
        context,
        IterationDomainAttr::new(2).expect("bounded domain"),
    );
    assert!(verify_op(&wrong_type_op, context).is_err());

    let bounded_type = AlgorithmType::new(context, 2).expect("bounded type").into();
    let missing = Operation::new(
        context,
        AlgorithmOp::get_concrete_op_info(),
        vec![bounded_type],
        vec![],
        vec![],
        0,
    );
    assert!(verify_op(&AlgorithmOp::from_operation(missing), context).is_err());

    let mismatched = AlgorithmOp::new(context, 2).expect("bounded algorithm");
    mismatched
        .get_operation()
        .deref_mut(context)
        .attributes
        .0
        .insert(
            ITERATION_DOMAIN_ATTR_KEY.try_into().expect("valid key"),
            Box::new(IterationDomainAttr::new(3).expect("bounded domain")),
        );
    assert!(verify_op(&mismatched, context).is_err());

    let extra = AlgorithmOp::new(context, 2).expect("bounded algorithm");
    extra
        .get_operation()
        .deref_mut(context)
        .attributes
        .0
        .insert(
            "kernel_hostile_extra".try_into().expect("valid key"),
            Box::new(BytesAttr::new(vec![0xde, 0xad, 0xbe, 0xef])),
        );
    assert!(verify_op(&extra, context).is_err());
}
