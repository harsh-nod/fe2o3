//! Inert source-shaped tail controls, not live rustc/source authority.
use super::*;
use fe2o3_kernel_ir::*;

// Reuse the shared full composition fixture; only attach the observed
// slice/global-id/Bool-discriminant tail. Do not introduce an executable proxy.
fn switched(direct: usize, helpers: &[usize], calls: &[usize]) -> Module {
    let mut module = canonical_fixture::module(direct, helpers, calls, 1);
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let root = &mut module.functions[0];
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
fn launch() -> crate::ProductionSourceLaunchRosterV1 {
    let (ssa, _) = composition_owners(composition_request(CompositionCase::Root));
    crate::ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "ordered_program_root",
            [15; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [2, 1, 1]),
        )],
    )
    .unwrap()
}
type ProjectionResult = Result<
    (
        fe2o3_pliron::ProductionRankedKernelV1,
        Vec<OrderedCompositionLaunchEnvelopeRequirementV1>,
    ),
    String,
>;
fn trial(
    module: &Module,
    floor: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (ProjectionResult, usize, usize) {
    let launch = launch();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(floor).unwrap();
    let mut retained = 0;
    let result = (|| {
        let (canonical, storage) =
            VerifiedCanonicalKernelIrModuleV17::from_module_ref_with_verification_budget_v17(
                module,
                &mut budget,
            )
            .map_err(|e| format!("{e:?}"))?;
        budget
            .reserve_storage(storage.retained_storage())
            .map_err(|e| format!("{e:?}"))?;
        retained += storage.retained_storage();
        let (owner, roster) =
            VerifiedOrderedProgramCompositionV1::try_from_canonical_v17(canonical, &mut budget)
                .map_err(|e| format!("{e:?}"))?;
        budget
            .reserve_storage(roster.retained_storage())
            .map_err(|e| format!("{e:?}"))?;
        retained += roster.retained_storage();
        let bytes = owner.canonical().canonical_bytes().to_vec();
        let live = budget.storage();
        let result = super::super::super::super::ordered_composition_checks_v1::project_for_test(
            &owner,
            &launch,
            &mut budget,
        );
        assert_eq!(budget.storage(), live);
        assert_eq!(owner.canonical().canonical_bytes(), bytes);
        result
    })();
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), floor);
    (result, budget.work(), budget.peak_storage())
}
fn compile(recipe: fe2o3_pliron::ProductionRankedKernelV1) {
    let construction = fe2o3_pliron::ProductionConstructionV1::ranked_kernel(
        "composition_source_shaped_tail",
        recipe,
    )
    .unwrap();
    let checked = fe2o3_pliron::compile_ranked_kernel_for_gfx942_lowering_v1(
        construction,
        fe2o3_pliron::ProductionSessionLimitsV1::default(),
        std::iter::empty(),
    )
    .unwrap();
    assert!(checked.all_mandatory_reports_are_clean());
    assert!(!checked.has_retained_policy_checked_refinement_staging());
}
#[test]
fn ordered_composition_ranked_source_switch_root_helpers_and_full_envelope() {
    for (direct, helpers, calls) in [
        (2, vec![], vec![]),
        (0, vec![1], vec![0]),
        (0, vec![1], vec![0, 0]),
        (1, vec![1], vec![0]),
        (1, vec![0], vec![0]),
        (0, vec![1, 1], vec![0, 1]),
    ] {
        let module = switched(direct, &helpers, &calls);
        let (recipe, bounds) = trial(&module, 73, 1_000_000_000, 64 * 1024 * 1024)
            .0
            .unwrap();
        assert_eq!(bounds.len(), 1);
        assert_eq!(bounds[0].parameter_index(), 3);
        assert_eq!(bounds[0].minimum_byte_len(), 512);
        assert!(bounds[0].requires_write_permission());
        assert!(!bounds[0].requires_initialized_read());
        compile(recipe);
    }
}
#[test]
fn ordered_composition_ranked_source_switch_preserves_original_phi_dependencies() {
    let mut module = switched(1, &[1], &[0, 0]);
    let body = module.functions[0].body.as_mut().unwrap();
    body.blocks[4]
        .parameters
        .push(ValueDef::new(ValueId(206), Type::Scalar(ScalarType::U32)));
    for (i, argument) in [(1, ValueId(0)), (2, ValueId(1))] {
        let Some(Terminator::Branch { arguments, .. }) = &mut body.blocks[i].terminator else {
            panic!("shared branch")
        };
        arguments.push(argument);
    }
    body.blocks[4].operations.push(Operation::effect_free(
        ValueDef::new(ValueId(207), Type::Scalar(ScalarType::U32)),
        OperationKind::Binary {
            op: BinaryOp::BitXor,
            lhs: ValueId(206),
            rhs: ValueId(2),
        },
    ));
    compile(
        trial(&module, 31, 1_000_000_000, 64 * 1024 * 1024)
            .0
            .unwrap()
            .0,
    );
}
#[test]
fn ordered_composition_ranked_source_switch_refuses_malformed_signed_and_cyclic_shapes() {
    let baseline = switched(1, &[], &[]);
    assert!(
        trial(&baseline, 37, 1_000_000_000, 64 * 1024 * 1024)
            .0
            .is_ok()
    );
    for mutation in 0..9 {
        let mut module = baseline.clone();
        let body = module.functions[0].body.as_mut().unwrap();
        match mutation {
            0 => {
                let Some(Terminator::Switch { cases, .. }) = &mut body.blocks[0].terminator else {
                    panic!()
                };
                cases[1].value = 2;
            }
            1 => {
                let Some(Terminator::Switch { cases, .. }) = &mut body.blocks[0].terminator else {
                    panic!()
                };
                cases.remove(0); // Retain the store edge; canonical dominance stays valid.
            }
            2 => body.blocks[3].terminator = Some(Terminator::Return { values: vec![] }),
            3 => body.blocks[3].operations.push(Operation::effect_free(
                ValueDef::new(ValueId(206), Type::Scalar(ScalarType::U32)),
                OperationKind::Constant(Constant::U32(7)),
            )),
            4 => {
                let cast = body.blocks[0].operations.last_mut().unwrap();
                cast.results[0].ty = Type::Scalar(ScalarType::U64);
                let OperationKind::Cast { to, .. } = &mut cast.kind else {
                    panic!()
                };
                *to = Type::Scalar(ScalarType::U64);
            }
            5 => {
                let cast = body.blocks[0].operations.last_mut().unwrap();
                let OperationKind::Cast { value, .. } = &mut cast.kind else {
                    panic!()
                };
                *value = ValueId(0); // U32 -> I64 is not the Bool discriminant.
            }
            6 => {
                body.blocks[4].terminator = Some(Terminator::Branch {
                    target: BlockId(44),
                    arguments: vec![],
                })
            }
            7 => body.blocks[0].operations.push(Operation::effect_free(
                ValueDef::new(ValueId(206), Type::Scalar(ScalarType::I64)),
                OperationKind::Constant(Constant::I64(-1)),
            )),
            8 => body.blocks[0].operations.push(Operation::effect_free(
                ValueDef::new(ValueId(206), Type::Scalar(ScalarType::I64)),
                OperationKind::Cast {
                    kind: CastKind::ZeroExtend,
                    value: ValueId(202),
                    to: Type::Scalar(ScalarType::I64),
                },
            )), // Not the actual switch selector; no broad signed-value admission.
            _ => unreachable!(),
        }
        // Prove each control reaches a valid canonical structural owner, rather
        // than passing because of an unrelated malformed SSA/dominance error.
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 64 * 1024 * 1024);
        let (canonical, storage) =
            VerifiedCanonicalKernelIrModuleV17::from_module_ref_with_verification_budget_v17(
                &module,
                &mut budget,
            )
            .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let _owner =
            VerifiedOrderedProgramCompositionV1::try_from_canonical_v17(canonical, &mut budget)
                .unwrap();
        let result = trial(&module, 37, 1_000_000_000, 64 * 1024 * 1024).0;
        assert!(result.is_err(), "mutation {mutation}");
        if mutation == 6 {
            assert!(
                result
                    .unwrap_err()
                    .contains("root loop needs unsupported ranked control proof")
            );
        }
    }
}
#[test]
fn ordered_composition_ranked_source_switch_exact_resources_and_nonzero_floor() {
    let module = switched(1, &[1], &[0, 0]);
    for floor in [0, 91] {
        let (result, work, peak) = trial(&module, floor, 1_000_000_000, 64 * 1024 * 1024);
        assert!(result.is_ok());
        assert!(trial(&module, floor, work, peak).0.is_ok());
        assert!(trial(&module, floor, work - 1, peak).0.is_err());
        assert!(trial(&module, floor, work, peak - 1).0.is_err());
    }
}
