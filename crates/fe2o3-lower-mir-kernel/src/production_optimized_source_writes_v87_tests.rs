pub(super) fn test_optimized_writes_v87(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    expected: usize,
    fault: Option<u8>,
    reached: &std::cell::Cell<bool>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let floor = budget.storage();
    let result = source_scalar_normalization_scratch_v18(
        original.source.cleanup,
        budget,
        optimized_write_headers_v87()?,
        |budget| {
            original.source.retain_construction(|| {
            let facts = optimized_source_writes_v87(original, optimized, 0, budget)?;
            assert_eq!(facts.len(), expected);
            assert!(facts.iter().all(Option::is_some));
            let first = facts[0].unwrap();
            let inventory = optimized.output_inventory(budget)?;
            for fact in facts.iter().flatten() {
                assert_eq!(fact.source.root_parameter, first.source.root_parameter);
                assert_eq!(fact.source.element, ScalarType::U32);
                assert_eq!(fact.tail.predicate, fact.tail.extent);
                assert!(matches!(source_operation_row_v18(inventory, fact.output[6], budget)?.operation.kind,
                    OperationKind::GuardedStore { pointer, predicate, value, .. }
                    if pointer == fact.tail.pointer && predicate == fact.tail.predicate && value == fact.value));
            }
            if expected > 1 {
                let last = facts[expected - 1].unwrap();
                assert_ne!(first.input[6], last.input[6]);
                assert_ne!(first.output[6], last.output[6]);
                // Address formation is outside the total-expression CSE whitelist.
                assert_eq!(first.output[..5], last.output[..5], "total metadata producers are commoned");
                assert_ne!(first.output[5], last.output[5]);
                assert!(matches!(optimized.operation(last.input[5], budget).unwrap(),
                    ProductionOptimizedSourceOperationV18::Retained { output, .. }
                    if output == last.output[5]));
                assert!((0..5).all(|i| matches!(optimized.operation(last.input[i], budget).unwrap(),
                    ProductionOptimizedSourceOperationV18::Rewritten { .. })));
            }
            let Some(fault) = fault else { reached.set(true); return Ok(()); };
            let fact = facts[expected - 1].unwrap();
            let result = match fault {
                0 => optimized_write_producer_v87(original, optimized, fact.input[0], fact.output[0].block.function,
                    fact.tail.data, budget).map(|_| ()),
                1 => optimized_write_producer_v87(original, optimized, fact.input[5], fact.output[0].block.function,
                    fact.tail.offset, budget).map(|_| ()),
                2 => optimized_write_value_descendant_v87(original, optimized,
                    fact.input[0].block.function, fact.output[0].block.function,
                    fact.source.index, fact.tail.zero, budget),
                3 => {
                    let function = optimized_source_root_function_v18(original, optimized, 0, budget)?;
                    let other = function.function.body.as_ref().unwrap().parameters[1 - fact.source.root_parameter];
                    optimized_write_value_descendant_v87(original, optimized,
                        fact.input[0].block.function, fact.output[0].block.function,
                        fact.source.root_input, other, budget)
                }
                _ => panic!("unknown optimized write mutation"),
            };
            let error = original.retain_query(result).unwrap_err();
            assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Binding(_)));
            let before = budget.work();
            assert_eq!(original.query(budget).unwrap_err().to_string(), error.to_string());
            assert_eq!(budget.work(), before);
            reached.set(true);
            Err(error)
        })
        },
    );
    assert_eq!(budget.storage(), floor);
    result
}

#[test]
fn optimized_write_headers_precede_scratch_and_restore_the_parent_floor() {
    let headers = optimized_write_headers_v87().unwrap();
    assert!(headers > issued_output_headers_v18().unwrap());
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(10);
        let mut budget = ArgumentBudgetV1::new(&mut work, headers + 17 - usize::from(short));
        budget.reserve_storage(17).unwrap();
        let result = budget.reserve_storage(headers);
        if short {
            assert!(matches!(result, Err(ArgumentResourceV1::Storage(error))
                if error.actual() == headers + 17 && error.limit() == headers + 16));
            assert_eq!(budget.failed_storage(), Some(headers + 17));
        } else {
            result.unwrap();
            budget.release_storage(headers).unwrap();
        }
        assert_eq!(budget.work(), 0);
        assert_eq!(budget.storage(), 17);
    }
}
