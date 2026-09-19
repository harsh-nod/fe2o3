use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

// These existing scalar fixtures test argument identity, not output coverage.
fn resolve(
    owner: &ProductionPreRankedKirOwnerV1,
    slot: usize,
    value: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<ConditionalOutputArgumentV1>, ProductionSemanticKirErrorV1> {
    let root = SemanticFunctionIdV1::from_index(1);
    owner.with_checked_arguments_v1(root, root, budget, |view| {
        conditional_output_argument_v1(view, slot, value)
    })
}

fn parameters(owner: &ProductionPreRankedKirOwnerV1) -> &[ValueId] {
    let row = owner
        .correspondence
        .lowered_functions()
        .iter()
        .find(|row| {
            row.correspondence_owner().index() == 1
                && row.role() == SemanticKirFunctionRoleV1::KernelEntry
        })
        .unwrap();
    &owner
        .executable()
        .module()
        .function(row.kernel_ir_function())
        .unwrap()
        .body
        .as_ref()
        .unwrap()
        .parameters
}

#[test]
fn whole_argument_resolution_rejects_component_projections_and_wrong_values() {
    let owner = argument_view_owner(false, ArgumentTupleShape::Mixed);
    let mut work = Work::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
    budget.reserve_storage(19).unwrap();
    let values = parameters(&owner);
    assert_eq!(values.len(), 5);
    let whole = resolve(&owner, 0, values[0], &mut budget).unwrap().unwrap();
    assert_eq!(
        (whole.source, whole.adjusted, whole.local.index(), whole.ty),
        (0, 0, 1, U32)
    );
    for (slot, &value) in values.iter().enumerate().skip(1) {
        assert_eq!(resolve(&owner, slot, value, &mut budget).unwrap(), None);
    }
    for (slot, value) in [(0, values[1]), (5, values[0]), (usize::MAX, values[0])] {
        assert!(matches!(
            resolve(&owner, slot, value, &mut budget),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        assert_eq!(budget.storage(), 19);
    }
}

fn shifted_source() -> ProductionSemanticSsaOwnerV1 {
    let original = argument_owner_shape(false, ArgumentTupleShape::Mixed, true, true);
    let semantic = original.source_semantic();
    let functions = semantic
        .functions()
        .iter()
        .map(|function| {
            if function.role() != SemanticFunctionRoleV1::KernelRoot {
                return function.clone();
            }
            let mut arguments = function.abi().arguments().to_vec();
            arguments.swap(0, 3);
            let abi = SemanticFunctionAbiV1::from_rustc(
                function.abi().identity(),
                semantic.target().identity(),
                SemanticCanonAbiV1::GpuKernel,
                SemanticExternAbiV1::GpuKernel,
                false,
                false,
                4,
                arguments,
                SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
            )
            .unwrap()
            .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 4])
            .unwrap();
            let locals = function
                .locals()
                .iter()
                .map(|local| {
                    let role = match local.role() {
                        SemanticLocalRoleV1::Argument(0) => SemanticLocalRoleV1::Argument(3),
                        SemanticLocalRoleV1::Argument(3) => SemanticLocalRoleV1::Argument(0),
                        other => other,
                    };
                    SemanticLocalDeclV1::new(local.identity(), local.ty(), role, local.source())
                })
                .collect();
            SemanticFunctionDeclV1::new(
                function.identity(),
                function.role(),
                function.item_definition_identity(),
                function.monomorphization_identity(),
                function.generic_type_arguments_identity(),
                function.const_generic_arguments_identity(),
                function.source(),
                abi,
                locals,
                function.entry(),
                function.blocks().to_vec(),
            )
            .unwrap()
            .with_kernel_entry(function.kernel_entry().unwrap().clone())
        })
        .collect();
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

#[test]
fn source_ordinal_does_not_follow_erased_or_expanded_physical_slots() {
    let owner = materialize_argument_view(shifted_source());
    let mut work = Work::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
    let values = parameters(&owner);
    let whole = resolve(&owner, 4, values[4], &mut budget).unwrap().unwrap();
    assert_eq!(
        (whole.source, whole.adjusted, whole.local.index(), whole.ty),
        (3, 3, 1, U32)
    );
    assert_ne!(whole.source, 4);
    for (slot, &value) in values.iter().enumerate().take(4) {
        assert_eq!(resolve(&owner, slot, value, &mut budget).unwrap(), None);
    }
    assert_eq!(budget.storage(), 0);
}

#[test]
fn output_argument_join_uses_the_callers_work_and_storage_budget() {
    let owner = argument_view_owner(false, ArgumentTupleShape::Mixed);
    let value = parameters(&owner)[0];
    let floor = 23;
    let mut work = Work::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(floor).unwrap();
    let expected = resolve(&owner, 0, value, &mut budget).unwrap();
    let exact_work = budget.work();
    let peak = budget.peak_storage();
    assert!(peak > floor);
    assert_eq!(budget.storage(), floor);
    for (work_limit, storage_limit, accepts) in [
        (exact_work, peak, true),
        (exact_work - 1, peak, false),
        (exact_work, peak - 1, false),
        (exact_work, floor, false),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = resolve(&owner, 0, value, &mut budget);
        if accepts {
            assert_eq!(result.unwrap(), expected);
            assert_eq!(budget.work(), exact_work);
        } else {
            assert!(
                matches!(
                    result,
                    Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(_))
                ),
                "{result:?}"
            );
        }
        assert_eq!(budget.storage(), floor);
        assert!(ledger == budget.work_ledger_identity_v1());
    }
}
