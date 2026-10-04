use super::super::tests::with_slots;
use super::*;

const LIMIT: usize = 256 * 1024 * 1024;

fn run(work: usize, storage: usize) -> (Result<()>, usize, usize, usize) {
    super::super::super::super::invocations::tests::run_variant(work, storage, true, |plan, out| {
        with_slots(plan, out, |slots, out| {
            let program = SourceByteProgram::derive(plan, slots, out)?;
            for root in 0..program.roots.len() {
                let hints = program.step_hints(root, out)?.unwrap();
                let range = &program.roots[root].0;
                assert_eq!(hints.entries.len(), range.len());
                for (instance, function) in program.functions[range.clone()].iter().enumerate() {
                    if let Some(function) = function {
                        for (block, row) in function.control.iter().enumerate() {
                            let hint = hints
                                .cuts
                                .iter()
                                .find(|hint| {
                                    hint.instance == instance
                                        && hint.pc == function.blocks.start + block
                                })
                                .unwrap();
                            assert_eq!(hint.statements, row.statements);
                            if let End::Call { child, arguments } = &row.end {
                                let call = hint.call.as_ref().unwrap();
                                assert_eq!(call.child, *child);
                                assert_eq!(call.arguments.len(), arguments.len());
                                for (actual, original) in call.arguments.iter().zip(arguments) {
                                    assert_eq!(Some(*actual), original.scalar_local_coordinates());
                                }
                            }
                        }
                    } else {
                        assert!(hints.entries[instance].is_none());
                    }
                }
                assert!(hints.fuels.capacity() >= hints.fuels.len());
                assert!(hints.entries.capacity() >= hints.entries.len());
                assert!(hints.cuts.capacity() >= hints.cuts.len());
            }
            assert!(out.text.is_empty());
            Ok(())
        })
    })
}

#[test]
fn source_step_hints_copy_authentic_coordinates_with_exact_and_one_short_accounts() {
    use super::super::super::super::invocations::tests::FLOOR;

    let measured = run(LIMIT, LIMIT);
    measured.0.unwrap();
    assert_eq!(measured.2, FLOOR);
    let exact = run(measured.1, measured.3);
    exact.0.unwrap();
    assert_eq!(exact.2, FLOOR);
    let short_work = run(measured.1 - 1, measured.3);
    assert!(short_work.0.is_err());
    assert_eq!(short_work.2, FLOOR);
    let short_storage = run(measured.1, measured.3 - 1);
    assert!(short_storage.0.is_err());
    assert_eq!(short_storage.2, FLOOR);
}

#[test]
fn source_step_hints_reject_foreign_or_refunded_accounts_before_debit_and_keep_denial() {
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
                                program.step_hints(0, &mut writer),
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
                    let before = (
                        out.budget.work(),
                        out.budget.storage(),
                        out.budget.peak_storage(),
                        out.text.len(),
                    );
                    for _ in 0..2 {
                        assert!(matches!(
                            program.step_hints(0, out),
                            Err(Error::Source(SourceError::Resource(Resource::Accounting)))
                        ));
                        assert_eq!(
                            (
                                out.budget.work(),
                                out.budget.storage(),
                                out.budget.peak_storage(),
                                out.text.len()
                            ),
                            before
                        );
                    }
                    program.step_hints(0, out).map(|_| ())
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
fn source_step_hints_decline_unsupported_shape_without_removing_generic_obligation() {
    super::super::super::super::invocations::tests::run_variant(LIMIT, LIMIT, true, |plan, out| {
        with_slots(plan, out, |slots, out| {
            let mut program = SourceByteProgram::derive(plan, slots, out)?;
            let index = program.roots[0].0.start;
            let function = program.functions[index].as_mut().unwrap();
            let old = std::mem::replace(&mut function.control[0].end, End::Abort);
            assert!(program.step_hints(0, out)?.is_none());
            assert!(program.step_hints(1, out)?.is_some());
            program.functions[index].as_mut().unwrap().control[0].end = old;
            assert!(program.step_hints(0, out)?.is_some());
            Ok(())
        })
    })
    .0
    .unwrap();
}
