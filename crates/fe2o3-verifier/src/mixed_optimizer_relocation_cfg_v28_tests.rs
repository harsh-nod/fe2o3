use super::super::super::{Error as ProofError, SOURCE_LIMIT, Writer, semantics};
use super::*;

#[path = "mixed_optimizer_relocation_composition_v28_tests.rs"]
mod composed_tests;

struct Generated {
    text: String,
    retained: usize,
}
impl std::ops::Deref for Generated {
    type Target = str;
    fn deref(&self) -> &str {
        &self.text
    }
}

fn generated(
    pair: &Pair<'_>,
    budget: &mut Budget<'_>,
) -> std::result::Result<Generated, ProofError> {
    let floor = budget.storage();
    let (input, input_storage) = Inventory::derive_v18(pair.input(), budget)?;
    budget.reserve_storage(input_storage.retained_storage())?;
    let (output, output_storage) = Inventory::derive_v18(pair.output(), budget)?;
    budget.reserve_storage(output_storage.retained_storage())?;
    budget.reserve_storage(2 * SOURCE_LIMIT)?;
    let mut writer = Writer::new(budget)?;
    let blocks = semantics::generate_relocation_cfg_v28(&input, &output, pair, &mut writer)?;
    assert_eq!(blocks, input.blocks().len());
    let text = writer.finish()?;
    drop((input, output));
    budget.release_storage(input_storage.retained_storage() + output_storage.retained_storage())?;
    Ok(Generated {
        text,
        retained: budget.storage() - floor,
    })
}

#[test]
fn relocation_cfg_obligations_use_expressions_at_zero_trip_and_nested_cuts() {
    for nested in [false, true] {
        with_pair(nested, |pair, budget| {
            let floor = budget.storage();
            let plan = build(pair, budget).unwrap();
            plan.replay(pair, budget).unwrap();
            let text = generated(pair, budget).unwrap();
            for result in &plan.results {
                let input = result.input;
                let output = result.output;
                assert!(text.contains(&format!("spec fn relocated_value_{input}_v28")));
                assert!(text.contains(&format!(
                    "optimized[{output}] == relocated_value_{input}_v28(base, op)"
                )));
                assert!(
                    !text.contains(&format!("base[{input}] == optimized[{output}]")),
                    "a moved value is not a stale-slot equation"
                );
            }
            let header = if nested { 3 } else { 1 };
            let cut = text
                .split(&format!("spec fn relocation_cut_{header}_v28"))
                .nth(1)
                .unwrap()
                .split("\n}\n")
                .next()
                .unwrap();
            for result in &plan.results {
                assert!(
                    !cut.contains(&format!("base[{}] == relocated_value_", result.input)),
                    "the loop header can be reached before its original body executes"
                );
            }
            assert!(text.contains("let base = n.values; let initial = n.memory;"));
            assert!(text.contains("let base = o.values; let initial = o.memory;"));
            assert!(text.contains("proof fn relocation_function_trace_0_v28"));
            assert!(!text.contains("block_simulation_"));
            for forbidden in ["assume(", "admit(", "external_body", "cfg_live_"] {
                assert!(!text.contains(forbidden));
            }
            let retained = text.retained;
            drop(text);
            budget.release_storage(retained).unwrap();
            plan.discard(budget).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}

fn resource(error: &ProofError) -> Resource {
    let mut cause: &(dyn std::error::Error + 'static) = error;
    loop {
        if let Some(error) = cause.downcast_ref::<Resource>() {
            return *error;
        }
        if let Some(FlowError::Resource(error)) = cause.downcast_ref::<FlowError>() {
            return *error;
        }
        if let Some(error) = cause.downcast_ref::<fe2o3_kernel_ir::CanonicalKernelIrWorkLimitV1>() {
            return Resource::Work(*error);
        }
        cause = cause
            .source()
            .unwrap_or_else(|| panic!("not a resource denial: {error:?}"));
    }
}

#[test]
fn relocation_cfg_resource_oracle_preserves_typed_scope_limits() {
    for storage in [false, true] {
        let mut work = Work::new(1);
        let mut budget = Budget::new(&mut work, 1);
        let denied = if storage {
            budget.reserve_storage(2).unwrap_err()
        } else {
            budget.charge_work(2).unwrap_err()
        };
        assert_eq!(
            resource(&ProofError::Flow(FlowError::Resource(denied))),
            denied
        );
    }
    assert!(std::panic::catch_unwind(|| resource(&ProofError::Flow(FlowError::Panicked))).is_err());
}

#[test]
fn relocation_cfg_generation_has_exact_and_one_short_work_and_storage() {
    with_pair(true, |pair, held| {
        let floor = held.storage();
        let run = |work_limit, storage_limit| {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(floor).unwrap();
            let result = generated(pair, &mut budget).map(|text| {
                assert!(text.contains("relocation_all_steps_v28"));
                drop(text);
            });
            let used = budget.work();
            let peak = budget.peak_storage();
            // This test owns every compiler-only value constructed after floor.
            // They have all dropped before their accepted reservations settle.
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
fn relocation_cfg_function_relations_never_read_foreign_function_slots() {
    let module = zero_operand_functions(4);
    with_pair_module(&module, 4, |pair, budget| {
        let floor = budget.storage();
        let (input, receipt) = Inventory::derive_v18(pair.input(), budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let text = generated(pair, budget).unwrap();
        for function in input.functions() {
            for block in function.blocks.clone() {
                let cut = text
                    .split(&format!("spec fn relocation_cut_{block}_v28"))
                    .nth(1)
                    .unwrap()
                    .split("\n}\n")
                    .next()
                    .unwrap();
                for foreign in input
                    .functions()
                    .iter()
                    .filter(|other| other.coordinate != function.coordinate)
                {
                    for definition in foreign.definitions.clone() {
                        assert!(!cut.contains(&format!("base[{definition}]")));
                    }
                }
            }
        }
        let retained = text.retained;
        drop((text, input));
        budget
            .release_storage(retained + receipt.retained_storage())
            .unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn relocation_cfg_disconnected_blocks_are_not_execution_cut_assumptions() {
    let mut module = fixture(false);
    module.functions[0]
        .body
        .as_mut()
        .unwrap()
        .blocks
        .push(block(99, Terminator::Return { values: vec![] }));
    with_pair_module(&module, 2, |pair, budget| {
        let floor = budget.storage();
        let text = generated(pair, budget).unwrap();
        let cut = text
            .split("spec fn relocation_cut_4_v28")
            .nth(1)
            .unwrap()
            .split("\n}\n")
            .next()
            .unwrap();
        assert!(cut.contains("&& false"));
        assert!(text.contains("proof fn relocation_block_step_4_v28"));
        let retained = text.retained;
        drop(text);
        budget.release_storage(retained).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn relocation_cfg_generator_rejects_equal_byte_foreign_input_inventory() {
    with_pair(false, |pair, budget| {
        let floor = budget.storage();
        let (foreign, foreign_storage) =
            Owner::from_module_ref_with_verification_budget_v18(&fixture(false), LAYOUTS, budget)
                .unwrap();
        budget
            .reserve_storage(foreign_storage.retained_storage())
            .unwrap();
        assert_eq!(foreign.identity(), pair.input().identity());
        let (input, input_storage) = Inventory::derive_v18(&foreign, budget).unwrap();
        budget
            .reserve_storage(input_storage.retained_storage())
            .unwrap();
        let (output, output_storage) = Inventory::derive_v18(pair.output(), budget).unwrap();
        budget
            .reserve_storage(output_storage.retained_storage())
            .unwrap();
        budget.reserve_storage(2 * SOURCE_LIMIT).unwrap();
        let mut writer = Writer::new(budget).unwrap();
        assert!(matches!(
            semantics::generate_relocation_cfg_v28(&input, &output, pair, &mut writer),
            Err(ProofError::Statement(
                "exact complete relocation graph pair"
            ))
        ));
        drop(writer);
        drop(input);
        drop(output);
        drop(foreign);
        budget.release_storage(budget.storage() - floor).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}
