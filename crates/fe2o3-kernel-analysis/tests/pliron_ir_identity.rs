use dialect_gpu::{HierarchyAttr, HierarchyIdOp, HierarchyIndexType};
use dialect_kernel::{
    BranchOp, DIALECT_NAME, DimensionAttr, IndexBinaryKindAttr, IndexBinaryOp, IndexConstantOp,
    IndexType, IndexUnknownOp, InvocationDimensionAttr, IterationDomainAttr, MemorySpaceAttr,
    RankedViewOp, RankedViewType, ReturnOp, register_dialect,
};
use fe2o3_kernel_analysis::{
    MAX_PLIRON_IDENTITY_BLOCKS_V1, MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1,
    PlironIrIdentityChangeV1, PlironIrIdentityErrorV1, PlironIrPreservationErrorV1,
    PlironPreserveLocationV1, PlironPreserveSnapshotSideV1,
    derive_pliron_ir_structural_identity_v1, require_pliron_ir_structural_identity_preserved_v1,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{
        attributes::TypeAttr,
        op_interfaces::OneRegionInterface,
        ops::FuncOp,
        types::{FunctionType, UnitType},
    },
    context::{Context, Ptr},
    dialect::DialectName,
    identifier::Identifier,
    linked_list::ContainsLinkedList,
    op::Op,
    r#type::TypeHandle,
};

fn setup() -> Context {
    let mut context = Context::new();
    register_dialect(&mut context, &DialectName::try_new(DIALECT_NAME).unwrap()).unwrap();
    dialect_gpu::register_dialect(&mut context).unwrap();
    dialect_proof::register_dialect(&mut context).unwrap();
    context
}

fn empty_function(context: &mut Context, name: &str, inputs: Vec<TypeHandle>) -> FuncOp {
    FuncOp::new(
        context,
        name.try_into().unwrap(),
        FunctionType::get(context, inputs, vec![]),
    )
}

fn append<O: Op>(context: &Context, block: Ptr<BasicBlock>, operation: &O) {
    operation.get_operation().insert_at_back(block, context);
}

fn block(context: &mut Context, function: &FuncOp, label: &str) -> Ptr<BasicBlock> {
    let block = BasicBlock::new(context, Some(label.try_into().unwrap()), vec![]);
    block.insert_at_back(function.get_region(context), context);
    block
}

#[derive(Clone, Copy)]
struct ArithmeticVariant {
    first_constant: Option<u64>,
    second_constant: u64,
    kind: IndexBinaryKindAttr,
    swap_operands: bool,
}

fn arithmetic_function(context: &mut Context, variant: ArithmeticVariant) -> FuncOp {
    let function = empty_function(context, "identity_kernel", vec![]);
    let entry = function.get_entry_block(context);
    let first = match variant.first_constant {
        Some(value) => {
            let operation = IndexConstantOp::new(context, value);
            let result = operation.result(context);
            append(context, entry, &operation);
            result
        }
        None => {
            let operation = IndexUnknownOp::new(context);
            let result = operation.result(context);
            append(context, entry, &operation);
            result
        }
    };
    let second = IndexConstantOp::new(context, variant.second_constant);
    let second_result = second.result(context);
    append(context, entry, &second);
    let (lhs, rhs) = if variant.swap_operands {
        (second_result, first)
    } else {
        (first, second_result)
    };
    let binary = IndexBinaryOp::new(context, variant.kind, lhs, rhs);
    let ret = ReturnOp::new(context);
    append(context, entry, &binary);
    append(context, entry, &ret);
    function
}

fn baseline() -> ArithmeticVariant {
    ArithmeticVariant {
        first_constant: Some(2),
        second_constant: 3,
        kind: IndexBinaryKindAttr::Add,
        swap_operands: false,
    }
}

fn identity_change(
    before_variant: ArithmeticVariant,
    after_variant: ArithmeticVariant,
) -> PlironIrPreservationErrorV1 {
    let before_context = &mut setup();
    let before = arithmetic_function(before_context, before_variant);
    let after_context = &mut setup();
    let after = arithmetic_function(after_context, after_variant);
    require_pliron_ir_structural_identity_preserved_v1(
        before_context,
        &before,
        after_context,
        &after,
    )
    .unwrap_err()
}

fn verified_change(error: &PlironIrPreservationErrorV1) -> &PlironIrIdentityChangeV1 {
    let PlironIrPreservationErrorV1::IdentityChanged(change) = error else {
        panic!("expected a verified structural change, got {error}")
    };
    change
}

#[test]
fn no_op_rereads_and_independent_reconstruction_are_stable() {
    let context = &mut setup();
    let function = arithmetic_function(context, baseline());
    let first = derive_pliron_ir_structural_identity_v1(context, &function).unwrap();
    let second = derive_pliron_ir_structural_identity_v1(context, &function).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.sha256(), second.sha256());
    assert_eq!(first.block_count(), 1);
    assert_eq!(first.operation_count(), 4);
    assert_eq!(first.value_count(), 3);
    assert!(!first.grants_operational_semantics_or_refinement_authority());

    let other_context = &mut setup();
    let other = arithmetic_function(other_context, baseline());
    let reconstructed = derive_pliron_ir_structural_identity_v1(other_context, &other).unwrap();
    assert!(first.exactly_matches(&reconstructed));
    assert_eq!(first.sha256(), reconstructed.sha256());
}

#[test]
fn operation_name_mutation_reports_the_exact_site() {
    let error = identity_change(
        baseline(),
        ArithmeticVariant {
            first_constant: None,
            ..baseline()
        },
    );
    let change = verified_change(&error);
    assert!(matches!(
        change.location(),
        PlironPreserveLocationV1::Operation {
            block: 0,
            operation: 0,
            ..
        }
    ));
    assert_eq!(change.component(), "operation");
    assert!(error.to_string().contains("FE2O3-PRESERVE-010"));
}

#[test]
fn operand_wiring_mutation_reports_the_binary_operands() {
    let error = identity_change(
        baseline(),
        ArithmeticVariant {
            swap_operands: true,
            ..baseline()
        },
    );
    let change = verified_change(&error);
    assert!(matches!(
        change.location(),
        PlironPreserveLocationV1::Operation {
            block: 0,
            operation: 2,
            ..
        }
    ));
    assert_eq!(change.component(), "operands");
    assert_eq!(change.before(), "v0, v1");
    assert_eq!(change.after(), "v1, v0");
}

#[test]
fn constants_and_operator_attributes_mutate_the_identity() {
    let constant_error = identity_change(
        baseline(),
        ArithmeticVariant {
            first_constant: Some(4),
            ..baseline()
        },
    );
    let change = verified_change(&constant_error);
    assert!(matches!(
        change.location(),
        PlironPreserveLocationV1::Operation { operation: 0, .. }
    ));
    assert_eq!(change.component(), "attributes");
    assert!(change.before().contains('2'), "{}", change.before());
    assert!(change.after().contains('4'), "{}", change.after());

    let operator_error = identity_change(
        baseline(),
        ArithmeticVariant {
            kind: IndexBinaryKindAttr::Multiply,
            ..baseline()
        },
    );
    let change = verified_change(&operator_error);
    assert!(matches!(
        change.location(),
        PlironPreserveLocationV1::Operation { operation: 2, .. }
    ));
    assert_eq!(change.component(), "attributes");
}

#[test]
fn function_and_block_argument_types_are_retained() {
    let index_context = &mut setup();
    let index: TypeHandle = IndexType::get(index_context).into();
    let index_function = empty_function(index_context, "typed_kernel", vec![index]);
    let ret = ReturnOp::new(index_context);
    append(
        index_context,
        index_function.get_entry_block(index_context),
        &ret,
    );

    let unit_context = &mut setup();
    let unit: TypeHandle = UnitType::get(unit_context).into();
    let unit_function = empty_function(unit_context, "typed_kernel", vec![unit]);
    let ret = ReturnOp::new(unit_context);
    append(
        unit_context,
        unit_function.get_entry_block(unit_context),
        &ret,
    );
    let index_identity =
        derive_pliron_ir_structural_identity_v1(index_context, &index_function).unwrap();
    let unit_identity =
        derive_pliron_ir_structural_identity_v1(unit_context, &unit_function).unwrap();
    assert_ne!(index_identity, unit_identity);
}

fn cfg_function(context: &mut Context, target_right: bool) -> FuncOp {
    let function = empty_function(context, "cfg_kernel", vec![]);
    let entry = function.get_entry_block(context);
    let left = block(context, &function, "left");
    let right = block(context, &function, "right");
    let branch = BranchOp::new(context, if target_right { right } else { left });
    let left_return = ReturnOp::new(context);
    let right_return = ReturnOp::new(context);
    append(context, entry, &branch);
    append(context, left, &left_return);
    append(context, right, &right_return);
    function
}

#[test]
fn cfg_successor_mutation_reports_the_terminator_edge() {
    let before_context = &mut setup();
    let before = cfg_function(before_context, false);
    let after_context = &mut setup();
    let after = cfg_function(after_context, true);
    let error = require_pliron_ir_structural_identity_preserved_v1(
        before_context,
        &before,
        after_context,
        &after,
    )
    .unwrap_err();
    let change = verified_change(&error);
    assert!(matches!(
        change.location(),
        PlironPreserveLocationV1::Operation {
            block: 0,
            operation: 0,
            ..
        }
    ));
    assert_eq!(change.component(), "successors");
    assert_eq!(change.before(), "block 1");
    assert_eq!(change.after(), "block 2");
}

#[test]
fn block_labels_are_alpha_names_not_semantic_structure() {
    let first_context = &mut setup();
    let first = cfg_function(first_context, false);
    let second_context = &mut setup();
    let second = cfg_function(second_context, false);
    let blocks = second
        .get_region(second_context)
        .deref(second_context)
        .iter(second_context)
        .collect::<Vec<_>>();
    blocks[1]
        .deref_mut(second_context)
        .set_label(Some("renamed".try_into().unwrap()));
    assert!(
        require_pliron_ir_structural_identity_preserved_v1(
            first_context,
            &first,
            second_context,
            &second,
        )
        .is_ok()
    );
}

#[test]
fn unsupported_ranked_structure_fails_before_snapshot_comparison() {
    let before_context = &mut setup();
    let before = arithmetic_function(before_context, baseline());
    let after_context = &mut setup();
    let after = empty_function(after_context, "identity_kernel", vec![]);
    let hierarchy = HierarchyIdOp::new(after_context, HierarchyAttr::Lane);
    let ret = ReturnOp::new(after_context);
    append(
        after_context,
        after.get_entry_block(after_context),
        &hierarchy,
    );
    append(after_context, after.get_entry_block(after_context), &ret);

    let error = require_pliron_ir_structural_identity_preserved_v1(
        before_context,
        &before,
        after_context,
        &after,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        PlironIrPreservationErrorV1::SnapshotFailed {
            side: PlironPreserveSnapshotSideV1::After,
            source: PlironIrIdentityErrorV1::UnsupportedOperation {
                location: PlironPreserveLocationV1::Operation {
                    block: 0,
                    operation: 0,
                    ..
                },
                ..
            }
        }
    ));
    assert!(error.to_string().contains("FE2O3-PRESERVE-001"));
}

#[test]
fn operand_from_outside_the_function_never_enters_an_identity() {
    let context = &mut setup();
    let function = empty_function(context, "external_operand", vec![]);
    let entry = function.get_entry_block(context);
    let local = IndexConstantOp::new(context, 1);
    let external = IndexConstantOp::new(context, 2);
    let binary = IndexBinaryOp::new(
        context,
        IndexBinaryKindAttr::Add,
        local.result(context),
        external.result(context),
    );
    let ret = ReturnOp::new(context);
    append(context, entry, &local);
    append(context, entry, &binary);
    append(context, entry, &ret);

    let error = derive_pliron_ir_structural_identity_v1(context, &function).unwrap_err();
    assert!(matches!(
        error,
        PlironIrIdentityErrorV1::ExternalOperand {
            location: PlironPreserveLocationV1::Operation {
                block: 0,
                operation: 1,
                ..
            },
            operand: 1,
            ..
        }
    ));
    assert!(error.to_string().contains("FE2O3-PRESERVE-003"));
}

#[test]
fn malformed_snapshot_is_distinct_from_a_verified_change() {
    let before_context = &mut setup();
    let malformed = empty_function(before_context, "malformed", vec![]);
    let after_context = &mut setup();
    let after = arithmetic_function(after_context, baseline());
    let error = require_pliron_ir_structural_identity_preserved_v1(
        before_context,
        &malformed,
        after_context,
        &after,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        PlironIrPreservationErrorV1::SnapshotFailed {
            side: PlironPreserveSnapshotSideV1::Before,
            source: PlironIrIdentityErrorV1::StructuralVerificationFailed { .. }
        }
    ));
    assert!(!error.to_string().contains("FE2O3-PRESERVE-010"));
}

#[test]
fn block_resource_limit_fails_before_recursive_verification() {
    let context = &mut setup();
    let function = empty_function(context, "too_many_blocks", vec![]);
    for ordinal in 0..MAX_PLIRON_IDENTITY_BLOCKS_V1 {
        let label = format!("b{ordinal}");
        let _ = block(context, &function, &label);
    }
    let error = derive_pliron_ir_structural_identity_v1(context, &function).unwrap_err();
    assert_eq!(
        error,
        PlironIrIdentityErrorV1::ResourceLimitExceeded {
            location: PlironPreserveLocationV1::Function,
            resource: "basic blocks",
            actual: MAX_PLIRON_IDENTITY_BLOCKS_V1 + 1,
            limit: MAX_PLIRON_IDENTITY_BLOCKS_V1,
        }
    );
    assert!(error.to_string().contains("FE2O3-PRESERVE-002"));
}

#[test]
fn registered_printer_output_is_bounded_before_transcript_growth() {
    let context = &mut setup();
    let name = "x".repeat(MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1 + 1);
    let function = empty_function(context, &name, vec![]);
    let ret = ReturnOp::new(context);
    append(context, function.get_entry_block(context), &ret);

    let error = derive_pliron_ir_structural_identity_v1(context, &function).unwrap_err();
    assert!(matches!(
        error,
        PlironIrIdentityErrorV1::ResourceLimitExceeded {
            location: PlironPreserveLocationV1::Function,
            resource: "rendered entity bytes",
            limit: MAX_PLIRON_IDENTITY_ENTITY_TEXT_BYTES_V1,
            ..
        }
    ));
}

#[test]
fn attribute_ids_distinguish_equal_payload_text() {
    let before_context = &mut setup();
    let before = arithmetic_function(before_context, baseline());
    before
        .get_operation()
        .deref_mut(before_context)
        .attributes
        .set(
            Identifier::try_from("test_same_payload").unwrap(),
            DimensionAttr(7),
        );

    let after_context = &mut setup();
    let after = arithmetic_function(after_context, baseline());
    after
        .get_operation()
        .deref_mut(after_context)
        .attributes
        .set(
            Identifier::try_from("test_same_payload").unwrap(),
            InvocationDimensionAttr(7),
        );

    let error = require_pliron_ir_structural_identity_preserved_v1(
        before_context,
        &before,
        after_context,
        &after,
    )
    .unwrap_err();
    let change = verified_change(&error);
    assert_eq!(change.component(), "attributes");
    assert!(change.before().contains("kernel.dimension"));
    assert!(change.after().contains("kernel.invocation_dimension"));
}

#[test]
fn registered_but_unadmitted_attribute_fails_closed() {
    let context = &mut setup();
    let function = arithmetic_function(context, baseline());
    function.get_operation().deref_mut(context).attributes.set(
        Identifier::try_from("test_unadmitted").unwrap(),
        IterationDomainAttr::new(1).unwrap(),
    );

    let error = derive_pliron_ir_structural_identity_v1(context, &function).unwrap_err();
    assert!(matches!(
        error,
        PlironIrIdentityErrorV1::UnsupportedAttribute { .. }
    ));
    assert!(error.to_string().contains("FE2O3-PRESERVE-001"));
    assert!(error.to_string().contains("kernel.iteration_domain"));
}

#[test]
fn registered_but_unadmitted_type_fails_before_verification() {
    let context = &mut setup();
    let hierarchy: TypeHandle = HierarchyIndexType::get(context, HierarchyAttr::Lane).into();
    let function = empty_function(context, "unadmitted_type", vec![hierarchy]);
    let ret = ReturnOp::new(context);
    append(context, function.get_entry_block(context), &ret);

    let error = derive_pliron_ir_structural_identity_v1(context, &function).unwrap_err();
    assert!(matches!(
        error,
        PlironIrIdentityErrorV1::UnsupportedType { .. }
    ));
    assert!(error.to_string().contains("FE2O3-PRESERVE-001"));
    assert!(error.to_string().contains("gpu.hierarchy_index"));
}

#[test]
fn unadmitted_type_nested_in_function_type_fails_closed() {
    let context = &mut setup();
    let function = arithmetic_function(context, baseline());
    let hierarchy: TypeHandle = HierarchyIndexType::get(context, HierarchyAttr::Lane).into();
    let nested: TypeHandle = FunctionType::get(context, vec![hierarchy], vec![]).into();
    function.get_operation().deref_mut(context).attributes.set(
        Identifier::try_from("test_nested_type").unwrap(),
        TypeAttr::new(nested),
    );

    let error = derive_pliron_ir_structural_identity_v1(context, &function).unwrap_err();
    assert!(matches!(
        error,
        PlironIrIdentityErrorV1::UnsupportedType { .. }
    ));
    assert!(error.to_string().contains("gpu.hierarchy_index"));
}

#[test]
fn ranked_identity_and_repeated_type_lookup_preserve_mutation_epoch() {
    let context = &mut setup();
    let function = empty_function(context, "ranked_identity_kernel", vec![]);
    let entry = function.get_entry_block(context);
    let view_type = RankedViewType::new(context, 32, true, vec![16]).unwrap();
    let view =
        RankedViewOp::new_in_space(context, view_type, vec![], MemorySpaceAttr::Global).unwrap();
    let ret = ReturnOp::new(context);
    append(context, entry, &view);
    append(context, entry, &ret);

    let index_before = context.ir_mutation_attempt_epoch().unwrap().value();
    let _first_index = IndexType::get(context);
    let index_inserted = context.ir_mutation_attempt_epoch().unwrap().value();
    assert!(index_inserted >= index_before);
    let _same_index = IndexType::get(context);
    assert_eq!(
        context.ir_mutation_attempt_epoch().unwrap().value(),
        index_inserted
    );

    let before_identity = context.ir_mutation_attempt_epoch().unwrap().value();
    let first = derive_pliron_ir_structural_identity_v1(context, &function).unwrap();
    let second = derive_pliron_ir_structural_identity_v1(context, &function).unwrap();
    assert!(first.exactly_matches(&second));
    assert_eq!(
        context.ir_mutation_attempt_epoch().unwrap().value(),
        before_identity,
        "canonical ranked-memory identity construction must be read-only"
    );
}
