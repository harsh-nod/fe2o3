use super::super::tests::with_slots;
use super::*;

const LIMIT: usize = 256 * 1024 * 1024;

#[test]
fn source_conservation_rejects_foreign_ledger_and_refunded_owner_before_debit() {
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
    for foreign in [false, true] {
        let result = super::super::super::super::invocations::tests::run_variant(
            LIMIT,
            LIMIT,
            true,
            |plan, out| {
                with_slots(plan, out, |slots, out| {
                    let program = SourceByteProgram::derive(plan, slots, out)?;
                    if foreign {
                        let mut work = Work::new(LIMIT);
                        let mut budget = Budget::new(&mut work, LIMIT);
                        budget.reserve_storage(out.budget.storage())?;
                        let before = (budget.work(), budget.storage(), budget.peak_storage());
                        {
                            let mut writer = Writer::new(&mut budget)?;
                            assert!(matches!(
                                program.conservation_fuels(0, &mut writer),
                                Err(Error::Source(SourceError::Resource(Resource::Accounting)))
                            ));
                            assert!(writer.text.is_empty());
                        }
                        assert_eq!(
                            (budget.work(), budget.storage(), budget.peak_storage()),
                            before
                        );
                    } else {
                        assert_eq!(out.budget.storage(), program.required);
                        out.budget.release_storage(1)?;
                    }
                    let before = (out.budget.work(), out.budget.storage(), out.text.len());
                    let rejected = program.conservation_fuels(0, out);
                    assert!(matches!(
                        rejected,
                        Err(Error::Source(SourceError::Resource(Resource::Accounting)))
                    ));
                    assert_eq!(
                        (out.budget.work(), out.budget.storage(), out.text.len()),
                        before
                    );
                    let repeated = program.conservation_fuels(0, out);
                    assert!(matches!(
                        repeated,
                        Err(Error::Source(SourceError::Resource(Resource::Accounting)))
                    ));
                    assert_eq!(
                        (out.budget.work(), out.budget.storage(), out.text.len()),
                        before
                    );
                    rejected.map(|_| ())
                })
            },
        );
        assert!(matches!(
            result.0,
            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
        ));
    }
}

#[test]
fn source_conservation_uses_real_statement_counts_for_every_active_instance() {
    for roots in [2, 3] {
        super::super::super::super::invocations::tests::run_root_variant(
            LIMIT,
            LIMIT,
            true,
            false,
            roots,
            |plan, out| {
                with_slots(plan, out, |slots, out| {
                    let program = SourceByteProgram::derive(plan, slots, out)?;
                    for root in 0..usize::from(roots) {
                        let fuels = program.conservation_fuels(root, out)?.unwrap();
                        let range = &program.roots[root].0;
                        assert_eq!(fuels.len(), range.len());
                        for (instance, function) in
                            program.functions[range.clone()].iter().enumerate()
                        {
                            let expected = function
                                .as_ref()
                                .map(|function| {
                                    function
                                        .control
                                        .iter()
                                        .map(|row| row.statements + 1)
                                        .max()
                                        .unwrap_or(0)
                                })
                                .unwrap_or(0);
                            assert_eq!(fuels[instance], expected);
                        }
                        assert!(fuels.contains(&1));
                        assert!(fuels.contains(&2));
                    }
                    Ok(())
                })
            },
        )
        .0
        .unwrap();
    }
}

#[test]
fn source_conservation_declines_unmodeled_control_without_changing_source_execution() {
    super::super::super::super::invocations::tests::run_variant(LIMIT, LIMIT, true, |plan, out| {
        with_slots(plan, out, |slots, out| {
            let mut program = SourceByteProgram::derive(plan, slots, out)?;
            assert!(program.conservation_fuels(0, out)?.is_some());
            let index = program.roots[0].0.start;
            let function = program.functions[index].as_mut().unwrap();
            let original = std::mem::replace(&mut function.control[0].end, End::Abort);
            assert!(program.conservation_fuels(0, out)?.is_none());
            assert!(program.conservation_fuels(1, out)?.is_some());
            program.functions[index].as_mut().unwrap().control[0].end = original;
            assert!(program.conservation_fuels(0, out)?.is_some());
            let child = match &program.functions[index].as_ref().unwrap().control[0].end {
                End::Call { child, .. } => *child,
                _ => panic!("fixture must retain its real source call"),
            };
            let child_index = index + child;
            let removed = program.functions[child_index].take();
            assert!(removed.is_some());
            assert!(program.conservation_fuels(0, out)?.is_none());
            program.functions[child_index] = removed;
            assert!(program.conservation_fuels(0, out)?.is_some());
            assert!(!event_supported(Event::Scalar));
            assert!(!event_supported(Event::AggregateReset { local: 0 }));
            assert!(event_supported(Event::Transfer {
                destination: Destination::Local(17),
                value: Value::Constant(53),
                scalar: ScalarV30::Unit,
            }));
            assert!(!event_supported(Event::Transfer {
                destination: Destination::Local(17),
                value: Value::Local {
                    local: 19,
                    moved: false
                },
                scalar: ScalarV30::Unit,
            }));
            Ok(())
        })
    })
    .0
    .unwrap();
}

#[test]
fn source_conservation_header_allowance_and_shared_law_domains_are_explicit() {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    assert_eq!(
        headers(),
        h::<Option<Vec<usize>>>()
            + h::<Vec<usize>>()
            + h::<bool>()
            + size_of::<std::slice::Iter<'_, Option<SourceByteFunction<'_, '_, '_>>>>()
            + size_of::<
                std::iter::Enumerate<std::slice::Iter<'_, Option<SourceByteFunction<'_, '_, '_>>>>,
            >()
            + size_of::<std::iter::Enumerate<std::slice::Iter<'_, BodyBlock>>>()
            + size_of::<std::slice::Iter<'_, TypedOperand>>()
            + size_of::<Range<usize>>()
            + 12 * size_of::<usize>()
            + 8 * size_of::<&()>()
    );
    let laws = include_str!("original_semantic_mir_source_constructor_laws_v81.vrs");
    assert_eq!(laws.matches("proof fn ").count(), 12);
    assert_eq!(laws.matches("#[verifier::spinoff_prover]").count(), 11);
    assert!(laws.contains("requires invocation_byte_heaps_related_v36(source, target, map),\n        map.private == Map::<MemoryAllocationV30, InvocationByteBindingV36>::empty(),"));
    assert!(laws.contains("requires\n        forall|allocation: MemoryAllocationV30| #[trigger] source.machine.memory.live.contains_key(allocation)\n            ==> !invocation_private_allocation_v36(allocation),"));
    assert!(laws.contains("destination.component.is_none()\n                && destination.memory.is_none() && destination.descriptor.is_none()"));
    for forbidden in [
        "assume(",
        "admit(",
        "external_body",
        "proof fn invocation_paired_source_preserved",
    ] {
        assert!(!laws.contains(forbidden));
    }
}
