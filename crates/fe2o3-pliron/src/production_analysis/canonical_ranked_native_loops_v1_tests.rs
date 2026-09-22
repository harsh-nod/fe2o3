use super::*;
use fe2o3_kernel_ir::{CastKind, ComparePredicate};
use pliron::builtin::attributes::IdentifierAttr;

fn scalar_constant(ty: ScalarType, value: i64) -> Constant {
    match ty {
        ScalarType::U8 => Constant::U8(u8::try_from(value).unwrap()),
        ScalarType::U32 => Constant::U32(u32::try_from(value).unwrap()),
        ScalarType::I32 => Constant::I32(i32::try_from(value).unwrap()),
        _ => panic!("fixture scalar is outside the explicit test roster"),
    }
}

fn loop_function(name: &str, scalar: ScalarType, seed: i64, narrow_bound: bool) -> Function {
    let ty = Type::Scalar(scalar);
    let bound_ty = if narrow_bound {
        Type::Scalar(ScalarType::U8)
    } else {
        ty.clone()
    };
    let mut entry = BasicBlock::new(BlockId(41));
    entry.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(1), ty.clone()),
            OperationKind::Constant(scalar_constant(scalar, seed)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2), ty.clone()),
            OperationKind::Constant(scalar_constant(scalar, if narrow_bound { 2 } else { 1 })),
        ),
    ];
    let bound = if narrow_bound {
        assert_eq!(scalar, ScalarType::U32);
        entry.operations.push(Operation::effect_free(
            ValueDef::new(ValueId(3), ty.clone()),
            OperationKind::Cast {
                kind: CastKind::ZeroExtend,
                value: ValueId(0),
                to: ty.clone(),
            },
        ));
        ValueId(3)
    } else {
        ValueId(0)
    };
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(43),
        arguments: vec![ValueId(1)],
    });
    let mut header = BasicBlock::new(BlockId(43));
    header
        .parameters
        .push(ValueDef::new(ValueId(10), ty.clone()));
    header.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(11), Type::BOOL),
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(10),
            rhs: bound,
        },
    ));
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(11),
        then_target: BlockId(47),
        then_arguments: vec![],
        else_target: BlockId(53),
        else_arguments: vec![],
    });
    let mut body = BasicBlock::new(BlockId(47));
    body.operations.push(Operation::new(
        vec![
            ValueDef::new(ValueId(12), ty),
            ValueDef::new(ValueId(13), Type::BOOL),
        ],
        OperationKind::Binary {
            op: BinaryOp::Checked(CheckedBinaryOperator::Add),
            lhs: ValueId(10),
            rhs: ValueId(2),
        },
    ));
    body.terminator = Some(Terminator::Branch {
        target: BlockId(43),
        arguments: vec![ValueId(12)],
    });
    let mut exit = BasicBlock::new(BlockId(53));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    Function::kernel_entry(
        name,
        Signature::new(vec![bound_ty], vec![]),
        vec![ValueId(0)],
        vec![entry, header, body, exit],
    )
}

fn loop_module(cases: &[(&str, ScalarType, i64, bool)]) -> Module {
    let mut module = Module::new("canonical-native-loop-integration");
    for &(name, ty, seed, narrow) in cases {
        module.functions.push(loop_function(name, ty, seed, narrow));
        module.kernels.push(Kernel::new(
            name,
            name,
            LaunchDomain::D1 {
                x: LaunchExtent::Static(64),
            },
        ));
    }
    module
}

fn assert_projection_symbols(owner: &Owner, budget: &mut Budget<'_>) {
    let floor = budget.storage();
    resources::protected(budget, |budget| {
        let mut projection = Projection::import(owner, budget)?;
        assert!(std::ptr::eq(projection.owner(), owner));
        projection.check(budget)?;
        projection.test_live(|context, root| {
            let region = root.deref(context).get_region(0);
            let block = region.deref(context).iter(context).next().unwrap();
            let mut count = 0;
            for (ordinal, function) in block.deref(context).iter(context).enumerate() {
                let raw = function.deref(context);
                let key = "sym_name".try_into().unwrap();
                let name = raw.attributes.get::<IdentifierAttr>(&key).unwrap();
                let actual: &str = name.as_ref().as_ref();
                assert_eq!(actual, format!("kir_fn_{ordinal}"));
                count += 1;
            }
            assert_eq!(count, owner.module().functions.len());
        });
        projection.check(budget)?;
        Ok(())
    })
    .unwrap();
    assert_eq!(budget.storage(), floor);
}

fn assert_public_loop_reports(module: &Module, steps: &[u64]) {
    with_checked(module, |checked, budget| {
        let owner = checked.inventory(budget).unwrap().owner();
        let bytes = owner.canonical().canonical_bytes().to_vec();
        let kernels = owner.module().kernels.clone();
        let functions = owner
            .module()
            .functions
            .iter()
            .map(|f| f.id.clone())
            .collect::<Vec<_>>();
        // Separate exact Projection observations on this same owner; the public
        // callback intentionally does not expose its private native graph.
        assert_projection_symbols(owner, budget);
        let floor = budget.storage();
        let mut entered = false;
        crate::with_canonical_ranked_policy_checks_v1(checked, budget, |policies, budget| {
            entered = true;
            let actual = policies.owner(budget)?;
            assert!(std::ptr::eq(actual, owner));
            assert_eq!(actual.canonical().canonical_bytes(), bytes);
            assert_eq!(actual.module().kernels, kernels);
            assert!(
                actual
                    .module()
                    .functions
                    .iter()
                    .map(|f| &f.id)
                    .eq(functions.iter())
            );
            assert_eq!(policies.function_count(budget)?, steps.len());
            for (ordinal, &step) in steps.iter().enumerate() {
                let report = policies.report(ordinal, budget)?;
                assert_eq!(
                    report.pass_order(),
                    &crate::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2
                );
                assert!(report.is_clean());
                let [certificate] = report.semantics().progress().certificates() else {
                    panic!("each actual imported loop must produce exactly one certificate");
                };
                assert_eq!(certificate.step(), step);
                assert_ne!(certificate.header(), certificate.exit());
                assert_ne!(certificate.body(), certificate.exit());
                assert!(!certificate.induction().is_empty());
                assert!(!certificate.bound().is_empty());
                assert!(
                    !report
                        .semantics()
                        .progress()
                        .grants_launch_or_liveness_authority()
                );
                let history = policies.history(ordinal, budget)?;
                assert_eq!(history.function(), ordinal);
                assert!(history.invocation().work_upper_bound() > 0);
                if ordinal > 0 {
                    assert!(history.floor().work_upper_bound() > 0);
                }
            }
            assert_eq!(policies.pending_obligations().iter().count(), 19);
            assert!(!policies.ranked_verification_is_complete());
            assert!(!policies.grants_artifact_or_launch_authority());
            Ok(())
        })
        .unwrap();
        assert!(entered);
        assert_eq!(budget.storage(), floor);
        assert_eq!(owner.canonical().canonical_bytes(), bytes);
        assert_eq!(owner.module().kernels, kernels);
        assert_projection_symbols(owner, budget);
    });
}

#[test]
fn canonical_native_dynamic_finite_loop_reaches_public_all_nine_callback() {
    assert_public_loop_reports(
        &loop_module(&[("dynamic", ScalarType::U32, 0, false)]),
        &[1],
    );
}

#[test]
fn canonical_native_typed_nonzero_seed_loops_keep_exact_owner_and_certificates() {
    for (ty, seed) in [
        (ScalarType::U32, 7),
        (ScalarType::U8, 2),
        (ScalarType::I32, -3),
    ] {
        assert_public_loop_reports(&loop_module(&[("typed", ty, seed, false)]), &[1]);
    }
}

#[test]
fn canonical_native_narrow_bound_step_two_reaches_public_callback() {
    assert_public_loop_reports(&loop_module(&[("narrow", ScalarType::U32, 7, true)]), &[2]);
}

#[test]
fn canonical_native_multiroot_loop_reports_preserve_ordered_roster() {
    let mut module = loop_module(&[
        ("zeta", ScalarType::U32, 7, false),
        ("alpha", ScalarType::I32, -3, false),
    ]);
    module.kernels.reverse();
    assert_public_loop_reports(&module, &[1, 1]);
}

#[test]
fn canonical_native_unknown_entry_cycle_refuses_before_public_callback() {
    let mut module = loop_module(&[("unknown", ScalarType::U32, 7, false)]);
    let function = &mut module.functions[0];
    function.signature.parameters = vec![Type::BOOL];
    let body = function.body.as_mut().unwrap();
    body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(43),
        then_arguments: vec![],
        else_target: BlockId(53),
        else_arguments: vec![],
    });
    body.blocks[1].parameters.clear();
    body.blocks[1].operations.clear();
    body.blocks[1].terminator = Some(Terminator::Branch {
        target: BlockId(43),
        arguments: vec![],
    });
    body.blocks.remove(2);
    with_checked(&module, |checked, budget| {
        let owner = checked.inventory(budget).unwrap().owner();
        let bytes = owner.canonical().canonical_bytes().to_vec();
        let roots = owner.module().kernels.clone();
        let floor = budget.storage();
        let error = crate::with_canonical_ranked_policy_checks_v1(
            checked,
            budget,
            |_, _| -> Result<(), Failure> { panic!("unknown cycle entry must not reach success") },
        )
        .unwrap_err();
        let Failure::Analysis {
            function: 0,
            cause: ProductionPlironPreloweringErrorV2::Semantic(cause),
        } = error.failure()
        else {
            panic!("expected real progress refusal: {error:?}");
        };
        assert!(
            cause
                .report()
                .progress()
                .findings()
                .iter()
                .any(|finding| matches!(
                    finding,
                    crate::PlironProgressFindingV1::ProgressIncomplete { .. }
                ))
        );
        assert!(
            !cause
                .report()
                .progress()
                .findings()
                .iter()
                .any(|finding| matches!(
                    finding,
                    crate::PlironProgressFindingV1::NonTerminatingCycle { .. }
                ))
        );
        assert!(error.observation().work_upper_bound() > 0);
        assert_eq!(error.last_invocation().unwrap().function(), 0);
        assert_eq!(budget.storage(), floor);
        assert_eq!(owner.canonical().canonical_bytes(), bytes);
        assert_eq!(owner.module().kernels, roots);
    });
}
