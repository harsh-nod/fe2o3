//! Native consumer of the shared singleton and crash-recovery engine.
use super::worker::{ANCHOR, WORKER, WorkerRecord};
use super::*;
use crate::compiler_execution_journal_recovery::{
    JournalNames, LEGACY_STATE_FILES, NATIVE_ISSUER_STATE_FILES, Recovery, RecoveryPlan,
    SingletonLock,
};
use fe2o3_artifact_transaction::{
    MeteredRetainedDurableDirectoryV2 as MeteredDirectory,
    NoRetainedDurableDirectoryHooksV1 as NoHooks, RetainedDurableDirectoryHooksV1 as Hooks,
    RetainedDurableDirectoryV1 as Directory,
};
use fe2o3_compiler_execution_protocol::CompilerExecutionWorkerAnchorJournalV2 as AnchorJournal;
use std::os::fd::BorrowedFd;

const JOURNAL: JournalNames = JournalNames {
    canonical: NATIVE_ISSUER_STATE_FILES[0],
    redo: NATIVE_ISSUER_STATE_FILES[1],
    recovery: NATIVE_ISSUER_STATE_FILES[2],
    maximum_bytes: record::BYTES,
};
// Legacy records cannot be interpreted as absence or upgraded into native owners.
const UNJOINED: [&str; 9] = [
    "compiler-execution-issuer-v2.state",
    "compiler-execution-issuer-v2.redo",
    "compiler-execution-issuer-v2.recovery",
    "compiler-execution-worker-v2.state",
    "compiler-execution-worker-v2.redo",
    "compiler-execution-worker-v2.recovery",
    "compiler-execution-worker-anchor-v1.state",
    "compiler-execution-worker-anchor-v1.redo",
    "compiler-execution-worker-anchor-v1.recovery",
];
const FRAME: usize = 32 * Ledger::STORAGE + 65536;
const WORK: usize = 1024 * Ledger::STORAGE + 65536;

pub(super) trait Stored: Sized {
    const STORAGE: usize;
    fn bytes(&self) -> &[u8];
    fn decode(bytes: &[u8], p: &Policy, b: &mut Budget<'_>) -> Result<Self>;
    fn successor(prior: Option<&Self>, next: &Self, b: &mut Budget<'_>) -> Result<()>;
}
impl Stored for Record {
    const STORAGE: usize = Record::STORAGE;
    fn bytes(&self) -> &[u8] {
        self.bytes()
    }
    fn decode(bytes: &[u8], p: &Policy, b: &mut Budget<'_>) -> Result<Self> {
        Record::decode(bytes, p, b)
    }
    fn successor(prior: Option<&Self>, next: &Self, b: &mut Budget<'_>) -> Result<()> {
        Record::successor(prior, next, b)
    }
}

pub(super) struct Ledger {
    pub record: Record,
    pub(super) worker: Option<WorkerRecord>,
    pub(super) anchor: Option<AnchorJournal>,
    pub(super) store: Directory,
    _lock: SingletonLock,
    pub(super) poisoned: bool,
}
impl Ledger {
    pub const STORAGE: usize =
        std::mem::size_of::<Self>() + MeteredDirectory::DIRECTORY_STORAGE + 128;

    pub fn recover(
        root: BorrowedFd<'_>,
        p: &Policy,
        key: &Key,
        b: &mut Budget<'_>,
    ) -> Result<Self> {
        Self::recover_with_hooks(root, p, key, &mut NoHooks, b)
    }
    pub(super) fn recover_with_hooks(
        root: BorrowedFd<'_>,
        p: &Policy,
        key: &Key,
        hooks: &mut impl Hooks,
        b: &mut Budget<'_>,
    ) -> Result<Self> {
        b.with_prepaid_scope(
            p.retained_storage() + key.retained_storage(),
            8,
            WORK,
            FRAME,
            |b| {
                let lock = SingletonLock::acquire(&root)?;
                let fd = rustix::io::fcntl_dupfd_cloexec(root, 0)?;
                let store = Directory::admit_service_owned(fd)?;
                b.reserve_storage(MeteredDirectory::DIRECTORY_STORAGE)?;
                reject_unjoined(&store, b)?;
                let mut io = NativeRecovery {
                    store: &store,
                    p,
                    hooks,
                    b,
                };
                let issuer: RecoveryPlan<Record> = JOURNAL.plan_using(&mut io)?;
                let worker: RecoveryPlan<WorkerRecord> = WORKER.plan_using(&mut io)?;
                let anchor: RecoveryPlan<AnchorJournal> = ANCHOR.plan_using(&mut io)?;
                if issuer.record.is_none() && (worker.record.is_some() || anchor.record.is_some()) {
                    return Err(Error::rejected(
                        "native Worker/anchor has no issuer journal",
                    ));
                }
                let genesis = if issuer.record.is_none() {
                    Some(Record::genesis(p, key, io.b)?)
                } else {
                    None
                };
                io.b.reserve_storage(Record::STORAGE)?;
                let selected = issuer
                    .record
                    .as_ref()
                    .or(genesis.as_ref())
                    .ok_or_else(|| Error::rejected("native recovery has no issuer state"))?;
                worker::validate_join(
                    selected,
                    worker.record.as_ref(),
                    anchor.record.as_ref(),
                    io.b,
                )?;
                JOURNAL.validate_plan(&issuer, &mut io)?;
                WORKER.validate_plan(&worker, &mut io)?;
                ANCHOR.validate_plan(&anchor, &mut io)?;
                // All three decoded positions and namespaces have passed their
                // joins. Only now may recovery promote or stabilize any record.
                let record = JOURNAL.apply_using(issuer, &mut io)?;
                let worker = WORKER.apply_using(worker, &mut io)?;
                let anchor = ANCHOR.apply_using(anchor, &mut io)?;
                let record = match record {
                    Some(record) => record,
                    None => {
                        let record =
                            genesis.ok_or_else(|| Error::rejected("native genesis missing"))?;
                        MeteredDirectory::new(&store, b).commit_record(
                            JOURNAL.canonical,
                            JOURNAL.redo,
                            record.bytes(),
                            record::BYTES,
                            hooks,
                        )?;
                        record
                    }
                };
                let ledger = Self {
                    record,
                    worker,
                    anchor,
                    store,
                    _lock: lock,
                    poisoned: false,
                };
                b.reserve_storage(Self::STORAGE)?;
                ledger.validate(b)?;
                Ok(ledger)
            },
        )
    }
    pub fn validate(&self, b: &mut Budget<'_>) -> Result<()> {
        b.with_prepaid_scope(Self::STORAGE, 8, WORK, FRAME, |b| {
            if self.poisoned {
                return Err(Error::rejected(
                    "native issuer requires recovery after durable failure",
                ));
            }
            reject_unjoined(&self.store, b)?;
            worker::validate_join(&self.record, self.worker.as_ref(), self.anchor.as_ref(), b)?;
            let bytes = read(&self.store, JOURNAL.canonical, record::BYTES, b)?;
            if bytes.as_deref() != Some(self.record.bytes().as_slice()) {
                return Err(Error::rejected("native canonical issuer record changed"));
            }
            let mut io = MeteredDirectory::new(&self.store, b);
            io.require_absent(JOURNAL.redo)?;
            io.require_absent(JOURNAL.recovery)?;
            validate_named(&self.store, &WORKER, self.worker.as_ref(), b)?;
            validate_named(&self.store, &ANCHOR, self.anchor.as_ref(), b)?;
            Ok(())
        })
    }
    pub fn commit(&mut self, next: Record, b: &mut Budget<'_>) -> Result<()> {
        self.commit_with_hooks(next, &mut NoHooks, b)
    }
    pub(super) fn commit_with_hooks(
        &mut self,
        next: Record,
        hooks: &mut impl Hooks,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        b.with_prepaid_scope(Self::STORAGE + Record::STORAGE, 8, WORK, FRAME, |b| {
            self.validate(b)?;
            Record::successor(Some(&self.record), &next, b)?;
            worker::validate_join(&next, self.worker.as_ref(), self.anchor.as_ref(), b)?;
            self.poisoned = true;
            MeteredDirectory::new(&self.store, b).commit_record(
                JOURNAL.canonical,
                JOURNAL.redo,
                next.bytes(),
                record::BYTES,
                hooks,
            )?;
            self.record = next;
            // Do not clear poison until every exact-byte/sidecar check succeeds.
            let bytes = read(&self.store, JOURNAL.canonical, record::BYTES, b)?;
            if bytes.as_deref() != Some(self.record.bytes().as_slice()) {
                return Err(Error::rejected("native committed issuer bytes changed"));
            }
            MeteredDirectory::new(&self.store, b).require_absent(JOURNAL.redo)?;
            MeteredDirectory::new(&self.store, b).require_absent(JOURNAL.recovery)?;
            validate_named(&self.store, &WORKER, self.worker.as_ref(), b)?;
            validate_named(&self.store, &ANCHOR, self.anchor.as_ref(), b)?;
            self.poisoned = false;
            Ok(())
        })
    }
}
pub(super) fn validate_named<T: Stored>(
    store: &Directory,
    names: &JournalNames,
    expected: Option<&T>,
    b: &mut Budget<'_>,
) -> Result<()> {
    let bytes = read(store, names.canonical, names.maximum_bytes, b)?;
    if bytes.as_deref() != expected.map(Stored::bytes) {
        return Err(Error::rejected("native retained journal bytes changed"));
    }
    let mut io = MeteredDirectory::new(store, b);
    io.require_absent(names.redo)?;
    io.require_absent(names.recovery)?;
    Ok(())
}
pub(super) fn commit_named<T: Stored>(
    store: &Directory,
    names: &JournalNames,
    prior: Option<&T>,
    next: &T,
    hooks: &mut impl Hooks,
    b: &mut Budget<'_>,
) -> Result<()> {
    T::successor(prior, next, b)?;
    validate_named(store, names, prior, b)?;
    MeteredDirectory::new(store, b).commit_record(
        names.canonical,
        names.redo,
        next.bytes(),
        names.maximum_bytes,
        hooks,
    )?;
    validate_named(store, names, Some(next), b)
}
fn reject_unjoined(store: &Directory, b: &mut Budget<'_>) -> Result<()> {
    let mut io = MeteredDirectory::new(store, b);
    for name in LEGACY_STATE_FILES.into_iter().chain(UNJOINED) {
        io.require_absent(name)?;
    }
    Ok(())
}
fn read(
    store: &Directory,
    name: &str,
    maximum: usize,
    b: &mut Budget<'_>,
) -> Result<Option<Vec<u8>>> {
    let bytes = MeteredDirectory::new(store, b).read_private(name, maximum)?;
    if let Some(bytes) = &bytes {
        b.reserve_storage(MeteredDirectory::record_storage(bytes.len())?)?;
    }
    Ok(bytes)
}
struct NativeRecovery<'a, 'work, H> {
    store: &'a Directory,
    p: &'a Policy,
    hooks: &'a mut H,
    b: &'a mut Budget<'work>,
}
impl<T: Stored, H: Hooks> Recovery<T, Error> for NativeRecovery<'_, '_, H> {
    fn read(&mut self, name: &'static str, maximum: usize) -> Result<Option<Vec<u8>>> {
        read(self.store, name, maximum, self.b)
    }
    fn absent(&mut self, name: &'static str) -> Result<()> {
        Ok(MeteredDirectory::new(self.store, self.b).require_absent(name)?)
    }
    fn decode(&mut self, bytes: &[u8]) -> Result<T> {
        let record = T::decode(bytes, self.p, self.b)?;
        self.b.reserve_storage(T::STORAGE)?;
        Ok(record)
    }
    fn successor(&mut self, prior: Option<&T>, next: &T) -> Result<()> {
        T::successor(prior, next, self.b)
    }
    fn promote(
        &mut self,
        names: &JournalNames,
        staged: &'static str,
        prior: Option<&[u8]>,
        bytes: &[u8],
    ) -> Result<()> {
        Ok(
            MeteredDirectory::new(self.store, self.b).promote_validated_redo(
                names.canonical,
                staged,
                prior,
                bytes,
                names.maximum_bytes,
                self.hooks,
            )?,
        )
    }
    fn stabilize(&mut self, names: &JournalNames, bytes: &[u8]) -> Result<()> {
        let held = MeteredDirectory::new(self.store, self.b)
            .establish_recovered_record_durability(
                names.canonical,
                names.recovery,
                bytes,
                names.maximum_bytes,
                self.hooks,
            )?;
        self.b
            .reserve_storage(MeteredDirectory::record_storage(held.len())?)?;
        Ok(())
    }
}
