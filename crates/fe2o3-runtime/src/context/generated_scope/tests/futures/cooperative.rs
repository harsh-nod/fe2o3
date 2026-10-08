//! CPU scheduling controls over original scoped lifecycle owners, not GPU evidence.
use super::*;

#[test]
fn advancing_driver_performs_one_scan_per_poll_and_finishes_without_extra_yield() {
    let mut context = context();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let (mut scope, tickets) = fixture(&mut context, &decoded, &dropped, &[0, 0]);
    let wake = Arc::new(Counter(AtomicUsize::new(0)));
    let waker = Waker::from(wake.clone());
    {
        let mut driver = std::pin::pin!(scope.drive_with_wake_v1(|_| -> std::future::Ready<()> {
            panic!("every nonterminal scan advances")
        }));
        assert!(poll(&mut driver, &waker).is_pending());
        assert_eq!((decoded.get(), dropped.get()), (0, 0));
        assert_eq!(wake.0.load(Ordering::SeqCst), 1);
        assert!(poll(&mut driver, &waker).is_pending());
        assert_eq!((decoded.get(), dropped.get()), (0, 0));
        assert_eq!(wake.0.load(Ordering::SeqCst), 2);
        assert!(matches!(poll(&mut driver, &waker), Poll::Ready(Ok(()))));
        assert_eq!((decoded.get(), dropped.get()), (2, 2));
        assert_eq!(wake.0.load(Ordering::SeqCst), 2);
    }
    assert_eq!(scope.pending_v1(), 0);
    drop((tickets, scope));
    assert!(context.cleanup().is_complete());
}

#[test]
fn ready_idle_wake_cannot_repeat_roster_scans_in_one_poll() {
    let mut context = context();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let (mut scope, tickets) = fixture(&mut context, &decoded, &dropped, &[8]);
    let waits = Cell::new(0);
    let wake = Arc::new(Counter(AtomicUsize::new(0)));
    let waker = Waker::from(wake.clone());
    {
        let mut driver = std::pin::pin!(scope.drive_with_wake_v1(|_| {
            waits.set(waits.get() + 1);
            std::future::ready(())
        }));
        assert!(poll(&mut driver, &waker).is_pending());
        assert_eq!(waits.get(), 0);
        for expected in 1..=8 {
            assert!(poll(&mut driver, &waker).is_pending());
            assert_eq!(waits.get(), expected);
            assert_eq!(wake.0.load(Ordering::SeqCst), expected + 1);
            assert_eq!((decoded.get(), dropped.get()), (0, 0));
        }
        assert!(poll(&mut driver, &waker).is_pending());
        assert_eq!(waits.get(), 8);
        assert_eq!(wake.0.load(Ordering::SeqCst), 10);
        assert!(matches!(poll(&mut driver, &waker), Poll::Ready(Ok(()))));
        assert_eq!(wake.0.load(Ordering::SeqCst), 10);
    }
    assert_eq!((decoded.get(), dropped.get()), (1, 1));
    drop((tickets, scope));
    assert!(context.cleanup().is_complete());
}

#[test]
fn already_settled_driver_neither_waits_nor_self_wakes() {
    let mut context = context();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let (mut scope, tickets) = fixture(&mut context, &decoded, &dropped, &[0]);
    scope.drain_v1().unwrap();
    let wake = Arc::new(Counter(AtomicUsize::new(0)));
    {
        let mut driver = std::pin::pin!(scope.drive_with_wake_v1(|_| -> std::future::Ready<()> {
            panic!("settled scopes must not wait")
        }));
        assert!(matches!(
            poll(&mut driver, &Waker::from(wake.clone())),
            Poll::Ready(Ok(()))
        ));
    }
    assert_eq!(wake.0.load(Ordering::SeqCst), 0);
    drop((tickets, scope));
    assert!(context.cleanup().is_complete());
}

fn executor_fairness(tokio: bool) {
    const COUNT: usize = 4096;
    let mut context = context();
    let decoded = Rc::new(Cell::new(0));
    let dropped = Cell::new(0);
    let ticks: Vec<_> = (0..COUNT).map(|index| index % 8).collect();
    let (mut scope, tickets) = fixture_with_timeout(
        &mut context,
        &decoded,
        &dropped,
        &ticks,
        Duration::from_secs(120),
    );
    let observed = Rc::new(Cell::new(None));
    let sibling = {
        let observed = Rc::clone(&observed);
        let decoded = Rc::clone(&decoded);
        async move { observed.set(Some(decoded.get())) }
    };
    if tokio {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let local = tokio::task::LocalSet::new();
        let sibling = local.spawn_local(sibling);
        runtime.block_on(local.run_until(async {
            scope
                .drive_with_wake_v1(|_| std::future::ready(()))
                .await
                .unwrap();
            sibling.await.unwrap();
        }));
    } else {
        use futures_util::task::LocalSpawnExt;
        let mut pool = futures_executor::LocalPool::new();
        pool.spawner().spawn_local(sibling).unwrap();
        pool.run_until(scope.drive_with_wake_v1(|_| std::future::ready(())))
            .unwrap();
        pool.run_until_stalled();
    }
    assert!(observed.get().is_some_and(|decoded| decoded < COUNT));
    assert_eq!((decoded.get(), dropped.get()), (COUNT, COUNT));
    assert_eq!(scope.pending_v1(), 0);
    for ticket in &tickets {
        assert!(matches!(scope.completion_v1(ticket).unwrap(), Some(Ok(()))));
    }
    drop((tickets, scope));
    assert!(!context.has_unpublished_holds_v1());
    assert!(context.cleanup().is_complete());
}

#[test]
fn advancing_4096_owner_driver_yields_to_tokio_current_thread_sibling() {
    executor_fairness(true);
}

#[test]
fn advancing_4096_owner_driver_yields_to_local_pool_sibling() {
    executor_fairness(false);
}
