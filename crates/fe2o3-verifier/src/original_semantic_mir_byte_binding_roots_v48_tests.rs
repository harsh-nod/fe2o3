use super::*;
use fe2o3_kernel_analysis::CanonicalKirFunctionRefV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};

const LIMIT: usize = 256 * 1024 * 1024;

fn run(inspect: impl FnOnce(&SourceByteBindings<'_, '_, '_>, &mut Writer<'_, '_>) -> Result<()>) {
    super::super::super::invocations::tests::run_source_transform(
        LIMIT,
        LIMIT,
        |types, functions| {
            super::super::paired::aggregate_tests::call_transform(types, functions, true)
        },
        |plan, out| {
            super::super::source_function::tests::with_slots(plan, out, |slots, out| {
                let bindings = SourceByteBindings::derive(slots, out)?;
                inspect(&bindings, out)
            })
        },
    )
    .0
    .unwrap();
}

#[test]
fn original_byte_bindings_keep_genuine_inlined_imports_out_of_the_root_census() {
    run(|bindings, out| {
        let relation = bindings.slots.correspondence(out)?;
        let inventory = relation.inventory(out.budget)?;
        assert_eq!(bindings.roots.len(), 2);
        assert_eq!(inventory.functions().len(), 3);
        let mut imported = inventory
            .functions()
            .iter()
            .filter(|row| row.function.role == FunctionRole::ExternalImport);
        let imported_function = imported.next().unwrap();
        assert!(imported.next().is_none());
        assert!(imported_function.function.body.is_none());
        assert!(imported_function.operations.is_empty());
        assert_eq!(
            bindings.rows.len(),
            inventory
                .operations()
                .iter()
                .filter(|row| matches!(row.operation.kind, OperationKind::Alloca { .. }))
                .count()
        );
        bindings.emit(out)?;
        for root in 0..2 {
            assert!(out.text.contains(&format!(
                "open spec fn invocation_source_byte_map_valid_{root}_v36"
            )));
        }
        assert!(!out.text.contains("invocation_source_byte_map_valid_2_v36"));
        Ok(())
    });
}

// Census-only hostile views do not enter an owner constructor or grant source
// authority. The positive above derives the complete genuine source consumer.
fn copied_view<'a>(row: &CanonicalKirFunctionRefV1<'a>) -> CanonicalKirFunctionRefV1<'a> {
    CanonicalKirFunctionRefV1 {
        coordinate: row.coordinate,
        function: row.function,
        blocks: row.blocks.clone(),
        definitions: row.definitions.clone(),
        operations: row.operations.clone(),
        uses: row.uses.clone(),
        edges: row.edges.clone(),
        edge_arguments: row.edge_arguments.clone(),
        effects: row.effects.clone(),
        calls: row.calls.clone(),
    }
}

#[test]
fn original_byte_binding_census_rejects_missing_extra_or_executable_import_roots() {
    run(|bindings, out| {
        let inventory = bindings.slots.correspondence(out)?.inventory(out.budget)?;
        let functions = inventory.functions();
        assert_eq!(functions.len(), 3);
        let mut selected = std::array::from_fn::<_, 3, _>(|index| {
            functions[index].function.role == FunctionRole::KernelEntry
        });
        check_root_census(functions, &selected, out)?;
        let kernel = selected.iter().position(|value| *value).unwrap();
        let imported = selected.iter().position(|value| !value).unwrap();
        for (index, value) in [(kernel, false), (imported, true)] {
            let old = selected[index];
            selected[index] = value;
            assert!(matches!(
                check_root_census(functions, &selected, out),
                Err(Error::Statement(
                    "original MIR byte map differs from its authenticated Alloca results"
                ))
            ));
            selected[index] = old;
        }
        let mut altered = std::array::from_fn::<_, 3, _>(|index| copied_view(&functions[index]));
        // A third executable entry is not covered by the two authenticated roots.
        let extra = [
            copied_view(&functions[0]),
            copied_view(&functions[1]),
            copied_view(&functions[2]),
            copied_view(&functions[kernel]),
        ];
        let extra_selected = [selected[0], selected[1], selected[2], false];
        assert!(check_root_census(&extra, &extra_selected, out).is_err());
        // A declaration label cannot hide physical instructions.
        altered[imported].operations = functions[kernel].operations.clone();
        assert!(!altered[imported].operations.is_empty());
        assert!(check_root_census(&altered, &selected, out).is_err());
        altered[imported] = copied_view(&functions[imported]);
        altered[imported].function = functions[kernel].function;
        assert!(check_root_census(&altered, &selected, out).is_err());
        assert!(check_root_census(functions, &selected[..2], out).is_err());
        check_root_census(functions, &selected, out)
    });
}

#[test]
fn original_byte_binding_root_census_has_independent_exact_query_resources() {
    run(|bindings, out| {
        let inventory = bindings.slots.correspondence(out)?.inventory(out.budget)?;
        let functions = inventory.functions();
        assert_eq!(functions.len(), 3);
        let selected = std::array::from_fn::<_, 3, _>(|index| {
            functions[index].function.role == FunctionRole::KernelEntry
        });
        let exact_work = 1 + 12 * functions.len();
        use crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT;
        for limit in [exact_work, exact_work - 1] {
            let mut work = Work::new(limit);
            let mut budget = Budget::new(&mut work, SOURCE_LIMIT);
            budget.reserve_storage(SOURCE_LIMIT)?;
            let result = {
                let mut writer = Writer::new(&mut budget)?;
                check_root_census(functions, &selected, &mut writer)
            };
            if limit == exact_work {
                result?;
                assert_eq!(budget.work(), exact_work);
            } else {
                assert!(matches!(result, Err(Error::Resource(Resource::Work(error)))
                    if error.limit() == limit && error.actual() == exact_work));
            }
            assert_eq!(budget.storage(), SOURCE_LIMIT);
            assert_eq!(budget.peak_storage(), SOURCE_LIMIT);
        }
        Ok(())
    });
}
