const SOURCE_OWNED_CALLBACK_WORK_LIMIT_V18: usize = 500_000_000;

#[derive(Debug)]
enum OwnedCallbackErrorV18 {
    Source(ProductionSourceOwnedViewErrorV18),
    Payload(Vec<u64>),
}

impl From<ProductionSourceOwnedViewErrorV18> for OwnedCallbackErrorV18 {
    fn from(error: ProductionSourceOwnedViewErrorV18) -> Self {
        Self::Source(error)
    }
}

fn owned_callback_payload_v18(budget: &mut ArgumentBudgetV1<'_>) -> Vec<u64> {
    budget
        .reserve_storage(64 * std::mem::size_of::<u64>())
        .unwrap();
    let payload = vec![0x271_u64; 64];
    assert_eq!(payload.capacity(), 64);
    payload
}

#[test]
fn source_callback_success_error_and_raw_panic_keep_only_owned_backing() {
    for mode in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SOURCE_OWNED_CALLBACK_WORK_LIMIT_V18);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(scalar_payload_owner_v18, &mut budget);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            prepared.with_source_consumer_v18(&mut budget, |source, budget| {
                source.root_count(budget)?;
                let payload = owned_callback_payload_v18(budget);
                match mode {
                    0 => Ok(payload),
                    1 => Err(OwnedCallbackErrorV18::Payload(payload)),
                    _ => {
                        budget
                            .reserve_storage(std::mem::size_of::<Vec<u64>>())
                            .unwrap();
                        std::panic::resume_unwind(Box::new(payload))
                    }
                }
            })
        }));
        let extra = if mode == 2 {
            std::mem::size_of::<Vec<u64>>()
        } else {
            0
        };
        let backing = 64 * std::mem::size_of::<u64>() + extra;
        assert_eq!(budget.storage(), MODULE_FLOOR + backing);
        match (mode, caught) {
            (0, Ok(Ok(payload))) | (1, Ok(Err(OwnedCallbackErrorV18::Payload(payload)))) => {
                assert_eq!(payload, vec![0x271; 64]);
                drop(payload);
            }
            (2, Err(payload)) => {
                let payload = payload.downcast::<Vec<u64>>().unwrap();
                assert_eq!(*payload, vec![0x271; 64]);
                drop(payload);
            }
            other => panic!("callback disposition changed: {other:?}"),
        }
        budget.release_storage(backing).unwrap();
        assert_eq!(budget.storage(), MODULE_FLOOR);
        assert_eq!(
            (budget.failed_work(), budget.failed_storage()),
            (None, None)
        );
    }
}
