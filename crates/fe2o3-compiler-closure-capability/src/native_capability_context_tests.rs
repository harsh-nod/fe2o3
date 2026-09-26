use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    cell::Cell,
    os::unix::fs::MetadataExt,
    panic::{AssertUnwindSafe, catch_unwind},
};

const N: usize = 4;
const EXTRA: usize = 19;
const LIMIT: usize = 1_000_000;
const DECODE_WORK: usize = 32;
const DECODE_STORAGE: usize = 17;
type Cap = NativeCapability<Probe, N>;

struct Probe([u8; N]);
struct Context {
    storage: usize,
    unwind: bool,
    calls: Cell<usize>,
}
impl Record<N> for Probe {
    type Context<'a> = &'a Context;
    const ROLE: CapabilityRole = CapabilityRole {
        name: "contextual transport probe",
        memfd_name: "fe2o3-context-test",
    };
    fn bytes(&self) -> &[u8; N] {
        &self.0
    }
    fn retained_storage(&self) -> usize {
        size_of::<(Self, Storage)>()
    }
    fn context_storage(context: &Context) -> Result<usize> {
        Ok(context.storage)
    }
    fn decode_retained(
        bytes: &[u8; N],
        context: &Context,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        context.calls.set(context.calls.get() + 1);
        assert_eq!(bytes, &[5; N]);
        budget.charge_work(DECODE_WORK)?;
        budget.reserve_storage(DECODE_STORAGE)?;
        if context.unwind {
            panic!("contextual decoder unwind");
        }
        Err(CompilerExecutionCapabilityErrorV2::Rejected(
            "contextual decoder refusal",
        ))
    }
}
fn references(witness: &File) -> usize {
    let expected = witness.metadata().unwrap();
    std::fs::read_dir("/proc/self/fd")
        .unwrap()
        .filter_map(|entry| std::fs::metadata(entry.ok()?.path()).ok())
        .filter(|m| (m.dev(), m.ino()) == (expected.dev(), expected.ino()))
        .count()
}

#[test]
fn context_floor_and_overflow_refuse_before_reading_or_calling_the_decoder() {
    for overflow in [false, true] {
        let context = Context {
            storage: if overflow { usize::MAX } else { 64 },
            unwind: false,
            calls: Cell::new(0),
        };
        let file = tests::sealed(&[5; N]);
        let witness = file.try_clone().unwrap();
        let floor = Cap::FILE_STORAGE + if overflow { 0 } else { context.storage - 1 };
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        b.reserve_storage(floor).unwrap();
        let error = tests::failure(Cap::from_file(file, &context, &mut b));
        if overflow {
            assert!(matches!(
                error,
                CompilerExecutionCapabilityErrorV2::Resource(Resource::Arithmetic)
            ));
        } else {
            assert!(matches!(
                error,
                CompilerExecutionCapabilityErrorV2::Resource(Resource::Accounting)
            ));
        }
        assert_eq!(context.calls.get(), 0);
        assert_eq!((b.storage(), b.peak_storage()), (floor, floor));
        assert_eq!(b.work(), if overflow { 0 } else { ENTRY_WORK });
        assert_eq!(references(&witness), 1);
    }
}

#[test]
fn contextual_decoder_refusal_and_unwind_restore_storage_without_refunding_history() {
    for unwind in [false, true] {
        let context = Context {
            storage: size_of::<Context>(),
            unwind,
            calls: Cell::new(0),
        };
        let file = tests::sealed(&[5; N]);
        let witness = file.try_clone().unwrap();
        let floor = 2 * Cap::FILE_STORAGE + context.storage + EXTRA;
        let mut w = Work::new(LIMIT);
        let mut b = Budget::new(&mut w, LIMIT);
        b.reserve_storage(floor).unwrap();
        b.charge_work(EXTRA).unwrap();
        assert!(b.charge_work(LIMIT).is_err());
        assert!(b.reserve_storage(LIMIT).is_err());
        let ledger = b.work_ledger_identity_v1();
        let result = catch_unwind(AssertUnwindSafe(|| Cap::from_file(file, &context, &mut b)));
        if unwind {
            let panic = match result {
                Err(panic) => panic,
                Ok(_) => panic!("missing decoder unwind"),
            };
            assert_eq!(
                panic.downcast_ref::<&str>().copied(),
                Some("contextual decoder unwind")
            );
        } else {
            assert!(matches!(
                tests::failure(result.unwrap()),
                CompilerExecutionCapabilityErrorV2::Rejected("contextual decoder refusal")
            ));
        }
        assert_eq!(context.calls.get(), 1);
        assert_eq!(b.storage(), floor);
        assert_eq!(b.work(), EXTRA + Cap::IO_WORK + DECODE_WORK);
        assert_eq!(b.peak_storage(), floor + Cap::IO_STORAGE + DECODE_STORAGE);
        assert_eq!(b.failed_work(), Some(EXTRA + LIMIT));
        assert_eq!(b.failed_storage(), Some(floor + LIMIT));
        assert!(b.work_ledger_identity_v1() == ledger);
        assert_eq!(references(&witness), 1);
        drop(witness);
        b.release_storage(2 * Cap::FILE_STORAGE).unwrap();
        assert_eq!(b.storage(), context.storage + EXTRA);
    }
}
