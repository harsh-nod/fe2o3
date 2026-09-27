//! Genuine F1 observation only: earlier intrinsic effects/F2/normal routing remain absent.
use super::*;
use crate::production_ranked_projection_v1::{
    bf16_nominal_source_preparation_v1::with_nominal_rich_source_preparation_v1,
    canonical_assertion_facts_v1::with_nominal_canonical_facts_observation_v1,
};
use crate::reference_effect_v1::{
    ReferenceEffectExpressionV1, ReferenceOutputCoordinateV1, ReferenceOutputWriteV1,
};
use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
use fe2o3_lower_mir_kernel::{
    Bf16NominalCallQueryErrorV1 as QueryError, CheckedBf16CallInstanceV1,
};
use std::panic::{AssertUnwindSafe, catch_unwind};
type Q<T> = std::result::Result<T, QueryError>;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Observe,
    Error,
    Panic,
    Reentry,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Observation {
    locals: usize,
    operations: usize,
    next_value: u32,
    next_argument: usize,
    index_address: usize,
    slice_address: usize,
    operations_address: usize,
}
fn projection(error: Error) -> QueryError {
    match error {
        Error::CanonicalAssertions(crate::production_ranked_projection_v1::canonical_assertion_facts_v1::CanonicalAssertionErrorV1::Resource(error)) => QueryError::Resource(error),
        _ => QueryError::Unavailable("actual argument initialization checkpoint refused"),
    }
}
fn observer_frame() -> Result<usize> {
    // New test owner/caller/header slots, no fixed residual allowance. Nested
    // checked/rich/facts/context constructors admit their concrete F/R separately.
    sum(&[
        call_frame::<Q<Observation>>(size_of::<(
            PendingActualRootPrefixIndicesV1,
            Observation,
            Mode,
            usize,
            usize,
            usize,
            usize,
            usize,
            usize,
            LedgerId,
            bool,
            Result<Observation>,
            Q<Observation>,
            std::thread::Result<Q<Observation>>,
            Box<dyn std::any::Any + Send>,
        )>())?,
        call_frame::<Q<Observation>>(size_of::<(
            &ProductionPreRankedKirOwnerV1,
            &CheckedBf16CallInstanceV1<'static>,
            &CanonicalKirInventoryV1<'static>,
            &crate::production_pipeline::ActualRetainedRankedInputsV1<'static>,
            &mut Budget<'static>,
            &mut usize,
            &mut PendingActualRootPrefixIndicesV1,
            Mode,
        )>())?,
        call_frame::<Observation>(size_of::<(
            ActualRootArgumentInitializationV1<'static>,
            &mut Context,
            usize,
            u32,
            usize,
            &AuthenticatedReferenceEffectBindingV1,
            ProductionRankedOperationV1,
            ProductionRankedValueIdV1,
            Option<&ProductionRankedOperationV1>,
        )>())?,
        call_frame::<()>(size_of::<(
            &[ProductionRankedOperationV1],
            &mut usize,
            ProductionRankedOperationV1,
            &mut Prep<'static, 'static>,
        )>())?,
        call_frame::<ProductionRankedValueIdV1>(size_of::<(&mut u32, u32, Option<u32>)>())?,
        call_frame::<QueryError>(size_of::<Error>())?,
        call_frame::<usize>(size_of::<[usize; 15]>())?,
        call_frame::<usize>(size_of::<(&[usize], usize, &usize, Option<usize>)>())?,
        // Independent observer callback frame, not covered by the driver closure.
        call_frame::<Observation>(size_of::<(
            &ActualRootArgumentInitializationV1<'static>,
            &mut Prep<'static, 'static>,
            usize,
            usize,
            u32,
            usize,
        )>())?,
        // Actual borrowed write roster and its source-level for-loop state.
        call_frame::<Option<&ReferenceOutputWriteV1>>(size_of::<(
            &Box<[ReferenceOutputWriteV1]>,
            &[ReferenceOutputWriteV1],
            std::slice::Iter<'static, ReferenceOutputWriteV1>,
            &ReferenceOutputWriteV1,
            &ReferenceOutputCoordinateV1,
        )>())?,
        // LogicalPoint binds a BORROW of Box<[Expression]>, not a copied tree.
        call_frame::<usize>(size_of::<(
            &Box<[ReferenceEffectExpressionV1]>,
            &[ReferenceEffectExpressionV1],
            usize,
        )>())?,
        // Distinct constant-prefix and axis loops, including yielded values.
        call_frame::<Option<i32>>(size_of::<(
            std::ops::Range<i32>,
            Option<i32>,
            ProductionRankedValueIdV1,
            &mut u32,
            &mut usize,
        )>())?,
        call_frame::<Option<usize>>(size_of::<(
            std::ops::Range<usize>,
            Option<usize>,
            usize,
            std::result::Result<u32, std::num::TryFromIntError>,
            ProductionRankedValueIdV1,
            &mut u32,
            &mut usize,
        )>())?,
        // Reserved-value table borrows and comparison operands remain live.
        call_frame::<bool>(size_of::<(
            Option<&Vec<ProductionRankedValueIdV1>>,
            &Vec<ProductionRankedValueIdV1>,
            Option<&ProductionRankedValueIdV1>,
            &ProductionRankedValueIdV1,
            &Option<&ProductionRankedValueIdV1>,
            &Option<&ProductionRankedValueIdV1>,
        )>())?,
        // expect_operation's two Option<&Operation> equality receivers/results.
        call_frame::<bool>(size_of::<(
            Option<&ProductionRankedOperationV1>,
            Option<&ProductionRankedOperationV1>,
            &Option<&ProductionRankedOperationV1>,
            &Option<&ProductionRankedOperationV1>,
            &ProductionRankedOperationV1,
            &ProductionRankedOperationV1,
        )>())?,
    ])
}
fn expect_operation(
    actual: &[ProductionRankedOperationV1],
    cursor: &mut usize,
    expected: ProductionRankedOperationV1,
    resources: &mut Prep<'_, '_>,
) -> Result<()> {
    resources.work(128)?;
    if actual.get(*cursor) != Some(&expected) {
        return Err(Error::Incomplete(
            "argument initializer changed the original entry prefix",
        ));
    }
    *cursor = cursor.checked_add(1).ok_or_else(arithmetic)?;
    Ok(())
}
fn next(next: &mut u32) -> Result<ProductionRankedValueIdV1> {
    let id = ProductionRankedValueIdV1::new(*next);
    *next = next.checked_add(1).ok_or_else(arithmetic)?;
    Ok(id)
}
fn inspect_payload_inner(
    view: ActualRootArgumentInitializationV1<'_>,
    context: &mut NominalRecipeResourcesV1<'_, '_, '_, '_, '_, '_>,
) -> Result<Observation> {
    context.with_resources(|resources| {
        let locals = view.function().locals().len();
        resources.work(locals.checked_mul(2).and_then(|n| n.checked_add(64)).ok_or_else(arithmetic)?)?;
        assert_eq!(view.index_arguments().len(), locals);
        assert_eq!(view.slice_arguments().len(), locals);
        assert!(view.index_arguments().iter().all(Option::is_none));
        assert!(view.slice_arguments().iter().all(Option::is_none));
        assert_eq!(view.next_argument(), 1);
        assert!(view.arguments.initialized && view.arguments.later.vacant());
        let mut cursor = 0usize;
        let mut next_value = 0u32;
        let mut reserved = 0usize;
        expect_operation(view.entry_operations(), &mut cursor,
            ranked_execution_layout_v1(view.source.source_root.layout()), resources)?;
        match view.source.references {
            [] => assert!(view.prefix.reserved_reference_values.is_none()),
            [binding] => {
                assert!(!binding.observable_output_writes.is_empty());
                for write in &binding.observable_output_writes {
                    resources.work(32)?;
                    let crate::reference_effect_v1::ReferenceOutputCoordinateV1::LogicalPoint(axes) = &write.coordinate
                        else { return Err(Error::Incomplete("initializer reference output is not a point")); };
                    for _ in 0..3 {
                        resources.work(64)?;
                        let id = next(&mut next_value)?;
                        assert_eq!(view.prefix.reserved_reference_values.as_ref().unwrap().get(reserved), Some(&id));
                        reserved += 1;
                        expect_operation(view.entry_operations(), &mut cursor,
                            ProductionRankedOperationV1::SemanticConstant { result: id, value: 0 }, resources)?;
                    }
                    for axis in 0..axes.len() {
                        resources.work(64)?;
                        let id = next(&mut next_value)?;
                        assert_eq!(view.prefix.reserved_reference_values.as_ref().unwrap().get(reserved), Some(&id));
                        reserved += 1;
                        expect_operation(view.entry_operations(), &mut cursor,
                            ProductionRankedOperationV1::SemanticSymbol { result: id,
                                symbol: u32::try_from(axis).map_err(|_| arithmetic())? }, resources)?;
                    }
                }
                assert_eq!(view.prefix.reserved_reference_values.as_ref().unwrap().len(), reserved);
            }
            _ => return Err(Error::Incomplete("initializer duplicate references")),
        }
        assert_eq!(cursor, view.entry_operations().len(), "no invocation/argument writer has executed");
        assert_eq!(next_value, view.next_value());
        Ok(Observation {
            locals, operations: cursor, next_value, next_argument: view.next_argument(),
            index_address: view.index_arguments().as_ptr() as usize,
            slice_address: view.slice_arguments().as_ptr() as usize,
            operations_address: view.entry_operations().as_ptr() as usize,
        })
    })
}
fn run(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    actual_inputs: &crate::production_pipeline::ActualRetainedRankedInputsV1<'_>,
    budget: &mut Budget<'_>,
    mode: Mode,
) -> Q<Observation> {
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, |budget| {
        let identity = budget.work_ledger_identity_v1();
        let slot = budget as *const Budget<'_> as usize;
        let floor = budget.storage();
        let before_work = budget.work();
        let before_peak = budget.peak_storage();
        let header = observer_frame().map_err(projection)?;
        budget.charge_work(header)?;
        budget.reserve_storage(header)?;
        let mut owned = header;
        let mut pending = PendingActualRootPrefixIndicesV1::new();
        let mut entered = false;
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            owner.with_checked_bf16_nominal_call_v1(
                inventory, source.root(), source.root(), source.call_block(), source.source_call(),
                budget, |checked, budget| {
                    with_nominal_rich_source_preparation_v1(
                        owner, inventory, source.root(), source.root(), source.call_block(), source.source_call(),
                        budget, |rich, budget| {
                            with_nominal_canonical_facts_observation_v1(
                                owner, inventory, source.root(), source.root(), source.call_block(), source.source_call(),
                                budget, |facts| {
                                    with_nominal_recipe_resources_v1(facts, rich, &mut owned, |context| {
                                        let result = context.with_actual_root_argument_initialization_v1(
                                            checked, rich, actual_inputs, &mut pending, |view, context| {
                                                entered = true;
                                                let observed = inspect_payload_inner(view, context)?;
                                                match mode {
                                                    Mode::Error => Err(Error::Incomplete("initializer observer refusal")),
                                                    Mode::Panic => panic!("initializer observer unwind"),
                                                    _ => Ok(observed),
                                                }
                                            },
                                        )?;
                                        if mode == Mode::Reentry {
                                            assert!(context.with_actual_root_argument_initialization_v1(
                                                checked, rich, actual_inputs, &mut pending,
                                                |_, _| -> Result<()> { panic!("initializer retry entered") },
                                            ).is_err());
                                            assert!(context.with_actual_root_prefix_indices_v1(
                                                checked, rich, actual_inputs, &mut pending,
                                                |_, _| -> Result<()> { panic!("old factory resumed initializer") },
                                            ).is_err());
                                        }
                                        Ok(result)
                                    }).map_err(projection)
                                },
                            )
                        },
                    )
                },
            )
        }));
        let result = match outcome {
            Ok(result) => result,
            Err(payload) => { drop(payload); Err(QueryError::CallbackPanicked) },
        };
        // Owners remain PHYSICALLY alive through all nested source/context
        // postflights and this outer custody check.
        assert!(entered && pending.started && !pending.completed);
        assert!(pending.indices.indices.is_empty() && pending.indices.index_fifo.is_empty());
        assert!(pending.arguments.initialized && pending.arguments.later.vacant());
        assert_eq!(pending.arguments.next_argument, 1);
        assert_eq!(pending.arguments.phase,
            if mode == Mode::Observe { Phase::InitializedBeforeArgumentWriters } else { Phase::Terminal });
        if let Ok(observed) = &result {
            assert_eq!(observed.index_address, pending.arguments.index_arguments.as_ptr() as usize);
            assert_eq!(observed.slice_address, pending.arguments.slice_arguments.as_ptr() as usize);
            assert_eq!(observed.operations_address, pending.prefix.entry_operations.as_ptr() as usize);
        }
        let protected = floor.checked_add(owned).ok_or(QueryError::Resource(Resource::Arithmetic))?;
        if slot != budget as *const Budget<'_> as usize || identity != budget.work_ledger_identity_v1()
            || budget.storage() < protected || budget.work() < before_work || budget.peak_storage() < before_peak
            || budget.failed_work().is_some() || budget.failed_storage().is_some()
        { drop(pending); return Err(QueryError::Resource(Resource::Accounting)); }
        drop(pending);
        budget.release_storage(owned)?;
        assert_eq!(budget.storage(), floor);
        result
    })
}
pub(crate) fn observe_actual_root_argument_initialization_for_test_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    actual_inputs: &crate::production_pipeline::ActualRetainedRankedInputsV1<'_>,
    budget: &mut Budget<'_>,
) -> Q<()> {
    let observed = run(
        owner,
        source,
        inventory,
        actual_inputs,
        budget,
        Mode::Observe,
    )?;
    let repeated = run(
        owner,
        source,
        inventory,
        actual_inputs,
        budget,
        Mode::Reentry,
    )?;
    assert_eq!(
        (
            observed.locals,
            observed.operations,
            observed.next_value,
            observed.next_argument
        ),
        (
            repeated.locals,
            repeated.operations,
            repeated.next_value,
            repeated.next_argument
        )
    );
    assert!(matches!(
        run(owner, source, inventory, actual_inputs, budget, Mode::Error),
        Err(QueryError::Unavailable(_))
    ));
    assert!(matches!(
        run(owner, source, inventory, actual_inputs, budget, Mode::Panic),
        Err(QueryError::CallbackPanicked)
    ));
    eprintln!(
        "fe2o3-root-argument-initialization-v1 locals={} operations={} next_value={} next_argument={} before_writers=true reentry=pass error=pass panic=pass",
        observed.locals, observed.operations, observed.next_value, observed.next_argument
    );
    Ok(())
}

#[test]
fn actual_argument_initialization_observer_header_has_no_residual_allowance() {
    fn expected<T>(locals: usize) -> usize {
        locals + 2 * size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    let rows = [
        expected::<Q<Observation>>(size_of::<(
            PendingActualRootPrefixIndicesV1,
            Observation,
            Mode,
            usize,
            usize,
            usize,
            usize,
            usize,
            usize,
            LedgerId,
            bool,
            Result<Observation>,
            Q<Observation>,
            std::thread::Result<Q<Observation>>,
            Box<dyn std::any::Any + Send>,
        )>()),
        expected::<Q<Observation>>(size_of::<(
            &ProductionPreRankedKirOwnerV1,
            &CheckedBf16CallInstanceV1<'static>,
            &CanonicalKirInventoryV1<'static>,
            &crate::production_pipeline::ActualRetainedRankedInputsV1<'static>,
            &mut Budget<'static>,
            &mut usize,
            &mut PendingActualRootPrefixIndicesV1,
            Mode,
        )>()),
        expected::<Observation>(size_of::<(
            ActualRootArgumentInitializationV1<'static>,
            &mut Context,
            usize,
            u32,
            usize,
            &AuthenticatedReferenceEffectBindingV1,
            ProductionRankedOperationV1,
            ProductionRankedValueIdV1,
            Option<&ProductionRankedOperationV1>,
        )>()),
        expected::<()>(size_of::<(
            &[ProductionRankedOperationV1],
            &mut usize,
            ProductionRankedOperationV1,
            &mut Prep<'static, 'static>,
        )>()),
        expected::<ProductionRankedValueIdV1>(size_of::<(&mut u32, u32, Option<u32>)>()),
        expected::<QueryError>(size_of::<Error>()),
        expected::<usize>(size_of::<[usize; 15]>()),
        expected::<usize>(size_of::<(&[usize], usize, &usize, Option<usize>)>()),
        expected::<Observation>(size_of::<(
            &ActualRootArgumentInitializationV1<'static>,
            &mut Prep<'static, 'static>,
            usize,
            usize,
            u32,
            usize,
        )>()),
        expected::<Option<&ReferenceOutputWriteV1>>(size_of::<(
            &Box<[ReferenceOutputWriteV1]>,
            &[ReferenceOutputWriteV1],
            std::slice::Iter<'static, ReferenceOutputWriteV1>,
            &ReferenceOutputWriteV1,
            &ReferenceOutputCoordinateV1,
        )>()),
        expected::<usize>(size_of::<(
            &Box<[ReferenceEffectExpressionV1]>,
            &[ReferenceEffectExpressionV1],
            usize,
        )>()),
        expected::<Option<i32>>(size_of::<(
            std::ops::Range<i32>,
            Option<i32>,
            ProductionRankedValueIdV1,
            &mut u32,
            &mut usize,
        )>()),
        expected::<Option<usize>>(size_of::<(
            std::ops::Range<usize>,
            Option<usize>,
            usize,
            std::result::Result<u32, std::num::TryFromIntError>,
            ProductionRankedValueIdV1,
            &mut u32,
            &mut usize,
        )>()),
        expected::<bool>(size_of::<(
            Option<&Vec<ProductionRankedValueIdV1>>,
            &Vec<ProductionRankedValueIdV1>,
            Option<&ProductionRankedValueIdV1>,
            &ProductionRankedValueIdV1,
            &Option<&ProductionRankedValueIdV1>,
            &Option<&ProductionRankedValueIdV1>,
        )>()),
        expected::<bool>(size_of::<(
            Option<&ProductionRankedOperationV1>,
            Option<&ProductionRankedOperationV1>,
            &Option<&ProductionRankedOperationV1>,
            &Option<&ProductionRankedOperationV1>,
            &ProductionRankedOperationV1,
            &ProductionRankedOperationV1,
        )>()),
    ];
    assert_eq!(observer_frame().unwrap(), rows.iter().sum::<usize>());
}
