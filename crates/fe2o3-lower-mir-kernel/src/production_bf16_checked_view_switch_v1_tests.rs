//! Component tests over real verified canonical-owner/inventory constructors.
//! They do not fabricate ProductionPreRankedKirOwnerV1 or claim frontend success.
use super::*;
use fe2o3_kernel_analysis::CanonicalKirSparseLimitsV1;
use fe2o3_kernel_ir::{CanonicalKernelIrWorkBudgetV1 as Work, VerifiedCanonicalKernelIrModuleV12};

fn component_target() -> fe2o3_mir_model::semantic_mir_v1::SemanticTargetDataLayoutV1 {
    use fe2o3_mir_model::semantic_mir_v1::{SemanticLayoutIdentityV1, SemanticTargetDataLayoutV1};
    SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([0x61; 32]))
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum GeometryInput {
    IndexLiteral,
    U64LiteralBitcast,
    U64SparseBitcast,
    U64DynamicBitcast,
    U32ZeroExtend,
}

#[derive(Clone, Copy)]
struct Config {
    geometry_input: GeometryInput,
    length: usize,
    offset: u64,
    rows: u64,
    columns: u64,
    stride: u64,
    variant: u8,
}
impl Config {
    fn plain() -> Self {
        Self {
            geometry_input: GeometryInput::IndexLiteral,
            length: 0,
            offset: 0,
            rows: 16,
            columns: 16,
            stride: 16,
            variant: 0,
        }
    }
}
fn fixture(c: Config) -> (Module, SemanticKirTerminatorOperationSpanV1) {
    let slice = Type::slice(
        Type::Scalar(ScalarType::U16),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    );
    let mut block = BasicBlock::new(BlockId(41));
    let one = |id, ty, kind| Operation::effect_free(ValueDef::new(ValueId(id), ty), kind);
    for (id, value) in [
        (0, c.offset),
        (1, 1),
        (2, c.rows),
        (3, c.columns),
        (4, c.stride),
    ] {
        // The lowerer's coerce_index receives ordinary U64 usize values;
        // its own constant one remains Index. Keep the old fixture unchanged
        // unless this explicit representation-bridge profile is requested.
        if c.geometry_input == GeometryInput::IndexLiteral || id == 1 {
            block.operations.push(one(
                id,
                Type::INDEX,
                OperationKind::Constant(Constant::Index(value)),
            ));
            continue;
        }
        let input = if c.geometry_input == GeometryInput::U64DynamicBitcast {
            ValueId(1003)
        } else {
            let (ty, constant) = if c.geometry_input == GeometryInput::U32ZeroExtend {
                (
                    Type::Scalar(ScalarType::U32),
                    Constant::U32(u32::try_from(value).unwrap()),
                )
            } else {
                (Type::Scalar(ScalarType::U64), Constant::U64(value))
            };
            block
                .operations
                .push(one(80 + id, ty, OperationKind::Constant(constant)));
            if c.geometry_input == GeometryInput::U64SparseBitcast {
                block.operations.push(one(
                    90 + id,
                    Type::Scalar(ScalarType::U64),
                    OperationKind::Constant(Constant::U64(0)),
                ));
                block.operations.push(one(
                    100 + id,
                    Type::Scalar(ScalarType::U64),
                    OperationKind::Binary {
                        op: BinaryOp::Add,
                        lhs: ValueId(80 + id),
                        rhs: ValueId(90 + id),
                    },
                ));
                ValueId(100 + id)
            } else {
                ValueId(80 + id)
            }
        };
        block.operations.push(one(
            id,
            Type::INDEX,
            OperationKind::Cast {
                kind: if c.geometry_input == GeometryInput::U32ZeroExtend {
                    CastKind::ZeroExtend
                } else {
                    CastKind::Bitcast
                },
                value: input,
                to: Type::INDEX,
            },
        ));
    }
    for (value, overflow, op, lhs, rhs) in [
        (5, 6, CheckedBinaryOperator::Subtract, 2, 1),
        (7, 8, CheckedBinaryOperator::Multiply, 5, 4),
        (9, 10, CheckedBinaryOperator::Add, 7, 3),
        (11, 12, CheckedBinaryOperator::Add, 0, 9),
    ] {
        block.operations.push(Operation::checked_binary(
            ValueDef::new(ValueId(value), Type::INDEX),
            ValueDef::new(ValueId(overflow), Type::BOOL),
            op,
            ValueId(lhs),
            ValueId(rhs),
        ));
    }
    if c.variant >= 5 {
        block.operations.push(one(
            19,
            Type::INDEX,
            OperationKind::Constant(Constant::Index(0)),
        ));
        for (id, lhs) in [(20, 2), (21, 3)] {
            block.operations.push(one(
                id,
                Type::BOOL,
                OperationKind::Compare {
                    predicate: ComparePredicate::Equal,
                    lhs: ValueId(lhs),
                    rhs: ValueId(19),
                },
            ));
        }
        block.operations.push(one(
            13,
            Type::BOOL,
            OperationKind::Binary {
                op: BinaryOp::BitOr,
                lhs: ValueId(20),
                rhs: ValueId(21),
            },
        ));
    } else {
        block.operations.push(one(
            13,
            Type::BOOL,
            OperationKind::Constant(Constant::Bool(false)),
        ));
    }
    block.operations.push(one(
        14,
        Type::INDEX,
        if c.variant == 4 {
            OperationKind::Constant(Constant::Index(256))
        } else {
            OperationKind::Select {
                condition: ValueId(13),
                true_value: ValueId(0),
                false_value: ValueId(11),
            }
        },
    ));
    block.operations.push(one(
        15,
        Type::INDEX,
        OperationKind::SliceLength {
            slice: ValueId(1000 + c.length as u32),
        },
    ));
    block.operations.push(one(
        16,
        Type::BOOL,
        OperationKind::Compare {
            predicate: if c.variant == 3 {
                ComparePredicate::LessThan
            } else {
                ComparePredicate::LessThanOrEqual
            },
            lhs: ValueId(14),
            rhs: ValueId(15),
        },
    ));
    if c.variant >= 5 {
        for (id, overflow) in [(22, 6), (23, 8), (24, 10), (25, 12)] {
            block.operations.push(one(
                id,
                Type::BOOL,
                OperationKind::Unary {
                    op: UnaryOp::Not,
                    operand: ValueId(overflow),
                },
            ));
        }
        for (id, lhs, rhs, op) in [
            (26, 22, 23, BinaryOp::BitAnd),
            (27, 26, 24, BinaryOp::BitAnd),
            (28, 27, 25, BinaryOp::BitAnd),
            (29, 13, 28, BinaryOp::BitOr),
        ] {
            block.operations.push(one(
                id,
                Type::BOOL,
                OperationKind::Binary {
                    op,
                    lhs: ValueId(lhs),
                    rhs: ValueId(rhs),
                },
            ));
        }
        block.operations.push(one(
            30,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThanOrEqual,
                lhs: ValueId(3),
                rhs: ValueId(4),
            },
        ));
        block.operations.push(one(
            31,
            Type::BOOL,
            OperationKind::Binary {
                op: BinaryOp::BitOr,
                lhs: ValueId(13),
                rhs: ValueId(30),
            },
        ));
        block.operations.push(one(
            17,
            Type::BOOL,
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: if c.variant == 6 {
                    ValueId(1002)
                } else {
                    ValueId(31)
                },
                rhs: ValueId(29),
            },
        ));
    } else {
        block.operations.push(one(
            17,
            Type::BOOL,
            OperationKind::Constant(Constant::Bool(c.variant != 1)),
        ));
    }
    block.operations.push(one(
        18,
        Type::BOOL,
        OperationKind::Binary {
            op: BinaryOp::BitAnd,
            lhs: if c.variant == 2 {
                ValueId(1002)
            } else {
                ValueId(17)
            },
            rhs: ValueId(16),
        },
    ));
    let count = block.operations.len() as u32;
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("checked-view-component");
    let mut parameters = vec![slice.clone(), slice, Type::BOOL];
    let mut arguments = vec![ValueId(1000), ValueId(1001), ValueId(1002)];
    if c.geometry_input == GeometryInput::U64DynamicBitcast {
        parameters.push(Type::Scalar(ScalarType::U64));
        arguments.push(ValueId(1003));
    }
    module.functions.push(Function::kernel_entry(
        "checked_view",
        Signature::new(parameters, vec![]),
        arguments,
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "checked_view",
        "checked_view",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    ));
    (
        module,
        SemanticKirTerminatorOperationSpanV1 {
            correspondence_owner: SemanticFunctionIdV1::from_index(3),
            semantic_function: SemanticFunctionIdV1::from_index(3),
            semantic_block: SemanticBlockIdV1::from_index(7),
            kernel_ir_block: BlockId(41),
            first_operation_ordinal: 0,
            operation_count: count,
        },
    )
}
struct Outcome {
    result: QueryResult<(ValueId, u64)>,
    work: usize,
    denied: bool,
}
fn run(c: Config, work_limit: usize, storage_limit: usize) -> Outcome {
    let (module, span) = fixture(c);
    let mut work = Work::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    let (owner, owner_storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &module,
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(owner_storage.retained_storage())
        .unwrap();
    let (inventory, inventory_storage) =
        CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let (report, report_storage) = CanonicalKirSparseV1::derive(
        &inventory,
        CanonicalKirSparseLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(report_storage.retained_storage())
        .unwrap();
    let before = budget.work();
    assert_eq!(
        report.retained_storage_v1(&mut budget).unwrap(),
        report_storage.retained_storage()
    );
    assert_eq!(budget.work(), before + 1);
    let graph = Graph {
        inventory: &inventory,
        caller: &inventory.functions()[0],
        target: component_target(),
    };
    let result = bf16_call_query_scope_v1(&mut budget, |b| {
        condition(&graph, &report, ValueId(18), &span, b)
    });
    let outcome = Outcome {
        result,
        work: budget.work(),
        denied: budget.failed_work().is_some() || budget.failed_storage().is_some(),
    };
    drop(report);
    budget
        .release_storage(report_storage.retained_storage())
        .unwrap();
    drop(inventory);
    budget
        .release_storage(inventory_storage.retained_storage())
        .unwrap();
    drop(owner);
    budget
        .release_storage(owner_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), 0);
    outcome
}
const LIMIT: usize = 10_000_000;

#[test]
fn exact_checked_geometry_uses_both_distinct_original_input_lengths() {
    for length in [0, 1] {
        let c = Config {
            length,
            ..Config::plain()
        };
        let out = run(c, LIMIT, LIMIT);
        assert_eq!(out.result.unwrap(), (ValueId(1000 + length as u32), 256));
        assert!(!out.denied);
    }
}
#[test]
fn changed_offset_shape_unknown_unsafe_and_noncanonical_conditions_refuse() {
    let cases = [
        Config {
            offset: 1,
            ..Config::plain()
        },
        Config {
            rows: 8,
            columns: 32,
            stride: 32,
            ..Config::plain()
        }, // Still 256: geometry matters.
        Config {
            rows: u64::MAX,
            stride: u64::MAX,
            ..Config::plain()
        },
        Config {
            variant: 1,
            ..Config::plain()
        },
        Config {
            variant: 2,
            ..Config::plain()
        },
        Config {
            variant: 3,
            ..Config::plain()
        },
        Config {
            variant: 4,
            ..Config::plain()
        },
    ];
    for c in cases {
        assert!(matches!(
            run(c, LIMIT, LIMIT).result,
            Err(Bf16NominalCallQueryErrorV1::Unavailable(_))
        ));
    }
}
#[test]
fn original_query_work_exact_and_one_short_is_sticky() {
    for c in [
        Config::plain(),
        Config {
            variant: 5,
            ..Config::plain()
        },
    ] {
        let measured = run(c, LIMIT, LIMIT);
        assert!(measured.result.is_ok());
        assert!(run(c, measured.work, LIMIT).result.is_ok());
        let denied = run(c, measured.work - 1, LIMIT);
        assert!(denied.result.is_err() && denied.denied);
    }
}
#[test]
fn query_scope_storage_exact_and_one_short_is_sticky() {
    // Isolate the real scope's reservation boundary. Do not assume the whole
    // fixture's historical initialization peak is the scope's required peak.
    const FLOOR: usize = 37;
    let scratch = bf16_call_query_scratch_v1::<(ValueId, u64)>().unwrap();
    for (limit, accepted) in [(FLOOR + scratch, true), (FLOOR + scratch - 1, false)] {
        let mut work = Work::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, limit);
        budget.reserve_storage(FLOOR).unwrap();
        let mut entered = false;
        let result = bf16_call_query_scope_v1(&mut budget, |_| {
            entered = true;
            Ok((ValueId(1000), 256u64))
        });
        assert_eq!(result.is_ok(), accepted);
        assert_eq!(entered, accepted);
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.failed_storage().is_some(), !accepted);
    }
}
#[test]
fn graph_owner_identity_is_not_equal_serialized_content() {
    let (module, _) = fixture(Config::plain());
    let mut work = Work::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    let (owner, os) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &module,
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(os.retained_storage()).unwrap();
    let (first, fs) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(fs.retained_storage()).unwrap();
    let (second, ss) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(ss.retained_storage()).unwrap();
    let (report, rs) =
        CanonicalKirSparseV1::derive(&first, CanonicalKirSparseLimitsV1::default(), &mut budget)
            .unwrap();
    budget.reserve_storage(rs.retained_storage()).unwrap();
    assert!(report.belongs_to(&first));
    assert!(!report.belongs_to(&second));
    drop(report);
    budget.release_storage(rs.retained_storage()).unwrap();
    drop(second);
    budget.release_storage(ss.retained_storage()).unwrap();
    drop(first);
    budget.release_storage(fs.retained_storage()).unwrap();
    drop(owner);
    budget.release_storage(os.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
}
#[test]
fn selector_walk_refuses_bad_definition_and_span_without_unmetered_fallback() {
    let (module, mut span) = fixture(Config::plain());
    let mut work = Work::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    let (owner, os) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &module,
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(os.retained_storage()).unwrap();
    let (inventory, is) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(is.retained_storage()).unwrap();
    let graph = Graph {
        inventory: &inventory,
        caller: &inventory.functions()[0],
        target: component_target(),
    };
    assert!(definition(&graph, ValueId(u32::MAX), &mut budget).is_err());
    let index = definition(&graph, ValueId(18), &mut budget).unwrap();
    let (_, coordinate) = operation(&graph, index, &mut budget).unwrap();
    span.kernel_ir_block = BlockId(99);
    assert!(in_span(coordinate, &span, &graph, &mut budget).is_err());
    span.kernel_ir_block = BlockId(41);
    span.operation_count = 1;
    assert!(in_span(coordinate, &span, &graph, &mut budget).is_err());
    drop(inventory);
    budget.release_storage(is.retained_storage()).unwrap();
    drop(owner);
    budget.release_storage(os.retained_storage()).unwrap();
}
#[test]
fn view_loan_api_cannot_return_a_borrowed_or_forged_owner() {
    fn signature<'w>(
        owner: &ProductionPreRankedKirOwnerV1,
        inventory: &CanonicalKirInventoryV1<'_>,
        report: &CanonicalKirSparseV1<'_, '_>,
        root: SemanticFunctionIdV1,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        budget: &mut ArgumentBudgetV1<'w>,
    ) -> QueryResult<Option<(u32, u32, u32, u64)>> {
        owner.with_checked_bf16_view_switch_v1(
            inventory,
            report,
            root,
            block,
            call,
            block,
            budget,
            |view, _| {
                Ok((
                    view.parameter(),
                    view.success().index(),
                    view.failure().index(),
                    view.required(),
                ))
            },
        )
    }
    let _ = signature;
}

fn variants(ok: u128, err: u128) -> Vec<SemanticEnumVariantV1> {
    use fe2o3_mir_model::semantic_mir_v1::SemanticAggregateTypeV1;
    vec![
        SemanticEnumVariantV1::new(
            ok,
            SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(7)]).unwrap(),
        ),
        SemanticEnumVariantV1::new(
            err,
            SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(9)]).unwrap(),
        ),
    ]
}
#[test]
fn result_payload_discriminants_are_not_boolean_or_variant_ordinals() {
    for (ok, err) in [(0, 1), (1, 0), (7, 42)] {
        for reversed in [false, true] {
            let mut rows = variants(ok, err);
            if reversed {
                rows.reverse()
            }
            let mut work = Work::new(LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            assert_eq!(
                result_discriminants(
                    &rows,
                    SemanticTypeIdV1::from_index(7),
                    SemanticTypeIdV1::from_index(9),
                    &mut budget
                )
                .unwrap(),
                (ok, err)
            );
        }
    }
}
#[test]
fn missing_duplicate_uninhabited_or_aliased_result_alternatives_refuse() {
    use fe2o3_mir_model::semantic_mir_v1::SemanticAggregateTypeV1;
    for mutation in 0..6 {
        let mut rows = variants(0, 1);
        match mutation {
            0 => {
                rows.pop();
            }
            1 => rows[1] = rows[0].clone(),
            2 => rows = variants(0, 0),
            3 => {
                rows[1] = SemanticEnumVariantV1::new(
                    1,
                    SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(10)]).unwrap(),
                )
            }
            4 => {
                rows[1] = SemanticEnumVariantV1::new_with_inhabitedness(
                    1,
                    SemanticAggregateTypeV1::new(vec![SemanticTypeIdV1::from_index(9)]).unwrap(),
                    true,
                )
            }
            5 => rows.push(rows[1].clone()),
            _ => unreachable!(),
        }
        let mut work = Work::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        assert!(result_discriminants(
            &rows,
            SemanticTypeIdV1::from_index(7),
            SemanticTypeIdV1::from_index(9),
            &mut budget
        )
        .is_err());
    }
    let mut work = Work::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, 0);
    assert!(result_discriminants(
        &variants(0, 1),
        SemanticTypeIdV1::from_index(7),
        SemanticTypeIdV1::from_index(7),
        &mut budget
    )
    .is_err());
}
#[test]
fn actual_explicit_and_default_edges_preserve_ok_zero_and_reversed_discriminants() {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticControlFlowEdgeV1 as Edge, SemanticSwitchTargetV1 as Target,
        SemanticSwitchTargetsV1 as Targets,
    };
    for (ok, err) in [(0, 1), (1, 0), (7, 42)] {
        for explicit_ok in [false, true] {
            let (value, explicit, otherwise) = if explicit_ok {
                (ok, 71, 93)
            } else {
                (err, 93, 71)
            };
            let targets = Targets::new(
                vec![Target::new(
                    value,
                    Edge::new(
                        SemanticEdgeRoleV1::SwitchValue,
                        SemanticBlockIdV1::from_index(explicit),
                    ),
                )],
                Edge::new(
                    SemanticEdgeRoleV1::SwitchOtherwise,
                    SemanticBlockIdV1::from_index(otherwise),
                ),
            )
            .unwrap();
            let mut work = Work::new(LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, 0);
            assert_eq!(
                source_target(&targets, ok, &mut budget).unwrap().index(),
                71
            );
            assert_eq!(
                source_target(&targets, err, &mut budget).unwrap().index(),
                93
            );
        }
    }
}
#[test]
fn source_case_roles_and_default_roles_are_checked_without_guessing() {
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticControlFlowEdgeV1 as Edge, SemanticSwitchTargetV1 as Target,
        SemanticSwitchTargetsV1 as Targets,
    };
    for wrong_default in [false, true] {
        let targets = Targets::new(
            vec![Target::new(
                0,
                Edge::new(
                    if wrong_default {
                        SemanticEdgeRoleV1::SwitchValue
                    } else {
                        SemanticEdgeRoleV1::Goto
                    },
                    SemanticBlockIdV1::from_index(71),
                ),
            )],
            Edge::new(
                if wrong_default {
                    SemanticEdgeRoleV1::Goto
                } else {
                    SemanticEdgeRoleV1::SwitchOtherwise
                },
                SemanticBlockIdV1::from_index(93),
            ),
        )
        .unwrap();
        let mut work = Work::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        assert!(source_target(&targets, 0, &mut budget).is_err());
    }
}
#[test]
fn swallowed_denial_or_panic_cannot_publish_a_checked_query_result() {
    let scratch = bf16_call_query_scratch_v1::<()>().unwrap();
    let mut work = Work::new(BF16_CALL_QUERY_ENTRY_WORK_V1);
    let mut budget = ArgumentBudgetV1::new(&mut work, scratch + 37);
    budget.reserve_storage(37).unwrap();
    assert!(matches!(
        bf16_call_query_scope_v1(&mut budget, |b| {
            assert!(b.charge_work(1).is_err());
            Ok(())
        }),
        Err(Bf16NominalCallQueryErrorV1::Resource(
            ArgumentResourceV1::Accounting
        ))
    ));
    assert_eq!(budget.storage(), 37);
    let mut work = Work::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, scratch + 37);
    budget.reserve_storage(37).unwrap();
    let result: QueryResult<()> =
        bf16_call_query_scope_v1(&mut budget, |_| panic!("component-only callback panic"));
    assert!(matches!(
        result,
        Err(Bf16NominalCallQueryErrorV1::CallbackPanicked)
    ));
    assert_eq!(budget.storage(), 37);
}
fn forwarding_module(depth: u32, duplicate_edge: bool) -> Module {
    let mut blocks = Vec::new();
    let mut entry = BasicBlock::new(BlockId(41));
    entry.terminator = Some(if duplicate_edge {
        Terminator::ConditionalBranch {
            condition: ValueId(101),
            then_target: BlockId(1000),
            then_arguments: vec![ValueId(100)],
            else_target: BlockId(1000),
            else_arguments: vec![ValueId(100)],
        }
    } else {
        Terminator::Branch {
            target: BlockId(1000),
            arguments: vec![ValueId(100)],
        }
    });
    blocks.push(entry);
    for ordinal in 0..depth {
        let mut block = BasicBlock::new(BlockId(1000 + ordinal));
        block
            .parameters
            .push(ValueDef::new(ValueId(200 + ordinal), Type::INDEX));
        block.terminator = Some(if ordinal + 1 == depth {
            Terminator::Return { values: vec![] }
        } else {
            Terminator::Branch {
                target: BlockId(1001 + ordinal),
                arguments: vec![ValueId(200 + ordinal)],
            }
        });
        blocks.push(block);
    }
    let mut module = Module::new("checked-view-forwarding-component");
    module.functions.push(Function::kernel_entry(
        "forwarding",
        Signature::new(vec![Type::INDEX, Type::BOOL], vec![]),
        vec![ValueId(100), ValueId(101)],
        blocks,
    ));
    module.kernels.push(Kernel::new(
        "forwarding",
        "forwarding",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    module
}
#[test]
fn forwarding_requires_single_occurrences_and_a_bounded_acyclic_chain() {
    for (depth, duplicate, accept) in [
        (1, false, true),
        (31, false, true),
        (32, false, false),
        (1, true, false),
    ] {
        let module = forwarding_module(depth, duplicate);
        let mut work = Work::new(LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
        let (owner, os) =
            VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
                &module,
                &mut budget,
            )
            .unwrap();
        budget.reserve_storage(os.retained_storage()).unwrap();
        let (inventory, is) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
        budget.reserve_storage(is.retained_storage()).unwrap();
        let graph = Graph {
            inventory: &inventory,
            caller: &inventory.functions()[0],
            target: component_target(),
        };
        let result = definition(&graph, ValueId(200 + depth - 1), &mut budget);
        assert_eq!(result.is_ok(), accept);
        if let Ok(index) = result {
            assert!(matches!(
                inventory.definitions()[index].coordinate,
                Definition::FunctionArgument { argument: 0, .. }
            ));
        }
        drop(inventory);
        budget.release_storage(is.retained_storage()).unwrap();
        drop(owner);
        budget.release_storage(os.retained_storage()).unwrap();
        assert_eq!(budget.storage(), 0);
    }
}

#[test]
fn exact_real_safety_dag_folds_locally_and_keeps_actual_length_dynamic() {
    for length in [0, 1] {
        let c = Config {
            length,
            variant: 5,
            ..Config::plain()
        };
        assert_eq!(
            run(c, LIMIT, LIMIT).result.unwrap(),
            (ValueId(1000 + length as u32), 256)
        );
    }
    for c in [
        Config {
            variant: 6,
            ..Config::plain()
        }, // Unknown original safety operand.
        Config {
            variant: 5,
            rows: 0,
            ..Config::plain()
        },
        Config {
            variant: 5,
            stride: 8,
            ..Config::plain()
        },
        Config {
            variant: 5,
            offset: 1,
            ..Config::plain()
        },
        Config {
            variant: 5,
            rows: u64::MAX,
            stride: u64::MAX,
            ..Config::plain()
        },
    ] {
        assert!(matches!(
            run(c, LIMIT, LIMIT).result,
            Err(Bf16NominalCallQueryErrorV1::Unavailable(_))
        ));
    }
}

fn with_exact_component(
    c: Config,
    extend_facts: bool,
    inspect: impl FnOnce(
        &Graph<'_, '_>,
        &CanonicalKirSparseV1<'_, '_>,
        &SemanticKirTerminatorOperationSpanV1,
        &mut ArgumentBudgetV1<'_>,
    ),
) {
    let (mut module, mut span) = fixture(c);
    if extend_facts {
        let body = module.functions[0].body.as_mut().unwrap();
        for ordinal in 0..65u32 {
            body.blocks[0].operations.push(Operation::effect_free(
                ValueDef::new(ValueId(2000 + ordinal), Type::BOOL),
                OperationKind::Unary {
                    op: UnaryOp::Not,
                    operand: if ordinal == 0 {
                        ValueId(13)
                    } else {
                        ValueId(1999 + ordinal)
                    },
                },
            ));
        }
        span.operation_count = body.blocks[0].operations.len() as u32;
    }
    let mut work = Work::new(LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, LIMIT);
    let (owner, os) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &module,
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(os.retained_storage()).unwrap();
    let (inventory, is) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(is.retained_storage()).unwrap();
    let (report, rs) = CanonicalKirSparseV1::derive(
        &inventory,
        CanonicalKirSparseLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(rs.retained_storage()).unwrap();
    let floor = budget.storage();
    let graph = Graph {
        inventory: &inventory,
        caller: &inventory.functions()[0],
        target: component_target(),
    };
    inspect(&graph, &report, &span, &mut budget);
    assert_eq!(budget.storage(), floor);
    drop(report);
    budget.release_storage(rs.retained_storage()).unwrap();
    drop(inventory);
    budget.release_storage(is.retained_storage()).unwrap();
    drop(owner);
    budget.release_storage(os.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn exact_target_index_value_and_overflow_ordinals_do_not_change_sparse_report() {
    for (rows, stride) in [(16, 16), (0, 16), (u64::MAX, u64::MAX)] {
        with_exact_component(
            Config {
                rows,
                stride,
                variant: 5,
                ..Config::plain()
            },
            false,
            |graph, report, span, budget| {
                let (sub, sub_overflow) = rows.overflowing_sub(1);
                let (mul, mul_overflow) = sub.overflowing_mul(stride);
                for (id, ty, expected) in [
                    (5, Type::INDEX, u128::from(sub)),
                    (6, Type::BOOL, u128::from(sub_overflow)),
                    (7, Type::INDEX, u128::from(mul)),
                    (8, Type::BOOL, u128::from(mul_overflow)),
                ] {
                    let index = definition(graph, ValueId(id), budget).unwrap();
                    assert_eq!(
                        report.value(index),
                        Some(CanonicalKirSparseValueV1::Dynamic)
                    );
                    assert_eq!(
                        exact_constant(graph, report, ValueId(id), &ty, span, budget).unwrap(),
                        Some(expected)
                    );
                    assert_eq!(
                        report.value(index),
                        Some(CanonicalKirSparseValueV1::Dynamic)
                    );
                }
                assert_eq!(
                    exact_constant(graph, report, ValueId(15), &Type::INDEX, span, budget).unwrap(),
                    None
                );
                assert_eq!(
                    exact_constant(graph, report, ValueId(16), &Type::BOOL, span, budget).unwrap(),
                    None
                );
            },
        );
    }
}

#[test]
fn exact_folding_is_span_bounded_and_unknowns_do_not_become_true() {
    with_exact_component(
        Config {
            variant: 6,
            ..Config::plain()
        },
        false,
        |graph, report, span, budget| {
            assert_eq!(
                exact_constant(graph, report, ValueId(17), &Type::BOOL, span, budget).unwrap(),
                None
            );
            let mut wrong = *span;
            wrong.first_operation_ordinal = 0;
            wrong.operation_count = 1;
            assert!(
                exact_constant(graph, report, ValueId(8), &Type::BOOL, &wrong, budget).is_err()
            );
        },
    );
}

#[test]
fn exact_usize_bridge_uses_sparse_u64_proof_without_changing_sparse_index() {
    for geometry_input in [
        GeometryInput::U64LiteralBitcast,
        GeometryInput::U64SparseBitcast,
    ] {
        for length in [0, 1] {
            with_exact_component(
                Config {
                    geometry_input,
                    length,
                    variant: 5,
                    ..Config::plain()
                },
                false,
                |graph, report, span, budget| {
                    for (id, expected) in [(0, 0u128), (2, 16), (3, 16), (4, 16)] {
                        let input = if geometry_input == GeometryInput::U64SparseBitcast {
                            ValueId(100 + id)
                        } else {
                            ValueId(80 + id)
                        };
                        let input_index = definition(graph, input, budget).unwrap();
                        assert!(matches!(
                            report.value(input_index),
                            Some(CanonicalKirSparseValueV1::Constant(value))
                                if value.ty() == ScalarType::U64 && value.bits() == expected
                        ));
                        let index = definition(graph, ValueId(id), budget).unwrap();
                        assert_eq!(
                            report.value(index),
                            Some(CanonicalKirSparseValueV1::Dynamic)
                        );
                        assert_eq!(
                            exact_constant(graph, report, ValueId(id), &Type::INDEX, span, budget)
                                .unwrap(),
                            Some(expected)
                        );
                        assert_eq!(
                            report.value(index),
                            Some(CanonicalKirSparseValueV1::Dynamic)
                        );
                    }
                    assert_eq!(
                        condition(graph, report, ValueId(18), span, budget).unwrap(),
                        (ValueId(1000 + length as u32), 256)
                    );
                    for (id, ty) in [(15, Type::INDEX), (16, Type::BOOL), (18, Type::BOOL)] {
                        assert_eq!(
                            exact_constant(graph, report, ValueId(id), &ty, span, budget).unwrap(),
                            None
                        );
                    }
                },
            );
        }
    }
}

#[test]
fn exact_usize_bridge_retains_full_u64_bits_and_overflow_refusal() {
    for rows in [0, 16, u64::MAX] {
        with_exact_component(
            Config {
                geometry_input: GeometryInput::U64LiteralBitcast,
                rows,
                variant: 5,
                ..Config::plain()
            },
            false,
            |graph, report, span, budget| {
                assert_eq!(
                    exact_constant(graph, report, ValueId(2), &Type::INDEX, span, budget).unwrap(),
                    Some(u128::from(rows))
                );
            },
        );
    }
    for config in [
        Config {
            rows: 0,
            ..Config::plain()
        },
        Config {
            stride: 8,
            ..Config::plain()
        },
        Config {
            offset: 1,
            ..Config::plain()
        },
        Config {
            rows: 8,
            columns: 32,
            stride: 32,
            ..Config::plain()
        },
        Config {
            rows: u64::MAX,
            stride: u64::MAX,
            ..Config::plain()
        },
    ] {
        assert!(matches!(
            run(
                Config {
                    geometry_input: GeometryInput::U64LiteralBitcast,
                    variant: 5,
                    ..config
                },
                LIMIT,
                LIMIT
            )
            .result,
            Err(Bf16NominalCallQueryErrorV1::Unavailable(_))
        ));
    }
}

#[test]
fn exact_usize_bridge_does_not_admit_dynamic_u64_or_other_index_casts() {
    // Both are real canonical-owner inputs. U32 -> Index ZeroExtend is a
    // legal hardware-index promotion, but is outside this closed bridge rule.
    for geometry_input in [
        GeometryInput::U64DynamicBitcast,
        GeometryInput::U32ZeroExtend,
    ] {
        with_exact_component(
            Config {
                geometry_input,
                variant: 5,
                ..Config::plain()
            },
            false,
            |graph, report, span, budget| {
                assert_eq!(
                    exact_constant(graph, report, ValueId(2), &Type::INDEX, span, budget).unwrap(),
                    None
                );
                assert!(matches!(
                    condition(graph, report, ValueId(18), span, budget),
                    Err(Bf16NominalCallQueryErrorV1::Unavailable(
                        "checked-view safety conjunction is not proved true"
                    ))
                ));
            },
        );
    }
}

#[test]
fn exact_usize_bridge_is_span_bound_and_does_not_extend_output_types() {
    with_exact_component(
        Config {
            geometry_input: GeometryInput::U64LiteralBitcast,
            variant: 5,
            ..Config::plain()
        },
        false,
        |graph, report, span, budget| {
            let index = definition(graph, ValueId(2), budget).unwrap();
            let (_, at) = operation(graph, index, budget).unwrap();
            let mut wrong = *span;
            wrong.operation_count = at.operation;
            assert!(matches!(
                exact_constant(graph, report, ValueId(2), &Type::INDEX, &wrong, budget),
                Err(Bf16NominalCallQueryErrorV1::Unavailable(
                    "checked-view expression leaves its original source span"
                ))
            ));
            assert!(matches!(
                exact_constant(
                    graph,
                    report,
                    ValueId(82),
                    &Type::Scalar(ScalarType::U64),
                    span,
                    budget
                ),
                Err(Bf16NominalCallQueryErrorV1::Unavailable(
                    "checked-view exact constant type is outside the closed profile"
                ))
            ));
        },
    );
}

#[test]
fn exact_usize_bridge_work_exact_and_one_short_remain_typed_and_sticky() {
    for geometry_input in [
        GeometryInput::U64LiteralBitcast,
        GeometryInput::U64SparseBitcast,
    ] {
        let config = Config {
            geometry_input,
            variant: 5,
            ..Config::plain()
        };
        let measured = run(config, LIMIT, LIMIT);
        assert!(measured.result.is_ok() && !measured.denied);
        let exact = run(config, measured.work, LIMIT);
        assert!(exact.result.is_ok() && !exact.denied);
        assert_eq!(exact.work, measured.work);
        let short = run(config, measured.work - 1, LIMIT);
        assert!(matches!(
            short.result,
            Err(Bf16NominalCallQueryErrorV1::Resource(
                ArgumentResourceV1::Work(_)
            ))
        ));
        assert!(short.denied);
        assert!(short.work < measured.work);
    }
}

#[test]
fn exact_fact_table_refuses_capacity_exhaustion_without_fallback() {
    with_exact_component(
        Config {
            variant: 5,
            ..Config::plain()
        },
        true,
        |graph, report, span, budget| {
            assert!(matches!(
                exact_constant(graph, report, ValueId(2064), &Type::BOOL, span, budget),
                Err(Bf16NominalCallQueryErrorV1::Unavailable(
                    "checked-view exact constant fact limit"
                ))
            ));
        },
    );
}
