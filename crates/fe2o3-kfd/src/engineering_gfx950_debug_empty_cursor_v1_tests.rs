//! Pure controls of the actual sticky cursor; no native owner is fabricated.
use super::*;
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

fn roster() -> Vec<Gfx950DebugLocalStepV1> {
    let mut c = Cursor::new(7);
    let mut v = Vec::new();
    while c.phase() != Gfx950DebugLocalPhaseV1::LocalBackingRetired {
        c.run(7, |step| {
            v.push(step);
            Ok(())
        })
        .unwrap();
        assert!(v.len() <= 61);
    }
    v
}
#[test]
fn exact_fixed_order_and_all_eight_resource_retirements() {
    use Gfx950DebugAllocationRetirementV1 as A;
    use Gfx950DebugLocalStepV1 as S;
    let v = roster();
    assert_eq!(v.len(), 61);
    assert_eq!(
        &v[..20],
        &[
            S::Revalidate,
            S::RegisterRuntime,
            S::CreateEvent,
            S::AllocateRing,
            S::AllocateControl,
            S::InitializeControl,
            S::AllocateSignal,
            S::InitializeSignal,
            S::AllocateKernarg,
            S::AllocateEop,
            S::AllocateCwsr,
            S::CreateQueue,
            S::MapDoorbell,
            S::InspectEmpty,
            S::DestroyQueue,
            S::DestroyEvent,
            S::WithdrawMetadata,
            S::DisableRuntime,
            S::ClearTrap,
            S::UnmapDoorbell,
        ]
    );
    for ordinal in 0..8 {
        assert_eq!(
            &v[20 + ordinal * 5..25 + ordinal * 5],
            &[
                S::RetireAllocation {
                    ordinal: ordinal as u8,
                    operation: A::UnmapGpu
                },
                S::RetireAllocation {
                    ordinal: ordinal as u8,
                    operation: A::UnmapCpu
                },
                S::RetireAllocation {
                    ordinal: ordinal as u8,
                    operation: A::FreeGpuHandle
                },
                S::RetireAllocation {
                    ordinal: ordinal as u8,
                    operation: A::ReleaseVa
                },
                S::RetireAllocation {
                    ordinal: ordinal as u8,
                    operation: A::ReconcileAccounting
                },
            ]
        );
    }
    assert_eq!(v[60], S::Reconcile);
}
#[test]
fn every_failed_native_boundary_is_terminal_without_another_call() {
    let expected = roster();
    for (fail_at, failed_step) in expected.iter().copied().enumerate() {
        let mut c = Cursor::new(7);
        let mut calls = Vec::new();
        for step in expected.iter().take(fail_at) {
            c.run(7, |s| {
                assert_eq!(s, *step);
                calls.push(s);
                Ok(())
            })
            .unwrap();
        }
        assert!(
            c.run(7, |s| {
                calls.push(s);
                Err(Gfx950DebugLocalErrorV1::Contract("injected"))
            })
            .is_err()
        );
        assert_eq!(c.phase(), Gfx950DebugLocalPhaseV1::Attempting(failed_step));
        assert!(matches!(
            c.run(7, |_| panic!("retry executed")),
            Err(Gfx950DebugLocalErrorV1::Phase)
        ));
        assert_eq!(calls, expected[..=fail_at]);
    }
}
#[test]
fn every_panic_boundary_stays_attempting_and_retains_armed_backing() {
    struct Marker(Rc<Cell<usize>>);
    impl Drop for Marker {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    for (fail_at, failed_step) in roster().into_iter().enumerate() {
        let drops = Rc::new(Cell::new(0));
        let mut held =
            super::super::super::retention::RetainNativeOnDropV1::new(Marker(drops.clone()));
        held.retain_before_native_effect();
        let mut c = Cursor::new(7);
        for _ in 0..fail_at {
            c.run(7, |_| Ok(())).unwrap();
        }
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _ = c.run(7, |_| -> Result<(), Gfx950DebugLocalErrorV1> {
                panic!("injected")
            });
        }));
        assert!(panic.is_err());
        assert_eq!(c.phase(), Gfx950DebugLocalPhaseV1::Attempting(failed_step));
        assert!(c.run(7, |_| panic!("retry")).is_err());
        drop(held);
        assert_eq!(drops.get(), 0);
        // One deliberately retained tiny synthetic Marker per finite control.
    }
}
#[test]
fn fork_or_zero_pid_refuses_before_any_native_or_lock_closure() {
    for (owner, observed) in [(7, 8), (7, 0), (0, 0), (0, 7)] {
        let mut c = Cursor::new(owner);
        let before = c.phase();
        assert!(matches!(
            c.run(observed, |_| panic!("must precede lock/ioctl")),
            Err(Gfx950DebugLocalErrorV1::ProcessChanged)
        ));
        assert_eq!(c.phase(), before);
    }
}
#[test]
fn completed_cursor_has_no_restart_or_repeated_retirement() {
    let mut c = Cursor::new(7);
    for _ in 0..61 {
        c.run(7, |_| Ok(())).unwrap();
    }
    assert_eq!(c.phase(), Gfx950DebugLocalPhaseV1::LocalBackingRetired);
    assert!(matches!(
        c.run(7, |_| panic!("restart")),
        Err(Gfx950DebugLocalErrorV1::Phase)
    ));
}
