//! Native consumer of the shared singleton and crash-recovery engine.
use super::*;
use crate::compiler_execution_journal_recovery::{
    JournalNames, LEGACY_STATE_FILES, NATIVE_ISSUER_STATE_FILES, Recovery, SingletonLock,
};
use fe2o3_artifact_transaction::{
    MeteredRetainedDurableDirectoryV2 as MeteredDirectory,
    NoRetainedDurableDirectoryHooksV1 as NoHooks, RetainedDurableDirectoryHooksV1 as Hooks,
    RetainedDurableDirectoryV1 as Directory,
};
use std::os::fd::BorrowedFd;

const JOURNAL: JournalNames = JournalNames {
    canonical: NATIVE_ISSUER_STATE_FILES[0],
    redo: NATIVE_ISSUER_STATE_FILES[1],
    recovery: NATIVE_ISSUER_STATE_FILES[2],
    maximum_bytes: record::BYTES,
};
// No native Worker join is implemented by this first-sequence service. Its
// namespace must be absent, not ignored or interpreted as an empty ledger.
const UNJOINED: [&str; 15] = [
    "compiler-execution-issuer-v2.state",
    "compiler-execution-issuer-v2.redo",
    "compiler-execution-issuer-v2.recovery",
    "compiler-execution-worker-v2.state",
    "compiler-execution-worker-v2.redo",
    "compiler-execution-worker-v2.recovery",
    "compiler-execution-worker-anchor-v1.state",
    "compiler-execution-worker-anchor-v1.redo",
    "compiler-execution-worker-anchor-v1.recovery",
    "compiler-execution-worker-v3.state",
    "compiler-execution-worker-v3.redo",
    "compiler-execution-worker-v3.recovery",
    "compiler-execution-worker-anchor-v2.state",
    "compiler-execution-worker-anchor-v2.redo",
    "compiler-execution-worker-anchor-v2.recovery",
];
const FRAME: usize = 12 * Record::STORAGE + 32 * record::BYTES + 65536;
const WORK: usize = 512 * record::BYTES + 65536;

pub(super) struct Ledger {
    pub record: Record,
    store: Directory,
    _lock: SingletonLock,
    poisoned: bool,
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
                let record = JOURNAL.recover_using(&mut NativeRecovery {
                    store: &store,
                    p,
                    hooks,
                    b,
                })?;
                let record = match record {
                    Some(record) => record,
                    None => {
                        let record = Record::genesis(p, key, b)?;
                        b.reserve_storage(Record::STORAGE)?;
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
            let bytes = read(&self.store, JOURNAL.canonical, record::BYTES, b)?;
            if bytes.as_deref() != Some(self.record.bytes().as_slice()) {
                return Err(Error::rejected("native canonical issuer record changed"));
            }
            let mut io = MeteredDirectory::new(&self.store, b);
            io.require_absent(JOURNAL.redo)?;
            io.require_absent(JOURNAL.recovery)?;
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
            self.poisoned = false;
            Ok(())
        })
    }
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
impl<H: Hooks> Recovery<Record, Error> for NativeRecovery<'_, '_, H> {
    fn read(&mut self, name: &'static str, maximum: usize) -> Result<Option<Vec<u8>>> {
        read(self.store, name, maximum, self.b)
    }
    fn absent(&mut self, name: &'static str) -> Result<()> {
        Ok(MeteredDirectory::new(self.store, self.b).require_absent(name)?)
    }
    fn decode(&mut self, bytes: &[u8]) -> Result<Record> {
        let record = Record::decode(bytes, self.p, self.b)?;
        self.b.reserve_storage(Record::STORAGE)?;
        Ok(record)
    }
    fn successor(&mut self, prior: Option<&Record>, next: &Record) -> Result<()> {
        Record::successor(prior, next, self.b)
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
