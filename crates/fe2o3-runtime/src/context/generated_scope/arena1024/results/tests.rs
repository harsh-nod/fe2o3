//! Heap observer/debit controls only, not native completion or DATA settlement.

use super::*;
use std::sync::Arc;

fn account(bytes: u64) -> ResourceCreditAccountV1 {
    ResourceCreditAccountV1::new(
        ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, bytes),
        8,
    )
    .unwrap()
}
fn domain(_: usize) -> Result<crate::RuntimeGeneratedResultDomainV1, RuntimeGfx942ReadbackErrorV1> {
    Ok(crate::RuntimeGeneratedResultDomainV1::from_owner(Arc::new(
        (),
    )))
}

#[test]
fn arena1024_result_precharge_and_original_domain_refusal_leave_no_issued_credit() {
    let short = account(0);
    assert!(prepare(&short, |_| panic!("unfunded original domain callback")).is_err());
    assert_eq!(short.usage().used, ResourceVectorV1::ZERO);
    let funded = account(16 << 20);
    let mut calls = 0;
    struct OriginalDomain(ResourceCreditAccountV1);
    impl Drop for OriginalDomain {
        fn drop(&mut self) {
            assert!(
                self.0.usage().retained_records >= 2,
                "domain/reply payload must be destroyed before refund"
            );
        }
    }
    assert!(
        prepare(&funded, |index| {
            calls += 1;
            if index == 41 {
                Err(RuntimeGfx942ReadbackErrorV1::InvalidStorage)
            } else {
                Ok(crate::RuntimeGeneratedResultDomainV1::from_owner(Arc::new(
                    OriginalDomain(funded.clone()),
                )))
            }
        })
        .is_err()
    );
    assert_eq!(calls, 42);
    assert_eq!(funded.usage().used, ResourceVectorV1::ZERO);
    assert_eq!(funded.usage().retained_records, 0);
    let original = Arc::new(());
    assert!(
        prepare(&funded, |_| Ok(
            crate::RuntimeGeneratedResultDomainV1::from_owner(Arc::clone(&original))
        ))
        .is_err()
    );
    assert_eq!(funded.usage().used, ResourceVectorV1::ZERO);
}

#[test]
fn arena1024_mixed_copied_cells_decode_once_and_observer_retains_original_reply_debit() {
    let funded = account(16 << 20);
    let PreparedResults { mut cells, payload } = prepare(&funded, domain).unwrap();
    let retained = funded.usage();
    let future = cells[1023].as_mut().unwrap().future.take().unwrap();
    let observer = RuntimeGfx942Arena1024ResultFutureV1 {
        future,
        _identity: Rc::new(()),
        _payload: Rc::clone(&payload),
        invariant: PhantomData,
    };
    let mut copied = vec![false; SLOTS];
    for i in [1023, 7, 511, 0] {
        copied[i] = true;
    }
    let mut order = Vec::new();
    assert_eq!(
        decode_copied(&mut cells, &copied, |index| {
            order.push(index);
            Ok(())
        }),
        4
    );
    assert_eq!(order, [0, 7, 511, 1023]);
    assert_eq!(
        decode_copied(&mut cells, &copied, |_| panic!("second decode")),
        0
    );
    assert!(cells[1].as_ref().unwrap().outcome.is_none());
    assert_eq!(
        funded.usage(),
        retained,
        "copied results do not refund their original storage"
    );
    drop(cells);
    drop(payload);
    assert_eq!(funded.usage().retained_records, 1);
    assert_eq!(
        funded
            .usage()
            .used
            .get(ResourceKindV1::ControlResidentBytes),
        (SLOTS * crate::async_engine::RuntimeAsyncReplyV1::<Outcome>::payload_bytes_v1()) as u64
    );
    drop(observer);
    assert_eq!(funded.usage().used, ResourceVectorV1::ZERO);
}

#[test]
fn arena1024_decoder_error_and_panic_never_decode_or_mark_an_uncopied_cell() {
    let funded = account(16 << 20);
    let PreparedResults { mut cells, payload } = prepare(&funded, domain).unwrap();
    let mut copied = vec![false; SLOTS];
    copied[19] = true;
    assert_eq!(
        decode_copied(&mut cells, &copied, |index| {
            assert_eq!(index, 19);
            Err(RuntimeGfx942ReadbackErrorV1::InvalidStorage)
        }),
        1
    );
    assert!(matches!(cells[19].as_ref().unwrap().outcome, Some(Err(_))));
    copied[20] = true;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        decode_copied(&mut cells, &copied, |index| {
            assert_eq!(index, 20);
            panic!("decoder panic")
        });
    }));
    assert!(result.is_err());
    assert!(cells[20].as_ref().unwrap().outcome.is_none());
    assert!(cells[21].as_ref().unwrap().outcome.is_none());
    assert_eq!(funded.usage().retained_records, 2);
    drop(cells);
    drop(payload);
    assert_eq!(funded.usage().used, ResourceVectorV1::ZERO);
}

struct ObservedDomain {
    account: ResourceCreditAccountV1,
    disposed: Arc<std::sync::atomic::AtomicUsize>,
}

impl Drop for ObservedDomain {
    fn drop(&mut self) {
        assert!(
            self.account.usage().retained_records >= 2,
            "pre-root reply cells must precede their original payload debit"
        );
        self.disposed
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

fn observed_results(
    funded: &ResourceCreditAccountV1,
    disposed: &Arc<std::sync::atomic::AtomicUsize>,
) -> PreparedResults {
    prepare(funded, |_| {
        Ok(crate::RuntimeGeneratedResultDomainV1::from_owner(Arc::new(
            ObservedDomain {
                account: funded.clone(),
                disposed: Arc::clone(disposed),
            },
        )))
    })
    .unwrap()
}

#[test]
fn arena1024_pre_root_capacity_refusal_disposes_cells_before_reply_debit() {
    let funded = account(16 << 20);
    let disposed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let refusal = (|| -> Result<(), RuntimeGfx942ScopeErrorV1> {
        let _results = observed_results(&funded, &disposed);
        let _later = HostMetadataTableV1::try_new(32 << 20, Some(&funded), || false)
            .map_err(|_| RuntimeGfx942ScopeErrorV1::Capacity)?;
        panic!("unfunded later admission must refuse before native entry");
    })();
    assert!(matches!(refusal, Err(RuntimeGfx942ScopeErrorV1::Capacity)));
    assert_eq!(disposed.load(std::sync::atomic::Ordering::SeqCst), SLOTS);
    assert_eq!(funded.usage().used, ResourceVectorV1::ZERO);
    assert_eq!(funded.usage().retained_records, 0);
}

#[test]
fn arena1024_pre_root_unwind_disposes_cells_and_quarantines_original_reply_debit() {
    let funded = account(16 << 20);
    let disposed = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _results = observed_results(&funded, &disposed);
        panic!("later pre-root admission panic");
    }));
    assert!(unwind.is_err());
    assert_eq!(disposed.load(std::sync::atomic::Ordering::SeqCst), SLOTS);
    assert_eq!(
        funded.usage().used,
        ResourceVectorV1::ZERO.with(
            ResourceKindV1::ControlResidentBytes,
            (SLOTS * crate::async_engine::RuntimeAsyncReplyV1::<Outcome>::payload_bytes_v1())
                as u64,
        )
    );
}
