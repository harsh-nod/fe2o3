use super::*;
use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, Waker},
};

type Owned = CanonicalKernelIrOwnedVerificationResourceBudgetV1;
type Work = CanonicalKernelIrWorkBudgetV1;
type Budget<'work> = CanonicalKernelIrVerificationResourceBudgetV1<'work>;

fn poll<F: Future>(future: Pin<&mut F>) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(Waker::noop()))
}

async fn suspend_once() {
    let mut suspended = false;
    std::future::poll_fn(|_| {
        if suspended {
            Poll::Ready(())
        } else {
            suspended = true;
            Poll::Pending
        }
    })
    .await
}

#[test]
fn owned_async_scope_preserves_original_identity_and_cumulative_prefix_across_awaits() {
    let mut account = Owned::new(Work::new(20), 12);
    account.with_budget(|budget| {
        budget.charge_work(3).unwrap();
        budget.reserve_storage(2).unwrap();
    });
    let stages = Cell::new(0);
    let mut future = Box::pin(account.into_budget_scope_async_v1(async |budget| {
        let ledger = budget.work_ledger_identity_v1();
        let storage = budget.storage_account_identity_v1();
        assert!(storage.is_some());
        let view = &*budget as *const Budget<'_>;
        assert_eq!((budget.work(), budget.storage()), (3, 2));
        budget.charge_work(4).unwrap();
        budget.reserve_storage(5).unwrap();
        stages.set(1);
        suspend_once().await;
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage_account_identity_v1(), storage);
        assert_eq!(&*budget as *const Budget<'_>, view);
        assert_eq!((budget.work(), budget.storage()), (7, 7));
        assert!(budget.charge_work(14).is_err());
        assert!(budget.reserve_storage(6).is_err());
        stages.set(2);
        suspend_once().await;
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage_account_identity_v1(), storage);
        assert_eq!(&*budget as *const Budget<'_>, view);
        assert_eq!((budget.work(), budget.storage()), (7, 7));
        budget.release_storage(3).unwrap();
        stages.set(3);
        "same account"
    }));
    assert!(poll(future.as_mut()).is_pending());
    assert_eq!(stages.get(), 1);
    assert!(poll(future.as_mut()).is_pending());
    assert_eq!(stages.get(), 2);
    let Poll::Ready((mut account, result)) = poll(future.as_mut()) else {
        panic!("account callback did not finish");
    };
    assert_eq!(stages.get(), 3);
    assert_eq!(result, "same account");
    assert_eq!(
        (account.work(), account.storage(), account.peak_storage()),
        (7, 4, 7)
    );
    assert_eq!(account.failed_work(), Some(21));
    assert_eq!(account.failed_storage(), Some(13));
    account.with_budget(|budget| {
        budget.charge_work(1).unwrap();
        assert_eq!((budget.work(), budget.storage()), (8, 4));
        assert_eq!(budget.failed_storage(), Some(13));
    });
}

struct Captured(Rc<Cell<usize>>);
impl Drop for Captured {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

struct Borrowed<'a, 'work> {
    budget: &'a mut Budget<'work>,
    observed: Rc<RefCell<Vec<(usize, usize)>>>,
}
impl Drop for Borrowed<'_, '_> {
    fn drop(&mut self) {
        // Reading the original view in Drop requires callback owners to retire
        // before the future releases the borrowed view and owned account.
        self.observed
            .borrow_mut()
            .push((self.budget.work(), self.budget.storage()));
    }
}

#[test]
fn owned_async_scope_unpolled_cancellation_never_enters_callback() {
    let entered = Rc::new(Cell::new(false));
    let dropped = Rc::new(Cell::new(0));
    let capture = Captured(Rc::clone(&dropped));
    let marker = Rc::clone(&entered);
    let future = Owned::new(Work::new(20), 12).into_budget_scope_async_v1(async move |budget| {
        marker.set(true);
        budget.charge_work(1).unwrap();
        std::future::pending::<()>().await;
        drop(capture);
    });
    drop(future);
    assert!(!entered.get());
    assert_eq!(dropped.get(), 1);
}

#[test]
fn owned_async_scope_cancellation_drops_borrowed_owners_before_original_account() {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let trace = Rc::clone(&observed);
    let mut future = Box::pin(Owned::new(Work::new(20), 12).into_budget_scope_async_v1(
        async move |budget| {
            budget.charge_work(7).unwrap();
            budget.reserve_storage(9).unwrap();
            let owner = Borrowed {
                budget,
                observed: trace,
            };
            std::future::pending::<()>().await;
            drop(owner);
        },
    ));
    assert!(poll(future.as_mut()).is_pending());
    assert!(observed.borrow().is_empty());
    drop(future);
    assert_eq!(*observed.borrow(), [(7, 9)]);
}

#[test]
fn owned_async_scope_early_error_returns_original_charges_without_refund() {
    let mut future = Box::pin(Owned::new(Work::new(20), 12).into_budget_scope_async_v1(
        async |budget| {
            budget.charge_work(7).unwrap();
            budget.reserve_storage(9).unwrap();
            Err::<(), _>("original refusal")
        },
    ));
    let Poll::Ready((account, result)) = poll(future.as_mut()) else {
        panic!("immediate callback unexpectedly suspended");
    };
    assert_eq!(result, Err("original refusal"));
    assert_eq!((account.work(), account.storage()), (7, 9));
}

#[test]
fn owned_async_scope_forgetting_polled_future_retains_its_callback_and_account() {
    let observed = Rc::new(RefCell::new(Vec::new()));
    let trace = Rc::clone(&observed);
    let mut future = Box::pin(Owned::new(Work::new(20), 12).into_budget_scope_async_v1(
        async move |budget| {
            budget.charge_work(7).unwrap();
            budget.reserve_storage(9).unwrap();
            let owner = Borrowed {
                budget,
                observed: trace,
            };
            std::future::pending::<()>().await;
            drop(owner);
        },
    ));
    assert!(poll(future.as_mut()).is_pending());
    // Deliberately leak this tiny CPU-only future. It owns the account itself;
    // there is no external account reference to recover, move or reset afterward.
    std::mem::forget(future);
    assert!(observed.borrow().is_empty());
    assert_eq!(Rc::strong_count(&observed), 2);
}
