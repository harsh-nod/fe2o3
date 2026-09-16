struct SourceOutputFormalModuleKernelV1<'a> {
    kernel: &'a fe2o3_kernel_ir::Kernel,
    ordinal: usize,
    claimed: bool,
}

#[cfg(test)]
mod formal_module_index_tests {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work, Kernel, LaunchDomain, LaunchExtent, Module,
    };

    // Inert key rows only, deliberately independent from a completed scope.
    fn component() -> Module {
        let mut module = Module::new("formal-module-index");
        let domain = LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        };
        module.kernels = vec![
            Kernel::new("b", "eb", domain.clone()),
            Kernel::new("a", "ea", domain),
        ];
        module
    }

    #[test]
    fn formal_module_index_joins_reordered_rows_and_refuses_wrong_or_repeated_keys() {
        let module = component();
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        budget.reserve_storage(97).unwrap();
        let mut rows = source_output_formal_module_index_v1(&module, &mut budget).unwrap();
        for (export, entry) in [(b"a".as_slice(), b"eb".as_slice()), (b"c", b"ea")] {
            assert!(matches!(
                source_output_formal_module_claim_v1(&mut rows, export, entry, &mut budget),
                Err(ProductionSourceOutputErrorV1::Invalid(
                    "formal module source-export/entry binding absent"
                ))
            ));
        }
        assert_eq!(
            source_output_formal_module_claim_v1(&mut rows, b"a", b"ea", &mut budget).unwrap(),
            1
        );
        assert_eq!(
            source_output_formal_module_claim_v1(&mut rows, b"b", b"eb", &mut budget).unwrap(),
            0
        );
        assert!(matches!(
            source_output_formal_module_claim_v1(&mut rows, b"a", b"ea", &mut budget),
            Err(ProductionSourceOutputErrorV1::Invalid(
                "formal module output kernel claimed twice"
            ))
        ));
        assert!(rows.iter().all(|row| row.claimed));
        drop(rows);
        budget.release_storage(budget.storage() - 97).unwrap();
        assert_eq!(budget.storage(), 97);
    }

    #[test]
    fn formal_module_index_refuses_duplicate_output_bindings() {
        let mut module = component();
        module.kernels.push(module.kernels[0].clone());
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        assert!(matches!(
            source_output_formal_module_index_v1(&module, &mut budget),
            Err(ProductionSourceOutputErrorV1::Invalid(
                "formal module kernel binding duplicated"
            ))
        ));
        budget.release_storage(budget.storage()).unwrap();
    }

    #[test]
    fn formal_module_index_exact_bounds_preserve_cumulative_prefixes() {
        const PREFIX: usize = 97;
        const HISTORY: usize = 7;
        // Keys contain 1+2 bytes; a full comparison costs 7. Index: entry1,
        // visits2, growth1, pushes2, heap11, adjacent7 =24. Claim a: two
        // (lookup1+key7), mark2 =18. Claim b: lookup1+key7+mark2 =10.
        const INDEX: usize = 24;
        const QUERY: usize = INDEX + 18 + 10;
        let header = std::mem::size_of::<Vec<SourceOutputFormalModuleKernelV1<'_>>>();
        // Existing origin-vector growth requests a minimum of four rows.
        let payload = 4 * std::mem::size_of::<SourceOutputFormalModuleKernelV1<'_>>();
        let module = component();
        for (work_limit, storage_limit, accepted, attempted, storage_denied) in [
            (
                HISTORY + QUERY,
                PREFIX + header + payload,
                HISTORY + QUERY,
                None,
                None,
            ),
            (
                HISTORY + QUERY - 1,
                PREFIX + header + payload,
                HISTORY + QUERY - 2,
                Some(HISTORY + QUERY),
                None,
            ),
            (
                HISTORY + QUERY,
                PREFIX + header + payload - 1,
                HISTORY + 3,
                None,
                Some(PREFIX + header + payload),
            ),
        ] {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.charge_work(HISTORY).unwrap();
            budget.reserve_storage(PREFIX).unwrap();
            let result = (|| {
                budget
                    .reserve_storage(header)
                    .map_err(ProductionSourceOutputErrorV1::Resource)?;
                let mut rows = source_output_formal_module_index_v1(&module, &mut budget)?;
                assert_eq!(
                    source_output_formal_module_claim_v1(&mut rows, b"a", b"ea", &mut budget)?,
                    1
                );
                assert_eq!(
                    source_output_formal_module_claim_v1(&mut rows, b"b", b"eb", &mut budget)?,
                    0
                );
                Ok::<_, ProductionSourceOutputErrorV1>(())
            })();
            assert_eq!(
                result.is_ok(),
                attempted.is_none() && storage_denied.is_none()
            );
            assert_eq!(budget.work(), accepted);
            assert_eq!(budget.failed_storage(), storage_denied);
            budget.release_storage(budget.storage() - PREFIX).unwrap();
            assert_eq!(budget.storage(), PREFIX);
            assert_eq!(work.failed_work(), attempted);
        }
    }
}

fn source_output_formal_module_key_work_v1(
    left: (&[u8], &[u8]),
    right: (&[u8], &[u8]),
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<std::cmp::Ordering, SemanticKirAssertOriginErrorV1> {
    let work = left
        .0
        .len()
        .checked_add(left.1.len())
        .and_then(|n| n.checked_add(right.0.len()))
        .and_then(|n| n.checked_add(right.1.len()))
        .and_then(|n| n.checked_add(1))
        .ok_or(SemanticKirAssertOriginErrorV1::Resource(
            AssertOriginResourceV1::Arithmetic,
        ))?;
    budget.charge_work(work)?;
    Ok(left.cmp(&right))
}

fn source_output_formal_module_key_v1<'a>(
    row: &SourceOutputFormalModuleKernelV1<'a>,
) -> (&'a [u8], &'a [u8]) {
    (
        row.kernel.id.as_str().as_bytes(),
        row.kernel.entry.as_str().as_bytes(),
    )
}

// Borrowed lookup data only. The completed-analysis caller supplies the actual
// checked O; tests may exercise malformed inert rows without minting a proof.
fn source_output_formal_module_index_v1<'a>(
    module: &'a fe2o3_kernel_ir::Module,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<Vec<SourceOutputFormalModuleKernelV1<'a>>, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    budget.charge_work(1).map_err(Error::Resource)?;
    let mut rows = Vec::new();
    for (ordinal, kernel) in module.kernels.iter().enumerate() {
        budget.charge_work(1).map_err(Error::Resource)?;
        assert_origin_push_v1(
            &mut rows,
            SourceOutputFormalModuleKernelV1 {
                kernel,
                ordinal,
                claimed: false,
            },
            budget,
        )
        .map_err(Error::SourceOrigin)?;
    }
    assert_origin_sort_v1(&mut rows, budget, |left, right, budget| {
        source_output_formal_module_key_work_v1(
            source_output_formal_module_key_v1(left),
            source_output_formal_module_key_v1(right),
            budget,
        )
    })
    .map_err(Error::SourceOrigin)?;
    for pair in rows.windows(2) {
        if source_output_formal_module_key_work_v1(
            source_output_formal_module_key_v1(&pair[0]),
            source_output_formal_module_key_v1(&pair[1]),
            budget,
        )
        .map_err(Error::SourceOrigin)?
        .is_eq()
        {
            return Err(Error::Invalid("formal module kernel binding duplicated"));
        }
    }
    Ok(rows)
}

fn source_output_formal_module_claim_v1(
    rows: &mut [SourceOutputFormalModuleKernelV1<'_>],
    export: &[u8],
    entry: &[u8],
    budget: &mut AssertOriginBudgetV1<'_>,
) -> Result<usize, ProductionSourceOutputErrorV1> {
    use ProductionSourceOutputErrorV1 as Error;
    let found = assert_origin_find_v1(rows, budget, |row, budget| {
        source_output_formal_module_key_work_v1(
            source_output_formal_module_key_v1(row),
            (export, entry),
            budget,
        )
    })
    .map_err(Error::SourceOrigin)?
    .ok_or(Error::Invalid(
        "formal module source-export/entry binding absent",
    ))?;
    budget.charge_work(2).map_err(Error::Resource)?;
    if std::mem::replace(&mut rows[found].claimed, true) {
        return Err(Error::Invalid("formal module output kernel claimed twice"));
    }
    Ok(rows[found].ordinal)
}

/// Derives fresh Complete-only obligations for the entire kernel roster of the
/// SAME actual O held by the complete source/Store/control result collection.
/// No raw output, kernel index or obligation is accepted from the caller.
///
/// Source order and actual kernel order are joined by full export/entry keys,
/// not function ordinal equality. New join scratch uses the original ledger;
/// the formal engine retains its separate historical resource domain. This is
/// not a final owner, reference proof, physical-address discharge, concrete
/// launch admission or termination/convergence evidence. Runtime bounds and
/// alias requirements remain obligations. No borrowed result escapes callback.
pub fn with_complete_formal_memory_module_v1<'scope, 'source, 'output, R>(
    analyses: &[ProductionScopedCanonicalStoreAnalysisV1<'scope, 'source, 'output>],
    budget: &mut AssertOriginBudgetV1<'_>,
    next: impl for<'formal> FnOnce(
        &[ProductionScopedCompleteFormalMemoryV1<'formal, 'scope, 'source, 'output>],
        &mut AssertOriginBudgetV1<'_>,
    ) -> Result<R, ProductionScopedFormalMemoryErrorV1>,
) -> Result<R, ProductionScopedFormalMemoryErrorV1> {
    use ProductionScopedFormalMemoryErrorV1 as Error;
    let source_error = Error::SourceOutput;
    budget
        .charge_work(3)
        .map_err(|e| source_error(ProductionSourceOutputErrorV1::Resource(e)))?;
    let first = analyses.first().ok_or_else(|| {
        source_error(ProductionSourceOutputErrorV1::Invalid(
            "formal module source roster empty",
        ))
    })?;
    first.require_live_v1(budget).map_err(source_error)?;
    let view = first.view();
    let output = first.output();
    if analyses.len() != view.source().source_launch().roots().len()
        || analyses.len() != output.module().kernels.len()
    {
        return Err(source_error(ProductionSourceOutputErrorV1::Invalid(
            "formal module source/output kernel roster differs",
        )));
    }
    let floor = budget.storage();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let resource = |e| source_error(ProductionSourceOutputErrorV1::Resource(e));
        budget.charge_work(1).map_err(resource)?;
        let header = std::mem::size_of::<Vec<SourceOutputFormalModuleKernelV1<'_>>>()
            + std::mem::size_of::<Vec<usize>>()
            + std::mem::size_of::<Vec<fe2o3_kernel_ir::FormalMemoryObligations>>()
            + std::mem::size_of::<Vec<ProductionScopedCompleteFormalMemoryV1<'_, '_, '_, '_>>>();
        budget.reserve_storage(header).map_err(resource)?;
        let mut index =
            source_output_formal_module_index_v1(output.module(), budget).map_err(source_error)?;
        let mut kernel_order = Vec::new();
        for (analysis, root) in analyses.iter().zip(view.source().source_launch().roots()) {
            budget.charge_work(5).map_err(resource)?;
            if !std::ptr::eq(analysis.view(), view)
                || !std::ptr::eq(analysis.output(), output)
                || analysis.selected_root() != root.selected_root()
            {
                return Err(source_error(ProductionSourceOutputErrorV1::Invalid(
                    "formal module mixed owner or reordered source root",
                )));
            }
            let function = analysis.output_function_v1(budget).map_err(source_error)?;
            let actual = output
                .module()
                .functions
                .get(function.0 as usize)
                .ok_or_else(|| {
                    source_error(ProductionSourceOutputErrorV1::Invalid(
                        "formal module output function absent",
                    ))
                })?;
            if actual.role != fe2o3_kernel_ir::FunctionRole::KernelEntry || actual.body.is_none() {
                return Err(source_error(ProductionSourceOutputErrorV1::Invalid(
                    "formal module output function is not a defined entry",
                )));
            }
            let source = view
                .source()
                .semantic_ssa()
                .source_semantic()
                .functions()
                .get(root.selected_root().index() as usize)
                .ok_or_else(|| {
                    source_error(ProductionSourceOutputErrorV1::Invalid(
                        "formal module source root absent",
                    ))
                })?;
            let source_entry = source.kernel_entry().ok_or_else(|| {
                source_error(ProductionSourceOutputErrorV1::Invalid(
                    "formal module source export absent",
                ))
            })?;
            budget.charge_work(4).map_err(resource)?;
            if source.role() != SemanticFunctionRoleV1::KernelRoot
                || source.identity() != root.semantic_root_identity()
                || source_entry.kernel_binding_identity().as_bytes() != &root.kernel_binding()
            {
                return Err(source_error(ProductionSourceOutputErrorV1::Invalid(
                    "formal module source root metadata differs",
                )));
            }
            let ordinal = source_output_formal_module_claim_v1(
                &mut index,
                source_entry.export_symbol().as_bytes(),
                actual.id.as_str().as_bytes(),
                budget,
            )
            .map_err(source_error)?;
            if output.module().kernels[ordinal].domain.rank() != root.source_rank() {
                return Err(source_error(ProductionSourceOutputErrorV1::Invalid(
                    "formal module output launch rank differs",
                )));
            }
            assert_origin_push_v1(&mut kernel_order, ordinal, budget)
                .map_err(|e| source_error(ProductionSourceOutputErrorV1::SourceOrigin(e)))?;
        }
        for row in &index {
            budget.charge_work(1).map_err(resource)?;
            if !row.claimed {
                return Err(source_error(ProductionSourceOutputErrorV1::Invalid(
                    "formal module output kernel is unclaimed",
                )));
            }
        }
        let mut obligations = Vec::new();
        for &ordinal in &kernel_order {
            budget.charge_work(1).map_err(resource)?;
            let row = crate::production_formal_memory_v1::derive_complete_output_formal_kernel_v1(
                output, ordinal,
            )
            .map_err(Error::Formal)?
            .ok_or_else(|| {
                source_error(ProductionSourceOutputErrorV1::Invalid(
                    "formal module joined kernel absent",
                ))
            })?;
            assert_origin_push_v1(&mut obligations, row, budget)
                .map_err(|e| source_error(ProductionSourceOutputErrorV1::SourceOrigin(e)))?;
        }
        let mut complete = Vec::new();
        for ((analysis, &ordinal), obligations) in
            analyses.iter().zip(&kernel_order).zip(&obligations)
        {
            budget.charge_work(1).map_err(resource)?;
            assert_origin_push_v1(
                &mut complete,
                ProductionScopedCompleteFormalMemoryV1 {
                    analysis,
                    kernel: &output.module().kernels[ordinal],
                    obligations,
                },
                budget,
            )
            .map_err(|e| source_error(ProductionSourceOutputErrorV1::SourceOrigin(e)))?;
        }
        let value = next(&complete, budget)?;
        for analysis in analyses {
            analysis.require_live_v1(budget).map_err(source_error)?;
        }
        Ok(value)
    }));
    let cleanup = budget
        .storage()
        .checked_sub(floor)
        .ok_or(AssertOriginResourceV1::Accounting)
        .and_then(|bytes| budget.release_storage(bytes))
        .map_err(|e| source_error(ProductionSourceOutputErrorV1::Resource(e)));
    match outcome {
        Ok(result) => {
            cleanup?;
            result
        }
        Err(payload) => {
            cleanup?;
            std::panic::resume_unwind(payload)
        }
    }
}
