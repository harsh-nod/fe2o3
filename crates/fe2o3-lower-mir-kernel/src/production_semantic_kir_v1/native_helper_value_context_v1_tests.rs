use super::super::helper_source_fixture_v1 as fixture;
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
};
use fe2o3_pliron::ProductionSemanticMirLimitsV1;
use std::cell::Cell;

#[path = "native_helper_value_ledger_v1_tests.rs"]
mod ledger_tests;

struct TestMeter<'a, 'w> {
    first: Budget<'w>,
    second: Budget<'w>,
    replaced: &'a Cell<bool>,
    failed: bool,
}
impl<'w> TestMeter<'_, 'w> {
    fn current(&mut self) -> &mut Budget<'w> {
        if self.replaced.get() {
            &mut self.second
        } else {
            &mut self.first
        }
    }
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
        Ok(if self.replaced.get() {
            self.second.storage()
        } else {
            self.first.storage()
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
fn run<T>(
    work: usize,
    storage: usize,
    action: impl FnOnce(&mut TestMeter<'_, '_>, &Cell<bool>) -> T,
) -> (T, usize, usize, usize, usize) {
    let mut first = Work::new(work);
    let mut second = Work::new(1_000_000);
    let replaced = Cell::new(false);
    let mut meter = TestMeter {
        first: Budget::new(&mut first, storage),
        second: Budget::new(&mut second, storage),
        replaced: &replaced,
        failed: false,
    };
    meter.first.reserve_storage(4096).unwrap();
    meter.second.reserve_storage(31).unwrap();
    let result = action(&mut meter, &replaced);
    (
        result,
        meter.first.storage(),
        meter.first.work(),
        meter.first.peak_storage(),
        meter.second.storage(),
    )
}

fn owner() -> ProductionPreRankedKirOwnerV1 {
    let semantic = fixture::source(vec![fixture::subtract()]);
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        &semantic,
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "helper_value_source",
            [31; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([1, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(semantic, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        semantic,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut work = Work::new(100_000_000);
    let mut budget = Budget::new(&mut work, 512 * 1024 * 1024);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
}
fn entry(module: &Module) -> &Function {
    module
        .functions
        .iter()
        .find(|function| function.id == module.kernels[0].entry)
        .unwrap()
}
fn call(function: &Function) -> (FunctionOperationLocation, &Operation) {
    function
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .find_map(|block| {
            block
                .operations
                .iter()
                .enumerate()
                .find_map(|(ordinal, operation)| {
                    matches!(operation.kind, OperationKind::Call { .. })
                        .then_some((FunctionOperationLocation::new(block.id, ordinal), operation))
                })
        })
        .unwrap()
}
fn constant(bits: u64) -> NormalizedScalarExpressionV1 {
    NormalizedScalarExpressionV1::Constant {
        scalar: ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        },
        bits,
    }
}

#[test]
fn actual_verified_native_helper_is_derived_and_bound_to_exact_call() {
    let owner = owner();
    let module = owner.executable.module();
    let entry = entry(module);
    let (location, operation) = call(entry);
    let (result, storage, _, _, _) = run(1_000_000, 16 * 1024 * 1024, |meter, _| {
        with_native_helper_values(
            owner.semantic_ssa.source_semantic(),
            module,
            &owner.correspondence,
            SemanticFunctionIdV1::from_index(0),
            entry,
            meter,
            |context, meter| {
                let template = context.root_call(entry, location, operation, meter)?;
                let (expression, bytes) =
                    template.instantiate(&[constant(7), constant(11)], meter)?;
                assert_eq!(
                    expression,
                    NormalizedScalarExpressionV1::Binary {
                        operation: ProductionSemanticBinaryOpV2::Subtract,
                        scalar: ProductionSemanticScalarTypeV2::Integer {
                            signed: false,
                            bits: 32
                        },
                        overflow: ProductionOverflowContractV2::Checked,
                        lhs: Box::new(constant(7)),
                        rhs: Box::new(constant(11))
                    }
                );
                drop(expression);
                meter.release(bytes)?;
                assert!(
                    context
                        .root_call(&entry.clone(), location, operation, meter)
                        .is_err()
                );
                assert!(
                    context
                        .root_call(entry, location, &operation.clone(), meter)
                        .is_err()
                );
                assert!(
                    context
                        .call(
                            &module.clone(),
                            owner.semantic_ssa.source_semantic(),
                            &owner.correspondence,
                            SemanticFunctionIdV1::from_index(0),
                            entry,
                            location,
                            operation,
                            meter
                        )
                        .is_err()
                );
                assert!(
                    context
                        .call(
                            module,
                            owner.semantic_ssa.source_semantic(),
                            &owner.correspondence,
                            SemanticFunctionIdV1::from_index(1),
                            entry,
                            location,
                            operation,
                            meter
                        )
                        .is_err()
                );
                Ok(())
            },
        )
    });
    result.unwrap();
    assert_eq!(storage, 4096);
}

#[test]
fn native_exact_and_one_short_cache_limits_are_shared_and_restore_floor() {
    let owner = owner();
    let module = owner.executable.module();
    let entry = entry(module);
    let (location, operation) = call(entry);
    let probe = |meter: &mut TestMeter<'_, '_>, _: &Cell<bool>| {
        with_native_helper_values(
            owner.semantic_ssa.source_semantic(),
            module,
            &owner.correspondence,
            SemanticFunctionIdV1::from_index(0),
            entry,
            meter,
            |context, meter| {
                context
                    .root_call(entry, location, operation, meter)
                    .map(|_| ())
            },
        )
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
fn native_scope_rejects_replaced_work_and_success_floor_loss() {
    let owner = owner();
    let module = owner.executable.module();
    let entry = entry(module);
    let (result, _, _, _, foreign) = run(1_000_000, 16 * 1024 * 1024, |meter, replaced| {
        with_native_helper_values(
            owner.semantic_ssa.source_semantic(),
            module,
            &owner.correspondence,
            SemanticFunctionIdV1::from_index(0),
            entry,
            meter,
            |_, _| {
                replaced.set(true);
                Ok(())
            },
        )
    });
    assert!(result.is_err());
    assert_eq!(foreign, 31);
    let (result, storage, _, _, _) = run(1_000_000, 16 * 1024 * 1024, |meter, _| {
        with_native_helper_values(
            owner.semantic_ssa.source_semantic(),
            module,
            &owner.correspondence,
            SemanticFunctionIdV1::from_index(0),
            entry,
            meter,
            |_, meter| {
                meter.release(7)?;
                Ok(())
            },
        )
    });
    assert!(result.is_err());
    // Invalid cache custody is detected before releasing the inherited floor.
    assert_eq!(storage, 4096);
}
