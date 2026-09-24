use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::cell::Cell;

#[derive(Clone, Copy)]
enum Fault {
    Storage,
    Work,
    LateError,
    LatePanic,
}
thread_local! {
    static FACTORY_FAULT: Cell<Option<Fault>> = const { Cell::new(None) };
    static FACTORY_HITS: Cell<usize> = const { Cell::new(0) };
}
struct ResetFault;
impl Drop for ResetFault {
    fn drop(&mut self) {
        FACTORY_FAULT.set(None);
    }
}
pub(super) fn factory_checkpoint() -> R<()> {
    match FACTORY_FAULT.take() {
        Some(Fault::LateError) => {
            FACTORY_HITS.set(FACTORY_HITS.get() + 1);
            Err(E::Mismatch("late factory test failure"))
        }
        Some(Fault::LatePanic) => {
            FACTORY_HITS.set(FACTORY_HITS.get() + 1);
            panic!("late factory test panic");
        }
        None => Ok(()),
        _ => panic!("quota failures must not arm the factory hook"),
    }
}

impl Output {
    pub(crate) fn source_test_consuming_failures_v3(
        &self,
        mut fresh: impl FnMut(&mut Budget<'_>) -> (Input, usize),
        work_limit: usize,
        storage_limit: usize,
    ) -> usize {
        let modes = [
            Fault::Storage,
            Fault::Work,
            Fault::LateError,
            Fault::LatePanic,
        ];
        for mode in modes {
            let mut work = Work::new(work_limit);
            let mut budget = Budget::new(&mut work, storage_limit);
            budget.reserve_storage(53).unwrap();
            budget.charge_work(17).unwrap();
            let ledger = budget.work_ledger_identity_v1();
            let (input, input_storage) = fresh(&mut budget);
            assert_eq!(budget.storage(), 53);
            assert_eq!(input.retained_storage_floor_v1(), 53 + input_storage);
            assert_eq!(
                input.source_test_is_erased_v3(),
                self.input.source_test_is_erased_v3()
            );
            assert_eq!(input.canonical_bytes(), self.input.canonical_bytes());
            budget.reserve_storage(input_storage).unwrap();
            let mut ballast = 0;
            assert!(FACTORY_FAULT.get().is_none());
            FACTORY_HITS.set(0);
            let reset = ResetFault;
            match mode {
                Fault::Storage => {
                    let guard = 2 * size_of::<usize>()
                        + size_of::<CanonicalKernelIrWorkLedgerIdentityV1>()
                        + size_of::<[Option<Box<dyn Any + Send>>; 2]>()
                        + size_of::<Result<(Output, NominalPolicy4NativeStorageV3), E>>();
                    ballast = storage_limit - guard + 1 - budget.storage();
                    budget.reserve_storage(ballast).unwrap();
                }
                Fault::Work => {
                    budget.charge_work(work_limit - budget.work() - 3).unwrap();
                }
                Fault::LateError | Fault::LatePanic => FACTORY_FAULT.set(Some(mode)),
            }
            let floor = budget.storage();
            let before_work = budget.work();
            let error = match input.into_nominal_native_transport_v3(&mut budget) {
                Ok(_) => panic!("injected consuming failure was ignored"),
                Err(error) => error,
            };
            assert!(FACTORY_FAULT.get().is_none());
            drop(reset);
            match (mode, error) {
                (Fault::Storage, E::Resource(Resource::Storage(error))) => {
                    assert_eq!(
                        (error.actual(), error.limit()),
                        (storage_limit + 1, storage_limit)
                    );
                    assert_eq!(budget.failed_storage(), Some(storage_limit + 1));
                    assert_eq!(budget.work(), before_work);
                    assert_eq!(FACTORY_HITS.get(), 0);
                }
                (Fault::Work, E::Resource(Resource::Work(error))) => {
                    assert_eq!(
                        (error.actual(), error.limit()),
                        (work_limit + 1, work_limit)
                    );
                    assert_eq!(budget.work(), before_work);
                    assert_eq!(FACTORY_HITS.get(), 0);
                }
                (Fault::LateError, E::Mismatch("late factory test failure"))
                | (Fault::LatePanic, E::Panicked) => {
                    assert_eq!(FACTORY_HITS.get(), 1);
                    assert!(budget.work() > before_work);
                }
                (_, other) => panic!("wrong consuming failure: {other:?}"),
            }
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            budget.release_storage(input_storage).unwrap();
            budget.release_storage(ballast).unwrap();
            assert_eq!(budget.storage(), 53);
        }
        self.source_test_source_proof_quota_failures_v3(fresh, work_limit, storage_limit);
        modes.len()
    }

    // Run on genuine source-produced owners, not fabricated authenticated graphs.
    pub(crate) fn source_test_receipt_v3(
        &self,
        receipt: NominalPolicy4NativeStorageV3,
        incoming: usize,
        buffers: [(*const u8, usize); 2],
    ) {
        let (text, vectors) = self.module.allocation_parts_for_test_v3();
        let mut heap = text.capacity();
        for rows in vectors {
            heap = heap
                .checked_add(rows.capacity().checked_mul(size_of::<String>()).unwrap())
                .unwrap();
            for name in rows {
                heap = heap.checked_add(name.capacity()).unwrap();
            }
        }
        let added = size_of::<Output>()
            .checked_sub(size_of::<Input>())
            .unwrap()
            .checked_add(heap)
            .unwrap();
        assert_eq!(self.module_storage, size_of::<Module>() + heap);
        assert_eq!(receipt.retained_storage(), added);
        assert_eq!(self.input_floor, incoming);
        assert_eq!(self.retained_floor, incoming + added);
        assert!(incoming >= self.input.retained_storage_floor_v1());
        let descriptor = self.descriptor_source().canonical_bytes();
        let output = self.output().canonical().canonical_bytes();
        assert_eq!(
            [
                (descriptor.as_ptr(), descriptor.len()),
                (output.as_ptr(), output.len())
            ],
            buffers
        );
    }

    pub(crate) fn source_test_rejections_v3(&mut self, budget: &mut Budget<'_>) {
        let floor = budget.storage();
        let ledger = budget.work_ledger_identity_v1();
        self.module_storage += 1;
        assert!(matches!(
            self.verify_equivalence(budget),
            Err(E::Mismatch("exact cumulative P4 native receipt"))
        ));
        self.module_storage -= 1;
        assert_eq!(budget.storage(), floor);

        let first = self.module.llvm_ir().as_bytes()[0];
        let changed = if first == b'#' { b'!' } else { b'#' };
        assert_eq!(
            self.module.replace_first_ascii_byte_for_test_v3(changed),
            first
        );
        assert!(matches!(
            self.verify_equivalence(budget),
            Err(E::Catalog(SourcePipelineCatalogCallbackErrorV1::Callback(
                NativeError::Invalid("exact native/descriptor text")
            )))
        ));
        assert_eq!(
            self.module.replace_first_ascii_byte_for_test_v3(first),
            changed
        );
        self.verify_equivalence(budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert!(budget.work_ledger_identity_v1() == ledger);

        let mut work = Work::new(usize::MAX);
        let mut short = Budget::new(&mut work, usize::MAX);
        short.reserve_storage(self.retained_floor - 1).unwrap();
        assert!(matches!(
            self.verify_equivalence(&mut short),
            Err(E::Resource(Resource::Accounting))
        ));
        assert_eq!(short.work(), 0);
        assert_eq!(short.storage(), self.retained_floor - 1);

        // Derive the first guard reservation independently of the production constant.
        let guard = 2 * size_of::<usize>()
            + size_of::<CanonicalKernelIrWorkLedgerIdentityV1>()
            + size_of::<[Option<Box<dyn Any + Send>>; 2]>()
            + size_of::<Result<(), NominalPolicy4NativeErrorV3>>();
        let attempted = self.retained_floor.checked_add(guard).unwrap();
        let mut work = Work::new(usize::MAX);
        let mut short = Budget::new(&mut work, attempted - 1);
        short.reserve_storage(self.retained_floor).unwrap();
        match self.verify_equivalence(&mut short) {
            Err(E::Resource(Resource::Storage(error))) => {
                assert_eq!((error.actual(), error.limit()), (attempted, attempted - 1));
            }
            other => panic!("native replay guard quota: {other:?}"),
        }
        assert_eq!(short.work(), 0);
        assert_eq!(short.storage(), self.retained_floor);
        assert_eq!(short.failed_storage(), Some(attempted));

        // Deny the fifth stored-symbol vector before any source or descriptor replay.
        let (_, vectors) = self.module.allocation_parts_for_test_v3();
        let costs = vectors.map(|rows| rows.len() + 2);
        let seed = 7;
        let attempted = seed + 4 + costs.iter().sum::<usize>();
        let accepted = seed + 4 + costs[..4].iter().sum::<usize>();
        let mut work = Work::new(attempted - 1);
        let mut short = Budget::new(&mut work, usize::MAX);
        short.reserve_storage(self.retained_floor).unwrap();
        short.charge_work(seed).unwrap();
        match self.verify_equivalence(&mut short) {
            Err(E::Module(module::NominalModuleErrorV3::Resource(Resource::Work(error)))) => {
                assert_eq!((error.actual(), error.limit()), (attempted, attempted - 1));
            }
            other => panic!("native symbol-prefix work quota: {other:?}"),
        }
        assert_eq!(short.work(), accepted);
        assert_eq!(short.storage(), self.retained_floor);
        drop(short);
        assert_eq!(work.failed_work(), Some(attempted));
    }
}

#[test]
fn native_scopes_preserve_floor_on_success_error_and_panic() {
    for mode in 0..4 {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 1024 * 1024);
        budget.reserve_storage(53).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let result = scoped(&mut budget, |budget| {
            budget.charge_work(3)?;
            budget.reserve_storage(19)?;
            match mode {
                0 => Ok(()),
                1 => Err(E::Mismatch("test failure")),
                2 => panic!("test panic"),
                _ => {
                    budget.release_storage(budget.storage() - 53).unwrap();
                    panic!("released guard before panic");
                }
            }
        });
        match mode {
            0 => assert!(result.is_ok()),
            1 => assert!(matches!(result, Err(E::Mismatch("test failure")))),
            2 => assert!(matches!(result, Err(E::Panicked))),
            _ => assert!(matches!(result, Err(E::Resource(Resource::Accounting)))),
        }
        assert_eq!(budget.storage(), 53);
        assert_eq!(budget.work(), 3);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
}

#[test]
fn native_scopes_never_refund_replacement_ledger() {
    let mut work = Work::new(100);
    let mut other_work = Work::new(100);
    let mut budget = Budget::new(&mut work, 1024 * 1024);
    let mut replacement = Budget::new(&mut other_work, 1024 * 1024);
    budget.reserve_storage(53).unwrap();
    replacement.reserve_storage(61).unwrap();
    let original = budget.work_ledger_identity_v1();
    let other = replacement.work_ledger_identity_v1();
    let result = scoped(&mut budget, |budget| {
        std::mem::swap(budget, &mut replacement);
        Ok(())
    });
    assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
    assert_eq!(budget.storage(), 61);
    assert!(budget.work_ledger_identity_v1() == other);
    assert!(replacement.work_ledger_identity_v1() == original);
    assert!(replacement.storage() > 53);
}

#[test]
fn native_scope_payload_destructor_unwinds_after_cleanup() {
    struct Payload;
    impl Drop for Payload {
        fn drop(&mut self) {
            panic!("payload destructor");
        }
    }
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 1024 * 1024);
    budget.reserve_storage(53).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let escaped = catch_unwind(AssertUnwindSafe(|| {
        scoped::<()>(&mut budget, |budget| {
            budget.reserve_storage(19)?;
            std::panic::panic_any(Payload);
        })
    }));
    assert!(escaped.is_err());
    assert_eq!(budget.storage(), 53);
    assert!(budget.work_ledger_identity_v1() == ledger);
}
