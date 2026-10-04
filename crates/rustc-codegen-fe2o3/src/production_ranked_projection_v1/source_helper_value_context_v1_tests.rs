use super::super::helper_source_fixture_v1 as fixture;
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_mir_model::semantic_mir_v1::*;
use std::cell::Cell;

#[path = "source_helper_value_ledger_v1_tests.rs"]
mod ledger_tests;

#[path = "source_helper_value_route_v1_tests.rs"]
mod route_tests;

#[path = "source_helper_constant_shift_v1_tests.rs"]
mod shift_tests;

#[path = "source_helper_masked_shift_v1_tests.rs"]
mod masked_shift_tests;

struct TestMeter<'a, 'w> {
    budget: Budget<'w>,
    foreign: Budget<'w>,
    switch: &'a Cell<bool>,
    failed: bool,
}
impl Meter for TestMeter<'_, '_> {
    fn work(&mut self, n: usize) -> Result<(), Error> {
        self.current().charge_work(n).map_err(|_| {
            self.failed = true;
            "work"
        })
    }
    fn reserve(&mut self, n: usize) -> Result<(), Error> {
        self.current().reserve_storage(n).map_err(|_| {
            self.failed = true;
            "storage"
        })
    }
    fn release(&mut self, n: usize) -> Result<(), Error> {
        self.current().release_storage(n).map_err(|_| "accounting")
    }
    fn exhausted(&self) -> bool {
        self.failed
    }
    fn storage(&self) -> Result<usize, Error> {
        Ok(if self.switch.get() {
            self.foreign.storage()
        } else {
            self.budget.storage()
        })
    }
    fn identity(&mut self) -> Result<Ledger, Error> {
        let slot = self as *const Self as usize;
        Ok(Ledger {
            slot,
            work: self.current().work_ledger_identity_v1(),
        })
    }
}
impl<'w> TestMeter<'_, 'w> {
    fn current(&mut self) -> &mut Budget<'w> {
        if self.switch.get() {
            &mut self.foreign
        } else {
            &mut self.budget
        }
    }
}

fn run<T>(
    work: usize,
    storage: usize,
    action: impl FnOnce(&mut TestMeter<'_, '_>, &Cell<bool>) -> T,
) -> (T, usize, usize, usize, usize) {
    let mut used = Work::new(work);
    let mut foreign = Work::new(1_000_000);
    let switch = Cell::new(false);
    let mut meter = TestMeter {
        budget: Budget::new(&mut used, storage),
        foreign: Budget::new(&mut foreign, storage),
        switch: &switch,
        failed: false,
    };
    meter.budget.reserve_storage(4096).unwrap();
    meter.foreign.reserve_storage(31).unwrap();
    let result = action(&mut meter, &switch);
    (
        result,
        meter.budget.storage(),
        meter.budget.work(),
        meter.budget.peak_storage(),
        meter.foreign.storage(),
    )
}
fn call(source: &AdmittedInertSemanticMirV1) -> &SemanticDirectCallV1 {
    let SemanticTerminatorKindV1::Call(call) =
        source.functions()[0].blocks()[0].terminator().kind()
    else {
        panic!("fixture call")
    };
    call
}
fn constant(value: u64) -> ProductionSemanticExpressionV2 {
    ProductionSemanticExpressionV2::Constant {
        scalar: ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        },
        bits: value,
    }
}

#[test]
fn genuine_source_noncommutative_and_nested_arguments_are_exact() {
    let nested = fixture::function(
        50,
        false,
        0,
        vec![
            fixture::block(
                91,
                vec![],
                fixture::call(2, vec![fixture::copy(2), fixture::copy(1)], 0, 1),
            ),
            fixture::block(92, vec![], SemanticTerminatorKindV1::Return),
        ],
    );
    for (helpers, expected) in [
        (vec![fixture::subtract()], (7, 11)),
        (vec![nested, fixture::subtract()], (11, 7)),
    ] {
        let source = fixture::source(helpers);
        let (result, storage, _, _, _) = run(1_000_000, 16 * 1024 * 1024, |meter, _| {
            with_source_helper_values(&source, 0, meter, |context, meter| {
                let template =
                    context.call(&source, &source.functions()[0], 0, call(&source), meter)?;
                let (expression, bytes) =
                    template.instantiate(&[constant(7), constant(11)], meter)?;
                let expected = ProductionSemanticExpressionV2::Binary {
                    operation: ProductionSemanticBinaryOpV2::Subtract,
                    scalar: ProductionSemanticScalarTypeV2::Integer {
                        signed: false,
                        bits: 32,
                    },
                    overflow: ProductionOverflowContractV2::Wrapping,
                    lhs: Box::new(constant(expected.0)),
                    rhs: Box::new(constant(expected.1)),
                };
                assert_eq!(expression, expected);
                drop(expression);
                meter.release(bytes)
            })
        });
        result.unwrap();
        assert_eq!(storage, 4096);
    }
}

#[test]
fn foreign_owner_root_and_copied_call_occurrences_are_rejected() {
    let source = fixture::source(vec![fixture::subtract()]);
    let other = fixture::source(vec![fixture::subtract()]);
    let (result, storage, _, _, _) = run(1_000_000, 16 * 1024 * 1024, |meter, _| {
        with_source_helper_values(&source, 0, meter, |context, meter| {
            assert!(
                context
                    .call(&other, &source.functions()[0], 0, call(&source), meter)
                    .is_err()
            );
            assert!(
                context
                    .call(&source, &other.functions()[0], 0, call(&source), meter)
                    .is_err()
            );
            assert!(
                context
                    .call(&source, &source.functions()[0], 1, call(&source), meter)
                    .is_err()
            );
            assert!(
                context
                    .call(
                        &source,
                        &source.functions()[0],
                        0,
                        &call(&source).clone(),
                        meter
                    )
                    .is_err()
            );
            context
                .call(&source, &source.functions()[0], 0, call(&source), meter)
                .map(|_| ())
        })
    });
    result.unwrap();
    assert_eq!(storage, 4096);
}

#[test]
fn actual_root_resolver_substitutes_live_defined_call_at_exact_use_site() {
    let source = fixture::source(vec![fixture::subtract()]);
    let function = &source.functions()[0];
    let (result, storage, _, _, _) = run(1_000_000, 16 * 1024 * 1024, |meter, _| {
        with_source_helper_values(&source, 0, meter, |context, meter| {
            let mut resolver =
                GpuSemanticExpressionResolverV2::new(source.types(), function).unwrap();
            resolver.helper_semantic = Some(&source);
            resolver.helper_values = Some(context);
            resolver.helper_meter = Some(&mut *meter);
            let mut resolver = resolver
                .with_scalar_callables_v1(source.callables())
                .unwrap();
            let expression = resolver.resolve_store_v2(
                function.blocks()[1].statements()[0].kind(),
                ScalarAssignmentSiteV1 {
                    block: 1,
                    statement: 0,
                },
            )?;
            let scalar = ProductionSemanticScalarTypeV2::Integer {
                signed: false,
                bits: 32,
            };
            assert_eq!(
                expression,
                ProductionSemanticExpressionV2::Binary {
                    operation: ProductionSemanticBinaryOpV2::Subtract,
                    scalar,
                    overflow: ProductionOverflowContractV2::Wrapping,
                    lhs: Box::new(ProductionSemanticExpressionV2::Symbol {
                        symbol: fe2o3_pliron::PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2,
                        scalar
                    }),
                    rhs: Box::new(ProductionSemanticExpressionV2::Symbol {
                        symbol: fe2o3_pliron::PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2 + 1,
                        scalar
                    })
                }
            );
            assert!(resolver.use_site.is_none());
            assert!(resolver.visiting.is_empty());
            let bytes = resolver.helper_reserved;
            drop(expression);
            drop(resolver);
            meter.release(bytes)
        })
    });
    result.unwrap();
    assert_eq!(storage, 4096);
}

#[test]
fn source_lifetime_move_partial_ops_and_recursive_rosters_stay_unresolved() {
    let source = SemanticSourceProvenanceV1::unavailable();
    let moved = SemanticOperandV1::Move(fixture::place(1));
    let helpers = vec![
        fixture::function(
            70,
            false,
            1,
            vec![fixture::block(
                71,
                vec![
                    fixture::assign(3, SemanticRvalueKindV1::Use(moved)),
                    fixture::assign(0, SemanticRvalueKindV1::Use(fixture::copy(1))),
                ],
                SemanticTerminatorKindV1::Return,
            )],
        ),
        fixture::function(
            70,
            false,
            1,
            vec![fixture::block(
                71,
                vec![
                    SemanticStatementV1::new(
                        source,
                        SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(3)),
                    ),
                    fixture::assign(3, SemanticRvalueKindV1::Use(fixture::copy(1))),
                    SemanticStatementV1::new(
                        source,
                        SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(3)),
                    ),
                    fixture::assign(0, SemanticRvalueKindV1::Use(fixture::copy(3))),
                ],
                SemanticTerminatorKindV1::Return,
            )],
        ),
        fixture::function(
            70,
            false,
            1,
            vec![fixture::block(
                71,
                vec![fixture::assign(
                    0,
                    SemanticRvalueKindV1::Use(fixture::copy(3)),
                )],
                SemanticTerminatorKindV1::Return,
            )],
        ),
        fixture::function(
            70,
            false,
            0,
            vec![fixture::block(
                71,
                vec![fixture::assign(
                    0,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::Divide,
                        left: fixture::copy(1),
                        right: fixture::copy(2),
                    },
                )],
                SemanticTerminatorKindV1::Return,
            )],
        ),
        fixture::function(
            70,
            false,
            0,
            vec![
                fixture::block(
                    71,
                    vec![],
                    fixture::call(1, vec![fixture::copy(1), fixture::copy(2)], 0, 1),
                ),
                fixture::block(72, vec![], SemanticTerminatorKindV1::Return),
            ],
        ),
    ];
    for helper in helpers {
        let source = fixture::source(vec![helper]);
        let (result, storage, _, _, _) = run(1_000_000, 16 * 1024 * 1024, |meter, _| {
            with_source_helper_values(&source, 0, meter, |context, meter| {
                assert!(
                    context
                        .call(&source, &source.functions()[0], 0, call(&source), meter)
                        .is_err()
                );
                Ok(())
            })
        });
        result.unwrap();
        assert_eq!(storage, 4096);
    }
}

#[test]
fn exact_and_one_short_shared_limits_restore_the_caller_floor() {
    let source = fixture::source(vec![fixture::subtract()]);
    let probe = |meter: &mut TestMeter<'_, '_>, _: &Cell<bool>| {
        with_source_helper_values(&source, 0, meter, |context, meter| {
            context
                .call(&source, &source.functions()[0], 0, call(&source), meter)
                .map(|_| ())
        })
    };
    let (result, storage, work, peak, _) = run(1_000_000, 16 * 1024 * 1024, probe);
    result.unwrap();
    assert_eq!(storage, 4096);
    assert!(run(work, peak, probe).0.is_ok());
    let (result, storage, _, _, _) = run(work - 1, peak, probe);
    assert!(result.is_err());
    assert_eq!(storage, 4096);
    let (result, storage, _, _, _) = run(work, peak - 1, probe);
    assert!(result.is_err());
    assert_eq!(storage, 4096);
}

#[test]
fn same_adapter_replaced_work_ledger_is_not_used_for_cleanup() {
    let source = fixture::source(vec![fixture::subtract()]);
    let (result, _, _, _, foreign) = run(1_000_000, 16 * 1024 * 1024, |meter, switch| {
        with_source_helper_values(&source, 0, meter, |_, _| {
            switch.set(true);
            Ok(())
        })
    });
    assert!(result.is_err());
    assert_eq!(foreign, 31);
}

#[test]
fn success_error_and_panic_detect_lost_inherited_floor() {
    let source = fixture::source(vec![fixture::subtract()]);
    for failed in [false, true] {
        let (result, storage, _, _, _) = run(1_000_000, 16 * 1024 * 1024, |meter, _| {
            with_source_helper_values(&source, 0, meter, |_, meter| {
                meter.release(7)?;
                if failed {
                    Err("callback error")
                } else {
                    Ok(())
                }
            })
        });
        assert!(result.is_err());
        // The invalid callback is rejected before cache release can consume
        // the inherited floor. Dropped payload permits rollback to that floor.
        assert_eq!(storage, 4096);
    }
    let (result, storage, _, _, _) = run(1_000_000, 16 * 1024 * 1024, |meter, _| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = with_source_helper_values(&source, 0, meter, |_, meter| -> Result<(), Error> {
                meter.release(7)?;
                std::panic::panic_any(173u32)
            });
        }))
    });
    assert_eq!(*result.unwrap_err().downcast::<u32>().unwrap(), 173);
    assert_eq!(storage, 4096); // Unwinding drops cache payload before scope rollback.
}

#[path = "source_helper_inline_singleton_u32_v1_tests.rs"]
mod inline_singleton_tests;
