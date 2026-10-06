use super::*;
use fe2o3_kernel_ir::{AccessMode, AddressSpace};

fn select_module(ty: Type, condition: u32, same: bool) -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.push(Instruction::effect_free(
        ValueDef::new(ValueId(4), ty.clone()),
        OperationKind::Select {
            condition: ValueId(condition),
            true_value: ValueId(1),
            false_value: ValueId(if same { 1 } else { 2 }),
        },
    ));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });
    let mut exit = BasicBlock::new(BlockId(1));
    exit.terminator = Some(Terminator::Return {
        values: vec![ValueId(4)],
    });
    let mut module = Module::new("concrete-select-v28");
    module.functions.push(Function::internal_helper(
        "entry",
        Signature::new(
            vec![Type::BOOL, ty.clone(), ty.clone(), Type::BOOL],
            vec![ty],
        ),
        vec![ValueId(0), ValueId(1), ValueId(2), ValueId(3)],
        vec![entry, exit],
    ));
    module
}

fn select_source(module: Module, cfg: bool) -> String {
    with_pure_cse_v27(module, |input, output, rows, budget| {
        let floor = budget.storage();
        budget.reserve_storage(SOURCE_LIMIT).unwrap();
        let mut out = Writer::new(budget).unwrap();
        if cfg {
            generate_cfg_v27(input, output, rows, &mut out).unwrap();
        } else {
            generate(input, output, rows, &mut out).unwrap();
        }
        let text = out.finish().unwrap();
        budget.release_storage(budget.storage() - floor).unwrap();
        text
    })
}

#[test]
fn select_is_concrete_in_both_cfg_profiles_and_retains_condition_and_branch_order() {
    for cfg in [false, true] {
        let first = select_source(select_module(Type::Scalar(ScalarType::U32), 0, false), cfg);
        let second = select_source(select_module(Type::Scalar(ScalarType::U32), 3, false), cfg);
        assert!(first.contains(SELECT_PRELUDE));
        assert!(first.contains("select_value_v28(base[0],base[1],base[2],)"));
        assert!(first.contains("base[4] == select_value_v28(base[0],base[1],base[2])"));
        assert!(second.contains("select_value_v28(base[3],base[1],base[2],)"));
        assert_ne!(first, second);
        for text in [&first, &second] {
            assert!(!text.contains("op(0, 0,"));
            assert!(!text.contains("op(1, 0,"));
            assert!(!text.contains("assume("));
            assert!(!text.contains("external_body"));
            crate::CanonicalGeneratedVerusProofInputV3::new(text.as_bytes().to_vec()).unwrap();
        }
    }
    assert!(!source(scalar_case(Constant::U32(0), BinaryOp::Add, false)).contains(SELECT_PRELUDE));
}

#[test]
fn actual_policy11_select_same_value_has_an_explicit_concrete_proof_law() {
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (owner, stored) = Owner::from_module_ref_with_verification_budget_v18(
        &select_module(Type::Scalar(ScalarType::U32), 0, true),
        LAYOUTS,
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(stored.retained_storage()).unwrap();
    let observed =
        fe2o3_pliron::optimize_neutral_kernel_ir_mixed_fixedpoint_v18(&owner, LAYOUTS, &mut budget)
            .unwrap();
    budget
        .reserve_storage(observed.storage().retained_storage())
        .unwrap();
    assert_eq!(observed.execution().policy_version(), 11);
    let (input, ai) = Inventory::derive_v18(&owner, &mut budget).unwrap();
    budget.reserve_storage(ai.retained_storage()).unwrap();
    let (output, bi) = Inventory::derive_v18(observed.owner(), &mut budget).unwrap();
    budget.reserve_storage(bi.retained_storage()).unwrap();
    assert_eq!(input.operations().len(), 1);
    assert!(output.operations().is_empty());
    let (checked, ci) = check_canonical_kir_transition_v18(
        &input,
        &output,
        observed.occurrences().candidate(),
        &mut budget,
    )
    .unwrap();
    budget.reserve_storage(ci.retained_storage()).unwrap();
    assert!(!checked.grants_authority());
    for cfg in [false, true] {
        let floor = budget.storage();
        budget.reserve_storage(SOURCE_LIMIT).unwrap();
        let mut out = Writer::new(&mut budget).unwrap();
        if cfg {
            generate_cfg_v27(&input, &output, checked.rows(), &mut out).unwrap();
        } else {
            generate(&input, &output, checked.rows(), &mut out).unwrap();
        }
        let text = out.finish().unwrap();
        assert!(text.contains("proof fn select_same_value_v28(condition: int, value: int)"));
        assert!(text.contains("select_same_value_v28(base[0],base[1],);"));
        assert!(text.contains("select_value_v28(base[0],base[1],base[1],)"));
        assert!(!text.contains("op(0, 0,"));
        crate::CanonicalGeneratedVerusProofInputV3::new(text.into_bytes()).unwrap();
        budget.release_storage(budget.storage() - floor).unwrap();
    }
}

#[test]
fn select_rejects_inconsistent_condition_branch_and_result_types() {
    let u32ty = Type::Scalar(ScalarType::U32);
    let u16ty = Type::Scalar(ScalarType::U16);
    let ro = Type::pointer(u32ty.clone(), AddressSpace::Global, AccessMode::ReadOnly);
    let rw = Type::pointer(u32ty.clone(), AddressSpace::Global, AccessMode::ReadWrite);
    let private = Type::pointer(u32ty.clone(), AddressSpace::Private, AccessMode::ReadOnly);
    for (condition, when_true, when_false, result) in [
        (&u32ty, &u32ty, &u32ty, &u32ty),
        (&Type::BOOL, &u16ty, &u32ty, &u32ty),
        (&Type::BOOL, &u32ty, &u16ty, &u32ty),
        (&Type::BOOL, &u32ty, &u32ty, &u16ty),
        (&Type::BOOL, &ro, &rw, &ro),
        (&Type::BOOL, &ro, &private, &ro),
    ] {
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        let mut out = Writer {
            text: String::new(),
            budget: &mut budget,
            failure: None,
        };
        assert!(matches!(
            select_types(condition, when_true, when_false, result, &mut out),
            Err(Error::Statement(
                "concrete Select condition and branch types"
            ))
        ));
    }
    let mut work = Work::new(LIMIT);
    let mut budget = Budget::new(&mut work, LIMIT);
    let (first, first_storage) = Owner::from_module_ref_with_verification_budget_v18(
        &select_module(u32ty, 0, false),
        LAYOUTS,
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(first_storage.retained_storage())
        .unwrap();
    let (second, second_storage) = Owner::from_module_ref_with_verification_budget_v18(
        &select_module(u16ty, 0, false),
        LAYOUTS,
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(second_storage.retained_storage())
        .unwrap();
    assert_ne!(first.identity(), second.identity());
    let (same_width, same_width_storage) = Owner::from_module_ref_with_verification_budget_v18(
        &select_module(Type::F32, 0, false),
        LAYOUTS,
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(same_width_storage.retained_storage())
        .unwrap();
    assert_ne!(first.identity(), same_width.identity());
    let mut malformed = select_module(Type::Scalar(ScalarType::U32), 0, false);
    malformed.functions[0].body.as_mut().unwrap().blocks[0].operations[0]
        .results
        .clear();
    assert!(matches!(
        Owner::from_module_ref_with_verification_budget_v18(&malformed, LAYOUTS, &mut budget),
        Err(fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18::Verification(_))
    ));
}

#[test]
fn select_nested_type_comparison_has_exact_paid_work_and_no_scratch_growth() {
    for depth in [0, 1, 16] {
        let mut ty = Type::Scalar(ScalarType::U32);
        for _ in 0..depth {
            ty = Type::pointer(ty, AddressSpace::Global, AccessMode::ReadOnly);
        }
        let expected = 4 + 6 * (depth + 1);
        let run = |limit| {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, 17);
            budget.reserve_storage(17).unwrap();
            let mut out = Writer {
                text: String::new(),
                budget: &mut budget,
                failure: None,
            };
            let result = select_types(&Type::BOOL, &ty, &ty, &ty, &mut out);
            drop(out);
            assert_eq!(budget.storage(), 17);
            (result, budget.work())
        };
        let (result, used) = run(expected);
        result.unwrap();
        assert_eq!(used, expected);
        assert!(
            matches!(run(expected - 1).0, Err(Error::Resource(Resource::Work(error))) if error.limit() == expected - 1 && error.actual() == expected)
        );
    }
}

#[test]
fn select_cfg_generation_has_exact_and_one_short_work_and_storage() {
    with_pure_cse_v27(
        select_module(Type::Scalar(ScalarType::U32), 0, false),
        |input, output, rows, _| {
            let run = |work_limit, storage_limit| {
                let mut work = Work::new(work_limit);
                let mut budget = Budget::new(&mut work, storage_limit);
                budget.reserve_storage(SOURCE_LIMIT + 17).unwrap();
                let floor = budget.storage();
                let mut out = Writer::new(&mut budget).unwrap();
                let writer = out.budget.storage() - floor;
                let result = generate_cfg_v27(input, output, rows, &mut out);
                drop(out);
                assert_eq!(budget.storage(), floor + writer);
                budget.release_storage(writer).unwrap();
                (result, budget.work(), budget.peak_storage())
            };
            let (result, work, storage) = run(LIMIT, LIMIT);
            result.unwrap();
            let (result, exact_work, exact_storage) = run(work, storage);
            result.unwrap();
            assert_eq!((exact_work, exact_storage), (work, storage));
            assert!(
                matches!(run(work - 1, storage).0, Err(Error::Resource(Resource::Work(error))) if error.limit() == work - 1)
            );
            assert!(
                matches!(run(work, storage - 1).0, Err(Error::Resource(Resource::Storage(error))) if error.limit() == storage - 1)
            );
        },
    );
}
