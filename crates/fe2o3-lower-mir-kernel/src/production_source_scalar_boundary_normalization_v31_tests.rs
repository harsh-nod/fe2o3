use super::*;

#[test]
fn source_scalar_normalization_uses_checked_phi_before_unique_origin() {
    with_policy11(private_entry_phi_owner_v20, |original, optimized, budget| {
        original.with_optimized_scalar_leaf_namespace_v18(
            optimized,
            0,
            &SourceScalarNamespaceV18::PrivateSourceWritesV22,
            budget,
            |leaves, budget| {
                with_check(leaves, budget, |check, budget| {
                    let row = check.leaves.boundary_find_v31([0, 0, 3, 2], budget)?.unwrap();
                    let definition = check.definition(row, budget)?;
                    let actual = check.inventory.definitions()[definition].value.unwrap();
                    assert_eq!(check.origins.value_origin(actual, budget).unwrap(), None);
                    let expression = ProductionSemanticExpressionV2::Symbol {
                        symbol: row.symbol,
                        scalar: row.scalar,
                    };
                    check.normalize(&expression, actual, budget)?;
                    let wrong = ProductionSemanticExpressionV2::Symbol {
                        symbol: PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2,
                        scalar: row.scalar,
                    };
                    assert!(matches!(check.normalize(&wrong, actual, budget),
                        Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "actual optimized scalar expression differs from its original source value"))));
                    check.normalize(&expression, actual, budget)?;
                    let foreign = &check.inventory.functions()[1];
                    assert!(matches!(leaves.read(foreign.function, actual, budget),
                        Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "optimized scalar read foreign output function"))));
                    check.normalize(&expression, actual, budget)
                })
            },
        )
    }).unwrap();
}

#[test]
fn source_scalar_normalization_refuses_unenrolled_phi_and_keeps_single_origin_fallback() {
    with_policy11(private_entry_phi_owner_v20, |original, optimized, budget| {
        original.with_optimized_scalar_leaf_namespace_v18(
            optimized,
            0,
            &SourceScalarNamespaceV18::PrivateSourceWritesV22,
            budget,
            |leaves, budget| {
                source_scalar_normalization_scratch_v18(
                    original.source.cleanup,
                    budget,
                    optimized_source_boundary_headers_v31()?,
                    |budget| {
                        let mut boundaries = optimized_source_scalar_boundaries_v31(
                            leaves.original, optimized, budget)?;
                        boundaries.values.clear();
                        let altered = ProductionOptimizedSourceScalarLeavesV18 {
                            original: leaves.original,
                            optimized,
                            function: leaves.function,
                            reads: leaves.reads,
                            wrapping: leaves.wrapping,
                            boundaries: &boundaries,
                            slot: leaves.slot,
                            ledger: leaves.ledger,
                            floor: leaves.floor,
                        };
                        let result = with_check(&altered, budget, |check, budget| {
                            let row = check.leaves.boundary_find_v31([0, 0, 3, 2], budget)?.unwrap();
                            let actual = leaves.boundaries.values.iter()
                                .find(|value| check.leaves.boundaries.rows[value.original].definition == row.definition)
                                .unwrap().value;
                            assert_eq!(altered.read(leaves.function.function, actual, budget)?, None);
                            assert_eq!(check.origins.value_origin(actual, budget).unwrap(), None);
                            let expression = ProductionSemanticExpressionV2::Symbol {
                                symbol: row.symbol,
                                scalar: row.scalar,
                            };
                            assert!(matches!(check.normalize(&expression, actual, budget),
                                Err(ProductionSourceOwnedViewErrorV18::Binding(
                                    "actual optimized scalar expression differs from its original source value"))));
                            let argument = leaves.function.function.body.as_ref().unwrap().parameters[0];
                            assert_eq!(altered.read(leaves.function.function, argument, budget)?, None);
                            assert_eq!(check.origins.value_origin(argument, budget).unwrap(), Some(argument));
                            check.normalize(&ProductionSemanticExpressionV2::Symbol {
                                symbol: PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2,
                                scalar: row.scalar,
                            }, argument, budget)
                        });
                        drop(altered);
                        drop(boundaries);
                        result
                    },
                )?;
                with_check(leaves, budget, |check, budget| {
                    let row = check.leaves.boundary_find_v31([0, 0, 3, 2], budget)?.unwrap();
                    let actual = check.inventory.definitions()[check.definition(row, budget)?].value.unwrap();
                    check.normalize(&ProductionSemanticExpressionV2::Symbol {
                        symbol: row.symbol,
                        scalar: row.scalar,
                    }, actual, budget)
                })
            },
        )
    }).unwrap();
}
