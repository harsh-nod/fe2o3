//! Actual source-tail shape expressed using the shared inert composition fixture.
//! This is structural qualification, not authenticated Rust source evidence.
use super::*;
fn switched(direct: usize, helper_regions: &[usize], helper_calls: &[usize]) -> Module {
    let mut module = fixture::module(direct, helper_regions, helper_calls, 1);
    let root = &mut module.functions[0];
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    root.signature.parameters[3] = Type::slice(scalar, AddressSpace::Global, AccessMode::ReadWrite);
    let body = root.body.as_mut().unwrap();
    let entry = &mut body.blocks[0];
    let mut store = entry.operations.pop().unwrap();
    let OperationKind::Store {
        pointer: target, ..
    } = &mut store.kind
    else {
        panic!("shared store");
    };
    *target = ValueId(204);
    let op = |id, ty, kind| Operation::effect_free(ValueDef::new(ValueId(id), ty), kind);
    entry.operations.extend([
        op(
            200,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        ),
        op(
            201,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(3) },
        ),
        op(
            202,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(200),
                rhs: ValueId(201),
            },
        ),
        op(
            203,
            pointer.clone(),
            OperationKind::SliceData { slice: ValueId(3) },
        ),
        op(
            204,
            pointer,
            OperationKind::GetElementPointer {
                base: ValueId(203),
                offset: ValueId(200),
            },
        ),
        op(
            205,
            Type::Scalar(ScalarType::I64),
            OperationKind::Cast {
                kind: CastKind::ZeroExtend,
                value: ValueId(202),
                to: Type::Scalar(ScalarType::I64),
            },
        ),
    ]);
    entry.terminator = Some(Terminator::Switch {
        selector: ValueId(205),
        cases: vec![
            SwitchCase {
                value: 0,
                target: BlockId(42),
                arguments: vec![],
            },
            SwitchCase {
                value: 1,
                target: BlockId(41),
                arguments: vec![],
            },
        ],
        default_target: BlockId(43),
        default_arguments: vec![],
    });
    let mut yes = BasicBlock::new(BlockId(41));
    yes.operations.push(store);
    yes.terminator = Some(Terminator::Branch {
        target: BlockId(44),
        arguments: vec![],
    });
    let mut no = BasicBlock::new(BlockId(42));
    no.terminator = Some(Terminator::Branch {
        target: BlockId(44),
        arguments: vec![],
    });
    let mut dead = BasicBlock::new(BlockId(43));
    dead.terminator = Some(Terminator::Unreachable);
    let mut done = BasicBlock::new(BlockId(44));
    done.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks.extend([yes, no, dead, done]);
    module
}
fn report(module: &Module) -> FormalMemoryObligationAnalysis {
    let owner = compose(module).unwrap();
    extract(&owner, FormalIndexWidth::Bits64, 37, 1_000_000, 1_000_000)
        .0
        .unwrap()
        .0
}
#[test]
fn composition_formal_bool_switch_preserves_exact_access_and_conservative_bounds() {
    for (direct, helpers, calls) in [
        (2, vec![], vec![]),
        (0, vec![1], vec![0]),
        (0, vec![1], vec![0, 0]),
    ] {
        let module = switched(direct, &helpers, &calls);
        let actual = report(&module);
        assert!(actual.is_complete());
        let obligations = actual.obligations();
        assert_eq!(obligations.accesses().len(), 1);
        assert!(obligations.inter_invocation_conflicts().is_empty());
        assert_eq!(
            obligations.accesses()[0].byte_offset(),
            ByteExpression::invocation_affine(0, 4)
        );
        // No selected-offset recipe was manufactured from the direct GEP.
        assert!(matches!(
            obligations.accesses()[0].domain(),
            FormalAccessDomainV1::LaunchEnvelope
        ));
        assert!(!obligations.bounds_requirements().is_empty());
        let mut conditional = module.clone();
        conditional.functions[0].body.as_mut().unwrap().blocks[0].terminator =
            Some(Terminator::ConditionalBranch {
                condition: ValueId(202),
                then_target: BlockId(41),
                then_arguments: vec![],
                else_target: BlockId(42),
                else_arguments: vec![],
            });
        let before = report(&conditional);
        assert_eq!(before.obligations().accesses(), obligations.accesses());
        assert_eq!(
            before.obligations().bounds_requirements(),
            obligations.bounds_requirements()
        );
        let owner = compose(&module).unwrap();
        let generic = derive_kernel_memory_obligations_from_verified_for_launch(
            owner.canonical().verified_module_ref_v1(),
            &module.kernels[0].id,
            ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [64, 1, 1],
            },
            FormalIndexWidth::Bits64,
        )
        .unwrap();
        assert!(!generic.is_complete()); // No generic authored/call effect promotion.
    }
}
#[test]
fn composition_formal_bool_switch_wrong_cases_selector_default_and_typed_switch_refuse() {
    for mutation in 0..8 {
        let mut module = switched(1, &[], &[]);
        let body = module.functions[0].body.as_mut().unwrap();
        match mutation {
            0 => {
                let Some(Terminator::Switch { cases, .. }) = &mut body.blocks[0].terminator else {
                    unreachable!()
                };
                cases[1].value = 2;
            }
            1 => {
                let Some(Terminator::Switch { cases, .. }) = &mut body.blocks[0].terminator else {
                    unreachable!()
                };
                cases.remove(0);
            }
            2 => {
                let Some(Terminator::Switch { selector, .. }) = &mut body.blocks[0].terminator
                else {
                    unreachable!()
                };
                *selector = ValueId(0);
            }
            3 => body.blocks[3].terminator = Some(Terminator::Return { values: vec![] }),
            4 => body.blocks[3].operations.push(Operation::effect_free(
                ValueDef::new(ValueId(206), Type::Scalar(ScalarType::U32)),
                OperationKind::Constant(Constant::U32(7)),
            )),
            5 => {
                let Some(Terminator::Switch { default_target, .. }) =
                    &mut body.blocks[0].terminator
                else {
                    unreachable!()
                };
                *default_target = BlockId(42);
            }
            6 => {
                body.blocks[0].terminator = Some(Terminator::IntegerSwitch {
                    selector: ValueId(205),
                    cases: vec![
                        IntegerSwitchCase {
                            value: Constant::I64(0),
                            target: BlockId(42),
                            arguments: vec![],
                        },
                        IntegerSwitchCase {
                            value: Constant::I64(1),
                            target: BlockId(41),
                            arguments: vec![],
                        },
                    ],
                    default_target: BlockId(43),
                    default_arguments: vec![],
                })
            }
            7 => {
                // A valid unsigned Bool discriminant is outside this exact
                // source-observed signed-isize profile, not silently broadened.
                let cast = body.blocks[0].operations.last_mut().unwrap();
                cast.results[0].ty = Type::Scalar(ScalarType::U64);
                let OperationKind::Cast { to, .. } = &mut cast.kind else {
                    unreachable!()
                };
                *to = Type::Scalar(ScalarType::U64);
            }
            _ => unreachable!(),
        }
        let owner = compose(&module).unwrap(); // Every negative remains valid canonical KIR.
        let result = extract(&owner, FormalIndexWidth::Bits64, 41, 1_000_000, 1_000_000).0;
        assert!(
            matches!(
                result,
                Err(OrderedCompositionFormalErrorV1::Profile(
                    "formal bounded branch profile"
                ))
            ),
            "wrong switch mutation {mutation}: {result:?}"
        );
    }
}
#[test]
fn composition_formal_bool_switch_keeps_exact_budget_cutoffs_and_nonzero_floor() {
    let owner = compose(&switched(1, &[1], &[0])).unwrap();
    for floor in [0, 91] {
        let (accepted, work, peak) = extract(
            &owner,
            FormalIndexWidth::Bits64,
            floor,
            1_000_000,
            1_000_000,
        );
        assert!(accepted.is_ok());
        assert!(
            extract(&owner, FormalIndexWidth::Bits64, floor, work, peak)
                .0
                .is_ok()
        );
        assert!(
            extract(&owner, FormalIndexWidth::Bits64, floor, work - 1, peak)
                .0
                .is_err()
        );
        assert!(
            extract(&owner, FormalIndexWidth::Bits64, floor, work, peak - 1)
                .0
                .is_err()
        );
    }
}
