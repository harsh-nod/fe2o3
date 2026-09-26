use super::*;

pub(super) fn native_header(
    recipes: &ProductionOptimizedExecutionRecipesV18<'_>,
    pending: &mut PendingCanonicalRankedSourceRolesV18<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
    limit: usize,
    short: bool,
    verified: &std::cell::Cell<bool>,
) -> NativeResult {
    let diagnostic = DiagnosticCell::new(None);
    recipes.with_pending_native_policies_v18(
        pending,
        budget,
        &diagnostic,
        |policies, budget| {
            assert!(policies.function_count(budget)? > 0);
            Ok(())
        },
    )?;
    diagnostic.set(None);
    // Independently mirror the fixed native-join constructor, including the
    // two one-byte alignments of its zero-sized consumer. Exact header leaves
    // zero bytes for the first retained output recipe backing allocation.
    let header = size_of::<Vec<OutputRecipe>>()
        + size_of::<ProductionLifecycleCheckedNativePoliciesV18<'_, '_>>()
        + 2 * size_of::<SourceOwnedResultV18<Vec<OutputRecipe>>>()
        + size_of::<std::thread::Result<NativeResult>>()
        + 2 * size_of::<NativeResult>()
        + 2;
    let retained = recipes
        .rows
        .iter()
        .filter(|row| row.output.is_some())
        .count();
    assert!(retained > 0);
    let floor = budget.storage();
    let padding = limit - floor - header + usize::from(short);
    budget.reserve_storage(padding).unwrap();
    let work = budget.work();
    let result = recipes.with_pending_native_policies_v18(
        pending,
        budget,
        &diagnostic,
        |_, _| -> NativeResult { panic!("header/backing cut reached native consumer") },
    );
    let expected = limit
        + if short {
            1
        } else {
            retained * size_of::<OutputRecipe>()
        };
    assert!(
        matches!(&result, Err(NativeError::Source(ProductionSourceOwnedViewErrorV18::Resource(
        ArgumentResourceV1::Storage(error)))) if error.actual() == expected)
    );
    assert_eq!(budget.failed_storage(), Some(expected));
    assert_eq!(
        budget.work() - work,
        if short { 0 } else { recipes.rows.len() + 1 }
    );
    assert!(diagnostic.get().is_none());
    assert_eq!(budget.storage(), floor + padding);
    budget.release_storage(padding).unwrap();
    assert_eq!(budget.storage(), floor);
    verified.set(true);
    result
}

pub(super) fn same_candidate(
    recipes: &ProductionOptimizedExecutionRecipesV18<'_>,
    pending: &mut PendingCanonicalRankedSourceRolesV18<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
    fault: u8,
    verified: &std::cell::Cell<bool>,
) -> NativeResult {
    let entered = std::cell::Cell::new(false);
    let diagnostic = DiagnosticCell::new(None);
    recipes.with_pending_native_policies_v18(
        pending,
        budget,
        &diagnostic,
        |policies, budget| {
            assert!(policies.lifecycle_source_roles_are_complete());
            for function in 0..policies.function_count(budget)? {
                if let Some(report) = policies.report(function, budget)? {
                    assert!(report.is_clean());
                    assert!(policies.history(function, budget)?.is_some());
                }
            }
            entered.set(true);
            Ok(())
        },
    )?;
    assert!(
        entered.get(),
        "same retained graph must pass the actual consumer before tampering"
    );
    let owner = pending
        .owner(budget)
        .map_err(|error| recipes.pending_error(error))?;
    let original = pending
        .obligations(budget)
        .map_err(|error| recipes.pending_error(error))?;
    let floor = budget.storage();
    budget
        .reserve_storage(2 * size_of::<Vec<usize>>())
        .map_err(|error| recipes.pending_error(error.into()))?;
    let mut rows = recipes.output_recipes(budget)?;
    let mut obligations = resources::vector(original.len() + 1, budget)?;
    for row in original {
        budget
            .charge_work(1)
            .map_err(|error| recipes.pending_error(error.into()))?;
        obligations.push(*row);
    }
    recipes.join_pending(owner, &obligations, &rows, budget)?;
    assert!(rows.len() >= 3);
    let mut foreign = None;
    match fault {
        0 => {
            rows.remove(0);
        }
        1 => {
            let mut duplicate = resources::vector(rows.len() + 1, budget)?;
            duplicate.push(rows[0]);
            duplicate.extend(rows.iter().copied());
            rows = duplicate;
        }
        2 => {
            rows[0].kind = ProductionOptimizedExecutionKindV18::ScopeEnd;
        }
        3 => {
            obligations.pop();
        }
        4 => {
            obligations.insert(0, obligations[0]);
        }
        5 => {
            let layouts = recipes
                .optimized
                .original
                .source
                .limits(budget)?
                .storage_layout_limits();
            let (other, receipt) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
                owner.module(), layouts, budget,
            ).unwrap();
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert_eq!(other.canonical_bytes(), owner.canonical_bytes());
            foreign = Some(other);
        }
        _ => panic!("unknown native join control"),
    }
    let result = recipes.join_pending(
        foreign.as_ref().unwrap_or(owner),
        &obligations,
        &rows,
        budget,
    );
    let expected = match fault {
        0 => "native lifecycle missing output recipe",
        1 | 3 => "native lifecycle missing output obligation",
        2 => "native lifecycle output operation changed kind",
        4 => "native lifecycle repeated or unordered obligation",
        5 => "native lifecycle substituted its checked output owner",
        _ => unreachable!(),
    };
    assert!(
        matches!(&result, Err(NativeError::Source(ProductionSourceOwnedViewErrorV18::Binding(detail))) if *detail == expected),
        "fault={fault}: {result:?}"
    );
    drop((foreign, obligations, rows));
    assert!(!recipes.optimized.original.source.cleanup.is_denied());
    budget.release_storage(budget.storage() - floor).unwrap();
    verified.set(true);
    result
}
