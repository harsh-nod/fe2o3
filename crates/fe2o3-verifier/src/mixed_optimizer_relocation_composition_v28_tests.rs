use super::*;
use fe2o3_kernel_analysis::{
    CheckedCanonicalKirTransitionV18 as Prefix, check_canonical_kir_transition_v18,
};
use fe2o3_kernel_ir::{AccessMode, AddressSpace, MemoryAccess};

fn composed_fixture(nested: bool) -> Module {
    let mut module = fixture(nested);
    let function = &mut module.functions[0];
    function.signature.parameters.push(Type::pointer(
        scalar(),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    ));
    let body = function.body.as_mut().unwrap();
    body.parameters.push(ValueId(30));
    let operations = &mut body.blocks[if nested { 4 } else { 2 }].operations;
    operations.insert(
        1,
        op(
            24,
            scalar(),
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(if nested { 10 } else { 2 }),
            },
        ),
    );
    let OperationKind::Binary { lhs, .. } = &mut operations[2].kind else {
        unreachable!()
    };
    *lhs = ValueId(24);
    for value in [20, 21] {
        operations.push(Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(30),
                value: ValueId(value),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
    }
    module
}

fn with_composed(
    nested: bool,
    consume: impl FnOnce(&Prefix<'_, '_, '_, '_>, &Inventory<'_>, &Pair<'_>, &mut Budget<'_>),
) {
    with_composed_module(&composed_fixture(nested), consume);
}

fn with_composed_module(
    module: &Module,
    consume: impl FnOnce(&Prefix<'_, '_, '_, '_>, &Inventory<'_>, &Pair<'_>, &mut Budget<'_>),
) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.reserve_storage(29).unwrap();
    let (owner, stored) =
        Owner::from_module_ref_with_verification_budget_v18(module, LAYOUTS, &mut budget).unwrap();
    budget.reserve_storage(stored.retained_storage()).unwrap();
    let observed =
        fe2o3_pliron::optimize_neutral_kernel_ir_mixed_pure_cse_v18(&owner, LAYOUTS, &mut budget)
            .unwrap();
    budget
        .reserve_storage(observed.storage().retained_storage())
        .unwrap();
    assert_eq!(observed.execution().policy_version(), 10);
    let (original, original_storage) = Inventory::derive_v18(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(original_storage.retained_storage())
        .unwrap();
    let (prefix, prefix_storage) = Inventory::derive_v18(observed.owner(), &mut budget).unwrap();
    budget
        .reserve_storage(prefix_storage.retained_storage())
        .unwrap();
    assert!(
        original.operations().len() > prefix.operations().len(),
        "the prefix must perform real CSE"
    );
    let (transition, transition_storage) = check_canonical_kir_transition_v18(
        &original,
        &prefix,
        observed.occurrences().candidate(),
        &mut budget,
    )
    .unwrap();
    budget
        .reserve_storage(transition_storage.retained_storage())
        .unwrap();
    let tail =
        fe2o3_kernel_opt::prepare_owned_licm_v18(observed.owner(), LAYOUTS, &mut budget).unwrap();
    budget.reserve_storage(tail.retained_storage()).unwrap();
    let (pair, pair_storage) = tail.replay_against(observed.owner(), &mut budget).unwrap();
    budget
        .reserve_storage(pair_storage.retained_storage())
        .unwrap();
    assert!(
        pair.origins().iter().any(|row| row.hoist.is_some()),
        "the tail must perform real LICM"
    );
    let (output, output_storage) = Inventory::derive_v18(pair.output(), &mut budget).unwrap();
    budget
        .reserve_storage(output_storage.retained_storage())
        .unwrap();
    consume(&transition, &output, &pair, &mut budget);
    drop(output);
    drop(pair);
    drop(tail);
    drop(transition);
    drop((original, prefix));
    drop(observed);
    drop(owner);
    budget.release_storage(budget.storage() - 29).unwrap();
    assert_eq!(budget.storage(), 29);
}

fn composed_select_fixture() -> Module {
    let mut module = composed_fixture(false);
    let function = &mut module.functions[0];
    function.signature.parameters.push(Type::BOOL);
    let body = function.body.as_mut().unwrap();
    body.parameters.push(ValueId(31));
    body.blocks[2].operations.push(op(
        32,
        scalar(),
        OperationKind::Select {
            condition: ValueId(31),
            true_value: ValueId(20),
            false_value: ValueId(2),
        },
    ));
    body.blocks[2].operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(30),
            value: ValueId(32),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    module
}

#[test]
fn composed_relocation_select_is_concrete_and_non_select_operator_bridge_stays_checked() {
    with_composed_module(
        &composed_select_fixture(),
        |checked, output, pair, budget| {
            assert!(pair.origins().iter().enumerate().any(|(ordinal, row)| {
                row.hoist.is_some()
                    && matches!(
                        checked.output().operations()[ordinal].operation.kind,
                        OperationKind::Select { .. }
                    )
            }));
            let floor = budget.storage();
            let text = generated_composed(checked, output, pair, budget).unwrap();
            assert_eq!(text.matches("open spec fn select_value_v28(").count(), 2);
            assert!(text.contains("select_value_v28(base[4],relocated_value_"));
            assert!(text.contains("proof fn composed_original_to_final_trace_0_v28"));
            let retained = text.retained;
            drop(text);
            budget.release_storage(retained).unwrap();
            budget.reserve_storage(2 * SOURCE_LIMIT).unwrap();
            let mut writer = Writer::new(budget).unwrap();
            semantics::bridge_negative_controls(checked, output, pair, &mut writer).unwrap();
            drop(writer);
            budget.release_storage(budget.storage() - floor).unwrap();
            assert_eq!(budget.storage(), floor);
        },
    );
}

fn generated_composed(
    checked: &Prefix<'_, '_, '_, '_>,
    output: &Inventory<'_>,
    pair: &Pair<'_>,
    budget: &mut Budget<'_>,
) -> std::result::Result<Generated, ProofError> {
    let floor = budget.storage();
    budget.reserve_storage(2 * SOURCE_LIMIT)?;
    let mut writer = Writer::new(budget)?;
    let blocks =
        semantics::generate_composed_relocation_cfg_v28(checked, output, pair, &mut writer)?;
    assert_eq!(blocks, output.blocks().len());
    Ok(Generated {
        text: writer.finish()?,
        retained: budget.storage() - floor,
    })
}

#[test]
fn composed_relocation_cfg_joins_actual_cse_and_motion_through_checked_operators() {
    for nested in [false, true] {
        with_composed(nested, |checked, output, pair, budget| {
            let floor = budget.storage();
            let text = generated_composed(checked, output, pair, budget).unwrap();
            for required in [
                "mod original_prefix_v28 {",
                "mod relocation_v28 {",
                "proof fn cfg_function_trace_refinement_0_v26",
                "proof fn relocation_function_trace_0_v28",
                "proof fn prefix_step_interpretations_agree_v28",
                "proof fn prefix_trace_interpretations_agree_v28",
                "proof fn composed_original_to_final_trace_0_v28",
                "open spec fn original_interpretation_v28",
                "prefix_operator_v28(op)",
                "super::cfg_trace_v26",
                "child_state_v28(p)",
            ] {
                assert!(text.contains(required), "missing {required}");
            }
            for forbidden in ["assume(", "admit(", "external_body"] {
                assert!(!text.contains(forbidden));
            }
            assert_eq!(
                text.matches("proof fn composed_original_to_final_trace_")
                    .count(),
                1
            );
            let retained = text.retained;
            let canonical =
                crate::CanonicalGeneratedVerusProofInputV3::new(text.text.into_bytes()).unwrap();
            drop(canonical);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn composed_relocation_cfg_generation_has_exact_and_one_short_work_and_storage() {
    with_composed(true, |checked, output, pair, held| {
        let floor = held.storage();
        let run = |work_limit, storage_limit| {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let result = generated_composed(checked, output, pair, &mut budget).map(drop);
            let used = budget.work();
            let peak = budget.peak_storage();
            budget.release_storage(budget.storage() - floor).unwrap();
            assert_eq!(budget.storage(), floor);
            (result, used, peak)
        };
        let (result, work, peak) = run(WORK, STORAGE);
        result.unwrap();
        let (result, exact_work, exact_peak) = run(work, peak);
        result.unwrap();
        assert_eq!((exact_work, exact_peak), (work, peak));
        assert!(
            matches!(resource(&run(work - 1, peak).0.unwrap_err()), Resource::Work(error) if error.limit() == work - 1)
        );
        assert!(
            matches!(resource(&run(work, peak - 1).0.unwrap_err()), Resource::Storage(error) if error.limit() == peak - 1)
        );
    });
}

#[test]
fn composed_relocation_cfg_rejects_equal_byte_foreign_intermediate_owner() {
    with_composed(false, |checked, output, pair, budget| {
        let floor = budget.storage();
        let foreign = fe2o3_pliron::optimize_neutral_kernel_ir_mixed_pure_cse_v18(
            checked.input().owner(),
            LAYOUTS,
            budget,
        )
        .unwrap();
        budget
            .reserve_storage(foreign.storage().retained_storage())
            .unwrap();
        assert_eq!(foreign.owner().identity(), pair.input().identity());
        let (inventory, storage) = Inventory::derive_v18(foreign.owner(), budget).unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let (transition, retained) = check_canonical_kir_transition_v18(
            checked.input(),
            &inventory,
            foreign.occurrences().candidate(),
            budget,
        )
        .unwrap();
        budget.reserve_storage(retained.retained_storage()).unwrap();
        budget.reserve_storage(2 * SOURCE_LIMIT).unwrap();
        let mut writer = Writer::new(budget).unwrap();
        assert!(matches!(
            semantics::generate_composed_relocation_cfg_v28(&transition, output, pair, &mut writer),
            Err(ProofError::Statement(
                "exact composed intermediate and final owners"
            ))
        ));
        drop(writer);
        drop(transition);
        drop(inventory);
        drop(foreign);
        budget.release_storage(budget.storage() - floor).unwrap();
    });
}

#[test]
fn composed_relocation_operator_bridge_rejects_missing_changed_invented_and_cross_class_keys() {
    for nested in [false, true] {
        with_composed(nested, |checked, output, pair, budget| {
            let floor = budget.storage();
            budget.reserve_storage(2 * SOURCE_LIMIT).unwrap();
            let mut writer = Writer::new(budget).unwrap();
            semantics::bridge_negative_controls(checked, output, pair, &mut writer).unwrap();
            drop(writer);
            budget.release_storage(budget.storage() - floor).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}
