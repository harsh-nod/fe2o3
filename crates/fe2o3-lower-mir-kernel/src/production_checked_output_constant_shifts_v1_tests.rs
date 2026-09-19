use super::*;
use crate::production_semantic_kir_v1::helper_source_fixture_v1 as fixture;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1, Function, Kernel, LaunchDomain,
    LaunchExtent, Module, Operation, Signature, Terminator, ValueDef, ValueId,
    VerifiedCanonicalKernelIrModuleV12,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticLayoutIdentityV1, SemanticTargetDataLayoutV1};

pub(crate) const FIXED: [ScalarType; 8] = [
    ScalarType::I8,
    ScalarType::U8,
    ScalarType::I16,
    ScalarType::U16,
    ScalarType::I32,
    ScalarType::U32,
    ScalarType::I64,
    ScalarType::U64,
];

fn constant(ty: ScalarType, bits: u64) -> Constant {
    match ty {
        ScalarType::I8 => Constant::I8(bits as i8),
        ScalarType::U8 => Constant::U8(bits as u8),
        ScalarType::I16 => Constant::I16(bits as i16),
        ScalarType::U16 => Constant::U16(bits as u16),
        ScalarType::I32 => Constant::I32(bits as i32),
        ScalarType::U32 => Constant::U32(bits as u32),
        ScalarType::I64 => Constant::I64(bits as i64),
        ScalarType::U64 => Constant::U64(bits),
        ScalarType::Index => Constant::Index(bits),
        _ => panic!("integer fixture"),
    }
}

pub(crate) fn candidate(
    scalar: ScalarType,
    op: BinaryOp,
    count: Option<u64>,
    mask: Option<u64>,
    helper: bool,
) -> Module {
    let ty = Type::Scalar(scalar);
    let mut body = BasicBlock::new(BlockId(17));
    let mut rhs = ValueId(1);
    if let Some(count) = count {
        body.operations.push(Operation::new(
            vec![ValueDef::new(ValueId(2), ty.clone())],
            OperationKind::Constant(constant(scalar, count)),
        ));
        rhs = ValueId(2);
    }
    if let Some(mask) = mask {
        body.operations.push(Operation::new(
            vec![ValueDef::new(ValueId(3), ty.clone())],
            OperationKind::Constant(constant(scalar, mask)),
        ));
        body.operations.push(Operation::new(
            vec![ValueDef::new(ValueId(4), ty.clone())],
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: rhs,
                rhs: ValueId(3),
            },
        ));
        rhs = ValueId(4);
    }
    body.operations.push(Operation::new(
        vec![ValueDef::new(ValueId(5), ty.clone())],
        OperationKind::Binary {
            op,
            lhs: ValueId(0),
            rhs,
        },
    ));
    body.terminator = Some(Terminator::Return {
        values: if helper { vec![ValueId(5)] } else { vec![] },
    });
    let mut module = Module::new("constant-shift-census");
    if helper {
        module.functions.push(Function::internal_helper(
            "shift_value",
            Signature::new(vec![ty.clone(), ty.clone()], vec![ty.clone()]),
            vec![ValueId(0), ValueId(1)],
            vec![body],
        ));
        let mut entry = BasicBlock::new(BlockId(41));
        entry.operations.push(Operation::new(
            vec![ValueDef::new(ValueId(2), ty.clone())],
            OperationKind::Call {
                callee: "shift_value".into(),
                arguments: vec![ValueId(0), ValueId(1)],
            },
        ));
        entry.terminator = Some(Terminator::Return { values: vec![] });
        module.functions.push(Function::kernel_entry(
            "root",
            Signature::new(vec![ty.clone(), ty], vec![]),
            vec![ValueId(0), ValueId(1)],
            vec![entry],
        ));
    } else {
        module.functions.push(Function::kernel_entry(
            "root",
            Signature::new(vec![ty.clone(), ty], vec![]),
            vec![ValueId(0), ValueId(1)],
            vec![body],
        ));
    }
    module.kernels.push(Kernel::new(
        "root",
        "root",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    module
}

pub(crate) fn inspect(
    candidate: &Module,
    action: impl FnOnce(&CanonicalKirInventoryV1<'_>, usize, &mut AssertOriginBudgetV1<'_>),
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut budget = AssertOriginBudgetV1::new(&mut work, 32 << 20);
    budget.reserve_storage(19).unwrap();
    let (owner, storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            candidate,
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let (inventory, scratch) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(scratch.retained_storage()).unwrap();
    let floor = budget.storage();
    let ordinal = inventory
        .operations()
        .iter()
        .position(|row| {
            matches!(
                row.operation.kind,
                OperationKind::Binary {
                    op: BinaryOp::ShiftLeft | BinaryOp::ShiftRight,
                    ..
                }
            )
        })
        .unwrap();
    action(&inventory, ordinal, &mut budget);
    budget.release_storage(budget.storage() - floor).unwrap();
    drop(inventory);
    budget.release_storage(scratch.retained_storage()).unwrap();
    drop(owner);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 19);
}

#[test]
fn fixed_literal_shifts_pass_actual_root_and_helper_censuses() {
    for scalar in FIXED {
        let width = u64::from(scalar.bit_width().unwrap());
        for op in [BinaryOp::ShiftLeft, BinaryOp::ShiftRight] {
            for count in [0, 3, width - 1] {
                for mask in [None, Some(width - 1)] {
                    for helper in [false, true] {
                        inspect(
                            &candidate(scalar, op, Some(count), mask, helper),
                            |inventory, ordinal, budget| {
                                assert!(native(inventory, ordinal, budget).unwrap());
                                let private =
                                    private_memory::check(inventory, 1024, budget).unwrap();
                                let division = unsigned_division::check(
                                    inventory,
                                    SemanticTargetDataLayoutV1::gfx942(
                                        SemanticLayoutIdentityV1::from_sha256([91; 32]),
                                    ),
                                    budget,
                                )
                                .unwrap();
                                let helpers = scalar_helpers::check(inventory, budget).unwrap();
                                census::native(
                                    inventory,
                                    &private,
                                    &division,
                                    &helpers,
                                    "constant shift test",
                                    |_, _| Ok(false),
                                    budget,
                                )
                                .unwrap();
                            },
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn native_mask_does_not_authorize_dynamic_negative_or_excess_counts() {
    for scalar in FIXED {
        let width = u64::from(scalar.bit_width().unwrap());
        for op in [BinaryOp::ShiftLeft, BinaryOp::ShiftRight] {
            for (count, mask) in [
                (None, None),
                (None, Some(width - 1)),
                (Some(width), None),
                (Some(width), Some(width - 1)),
                (Some(width + 1), Some(width - 1)),
                (Some(3), Some(width - 2)),
                (Some(u64::MAX), Some(width - 1)),
            ] {
                inspect(
                    &candidate(scalar, op, count, mask, true),
                    |inventory, ordinal, budget| {
                        assert!(!native(inventory, ordinal, budget).unwrap());
                        if mask == Some(width - 1) {
                            // The new actual-mask census proves native totality,
                            // never source validity. Genuine raw-source refusal
                            // is independently checked below after materialization.
                            scalar_helpers::check(inventory, budget).unwrap();
                            return;
                        }
                        assert!(matches!(
                            scalar_helpers::check(inventory, budget),
                            Err(E::Unsupported {
                                phase: "scalar helpers",
                                detail: "closed scalar helper opcode"
                            })
                        ));
                    },
                );
            }
        }
    }
}

#[test]
fn index_and_128_bit_shifts_remain_outside_the_new_domain() {
    for scalar in [ScalarType::Index, ScalarType::I128, ScalarType::U128] {
        inspect(
            &candidate(
                scalar,
                BinaryOp::ShiftRight,
                if scalar == ScalarType::Index {
                    Some(3)
                } else {
                    None
                },
                None,
                false,
            ),
            |inventory, ordinal, budget| {
                assert!(!native(inventory, ordinal, budget).unwrap());
            },
        );
    }
    inspect(
        &candidate(
            ScalarType::Index,
            BinaryOp::ShiftRight,
            Some(3),
            Some(63),
            true,
        ),
        |inventory, ordinal, budget| {
            assert!(!native(inventory, ordinal, budget).unwrap());
            assert!(matches!(
                scalar_helpers::check(inventory, budget),
                Err(E::Unsupported {
                    phase: "scalar helpers",
                    detail: "closed scalar helper opcode"
                })
            ));
        },
    );
}

#[test]
fn fixed_census_cost_has_no_storage_and_refuses_before_one_short_work() {
    inspect(
        &candidate(
            ScalarType::U64,
            BinaryOp::ShiftRight,
            Some(3),
            Some(63),
            false,
        ),
        |inventory, ordinal, outer| {
            for available in [0, 63, 64] {
                let floor = outer.storage();
                let mut work = CanonicalKernelIrWorkBudgetV1::new(available);
                let mut budget = AssertOriginBudgetV1::new(&mut work, floor);
                budget.reserve_storage(floor).unwrap();
                let result = native(inventory, ordinal, &mut budget);
                assert_eq!(budget.storage(), floor);
                if available == 64 {
                    assert!(result.unwrap());
                    assert_eq!(budget.work(), 64);
                } else {
                    assert!(matches!(
                        result,
                        Err(E::Resource(AssertOriginResourceV1::Work(_)))
                    ));
                }
            }
            assert!(!native(inventory, usize::MAX, outer).unwrap());
        },
    );
}

#[test]
fn genuinely_admitted_source_requires_a_literal_before_any_materializer_mask() {
    use fe2o3_mir_model::semantic_mir_v1::*;
    for operation in [
        SemanticBinaryOpV1::ShiftLeft,
        SemanticBinaryOpV1::ShiftRight,
    ] {
        for count in [
            None,
            Some(0_u32),
            Some(3),
            Some(31),
            Some(32),
            Some(33),
            Some(u32::MAX),
        ] {
            let right = count.map_or_else(
                || fixture::copy(2),
                |count| {
                    SemanticOperandV1::Constant(SemanticConstantV1::new(
                        fixture::WORD,
                        SemanticConstantValueV1::Scalar(
                            SemanticScalarValueV1::new(count.into(), 4).unwrap(),
                        ),
                    ))
                },
            );
            let input = fixture::source(vec![fixture::function(
                60,
                false,
                0,
                vec![fixture::block(
                    61,
                    vec![fixture::assign(
                        0,
                        SemanticRvalueKindV1::Binary {
                            operation,
                            left: fixture::copy(1),
                            right,
                        },
                    )],
                    SemanticTerminatorKindV1::Return,
                )],
            )]);
            let SemanticStatementKindV1::Assign(assignment) =
                input.functions()[1].blocks()[0].statements()[0].kind()
            else {
                panic!("source assignment")
            };
            for work_limit in [15, 16] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
                let mut budget = AssertOriginBudgetV1::new(&mut work, 19);
                budget.reserve_storage(19).unwrap();
                let result = source(&input, assignment.value(), &mut budget);
                if work_limit == 16 {
                    assert_eq!(result.unwrap(), count.is_some_and(|n| n < 32));
                    assert_eq!(budget.work(), 16);
                } else {
                    assert!(matches!(
                        result,
                        Err(E::Resource(AssertOriginResourceV1::Work(_)))
                    ));
                    assert_eq!(budget.work(), 0);
                }
                assert_eq!(budget.storage(), 19);
            }
            let launch = crate::ProductionSourceLaunchRosterV1::try_new(
                &input,
                &[crate::ProductionSourceLaunchRootInputV1::new(
                    "helper_value_source",
                    [31; 32],
                    crate::ProductionSourceLaunchInputV1::new(1, Some([1, 1, 1]), [1, 1, 1]),
                )],
            )
            .unwrap();
            let semantic = fe2o3_pliron::ProductionSemanticMirOwnerV1::try_new(
                input,
                fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
            )
            .unwrap();
            let ssa = fe2o3_pliron::ProductionSemanticSsaOwnerV1::try_new(
                semantic,
                fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
            )
            .unwrap();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 128 << 20);
            budget.reserve_storage(19).unwrap();
            let materialized = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
                ssa,
                launch,
                ProductionSemanticKirLimitsV1::default(),
                &mut budget,
            )
            .unwrap();
            let source_storage = materialized.unit_local_source_storage_floor_v1().unwrap();
            budget.reserve_storage(source_storage).unwrap();
            let floor = budget.storage();
            let result = census::source_parts(
                materialized.semantic_ssa().source_semantic(),
                &materialized.correspondence,
                &mut budget,
            );
            if count.is_some_and(|n| n < 32) {
                result.unwrap();
            } else {
                assert!(matches!(
                    result,
                    Err(E::Unsupported {
                        phase: "source",
                        detail: "total scalar/global recipe; no unchecked arithmetic"
                    })
                ));
            }
            budget.release_storage(budget.storage() - floor).unwrap();
            drop(materialized);
            budget.release_storage(source_storage).unwrap();
            assert_eq!(budget.storage(), 19);
        }
    }
}
