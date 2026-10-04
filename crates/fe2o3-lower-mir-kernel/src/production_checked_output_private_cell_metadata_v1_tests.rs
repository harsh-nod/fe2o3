use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;

#[derive(Clone, Copy)]
enum Mode {
    Existing,
    Observe,
    Refuse,
    Panic,
}

struct BorrowedOnDrop<'s, 'g, 'd> {
    sites: Option<promotion_sites::CheckedPromotedSites<'s, 'g>>,
    dropped: &'d Cell<bool>,
    inspected_on_drop: &'d Cell<bool>,
}
impl Drop for BorrowedOnDrop<'_, '_, '_> {
    fn drop(&mut self) {
        if let Some(sites) = &self.sites {
            assert_eq!(sites.statements().len(), sites.output().operations().len());
            assert_eq!(sites.traps().len(), sites.statements().len());
            assert!(!sites.source().semantic().roots().is_empty());
            self.inspected_on_drop.set(true);
        }
        self.dropped.set(true);
    }
}

fn exercise(
    prefix: Prefix8<'_>,
    tail: &PromotionTail,
    expected: &[FormalMemoryObligations],
    floor: usize,
    sibling: &[u8],
) {
    assert!(!sibling.is_empty());
    assert!(sibling.iter().all(|byte| *byte == 0xa5));
    let run = |mode: Mode| {
        let mut work = Work::new(500_000_000);
        let entered = Cell::new(0);
        let dropped = Cell::new(false);
        let inspected_on_drop = Cell::new(false);
        let (accepted, peak) = {
            let mut budget = AssertOriginBudgetV1::new(&mut work, 512 << 20);
            budget.reserve_storage(floor).unwrap();
            budget.charge_work(17).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let result = promotion_scoped(floor, &mut budget, |budget, binding| {
                if matches!(mode, Mode::Existing) {
                    return check_promoted_output(prefix, tail, budget, binding);
                }
                with_promoted_output_sites(prefix, tail, budget, binding, |sites, budget, inner| {
                    entered.set(entered.get() + 1);
                    assert!(std::ptr::eq(sites.output().owner(), tail.output()));
                    assert!(std::ptr::eq(
                        sites.source().semantic(),
                        prefix.historical().source().semantic()
                    ));
                    assert_eq!(sites.statements().len(), sites.output().operations().len());
                    assert_eq!(sites.traps().len(), sites.statements().len());
                    let semantic = sites.source().semantic();
                    let mut statements = 0;
                    for (function, block, statement) in sites.statements().iter().flatten() {
                        assert!(
                            (*statement as usize)
                                < semantic.functions()[function.index() as usize].blocks()
                                    [block.index() as usize]
                                    .statements()
                                    .len()
                        );
                        statements += 1;
                    }
                    assert!(
                        statements > 0,
                        "actual source statements, not detached numeric fixtures"
                    );
                    for (authorized, operation) in
                        sites.traps().iter().zip(sites.output().operations())
                    {
                        if *authorized {
                            assert!(matches!(&operation.operation.kind,
                                OperationKind::Call { callee, arguments } if matches!(
                                    fe2o3_kernel_ir::AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments),
                                    Some(fe2o3_kernel_ir::AmdGpuDiagnosticOperation::Trap))));
                        }
                    }
                    assert!(budget.work_ledger_identity_v1() == ledger);
                    assert!(
                        budget.storage() > floor,
                        "actual reconstructed metadata remains reserved"
                    );
                    inner.check(budget)?;
                    let mut held = BorrowedOnDrop {
                        sites: Some(sites),
                        dropped: &dropped,
                        inspected_on_drop: &inspected_on_drop,
                    };
                    match mode {
                        Mode::Observe => {
                            promotion_sites::check_sites(held.sites.take().unwrap(), budget, inner)
                        }
                        Mode::Refuse => {
                            Err(refused("promoted metadata callback", "test refusal").into())
                        }
                        Mode::Panic => std::panic::panic_any(716_u32),
                        Mode::Existing => unreachable!(),
                    }
                })
            });
            match mode {
                Mode::Existing | Mode::Observe => assert_eq!(result.unwrap().as_ref(), expected),
                Mode::Refuse => assert!(matches!(
                    result,
                    Err(PError::Admission(E::Unsupported {
                        phase: "promoted metadata callback",
                        detail: "test refusal",
                    }))
                )),
                Mode::Panic => assert!(matches!(result, Err(PError::Panicked))),
            }
            assert_eq!(entered.get(), usize::from(!matches!(mode, Mode::Existing)));
            assert_eq!(dropped.get(), !matches!(mode, Mode::Existing));
            assert_eq!(
                inspected_on_drop.get(),
                matches!(mode, Mode::Refuse | Mode::Panic)
            );
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(budget.failed_storage(), None);
            assert!(sibling.iter().all(|byte| *byte == 0xa5));
            (budget.work(), budget.peak_storage())
        };
        assert_eq!(work.work(), accepted);
        assert_eq!(work.failed_work(), None);
        (accepted, peak)
    };
    let existing = run(Mode::Existing);
    assert_eq!(
        run(Mode::Observe),
        existing,
        "borrowing actual metadata adds no work/storage debit"
    );
    let refused = run(Mode::Refuse);
    assert_eq!(
        run(Mode::Panic),
        refused,
        "both exits occur after the same genuine reconstruction"
    );
    assert!(refused.0 < existing.0);
}

impl ProductionOwnedPrivateCellPromotionContinuationV1 {
    pub(crate) fn exercise_checked_promoted_sites_scope_v1(&self, floor: usize, sibling: &[u8]) {
        assert!(floor >= self.retained_input_storage_floor_v1().unwrap() + sibling.len());
        exercise(
            Prefix8::Direct(&self.prefix),
            &self.data.tail,
            &self.data.kernels,
            floor,
            sibling,
        );
    }
}

impl ProductionOwnedUnitLocalPrivateCellPromotionContinuationV1 {
    pub(crate) fn exercise_checked_promoted_sites_scope_v1(&self, floor: usize, sibling: &[u8]) {
        assert!(floor >= self.retained_input_storage_floor_v1().unwrap() + sibling.len());
        exercise(
            Prefix8::Erased(&self.prefix),
            &self.data.tail,
            &self.data.kernels,
            floor,
            sibling,
        );
    }
}
