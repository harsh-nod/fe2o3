//! One namespace state machine for issuer, Worker, and anchor journals.
//!
//! A redo proposes a successor; a recovery file retains an already canonical
//! record. Distinct names preserve that distinction if recovery itself crashes.
use fe2o3_artifact_transaction::{
    RetainedDurableDirectoryErrorV1 as IoError, RetainedDurableDirectoryHooksV1,
    RetainedDurableDirectoryV1,
};
#[path = "compiler_execution_singleton.rs"]
mod singleton;
pub(super) use singleton::SingletonLock;

#[cfg(test)]
pub(super) mod test_support;
#[cfg(test)]
mod tests;

pub(super) const LEGACY_STATE_FILES: [&str; 4] = [
    "compiler-execution-issuer-v1.state",
    "compiler-execution-issuer-v1.redo",
    "compiler-execution-worker-v1.state",
    "compiler-execution-worker-v1.redo",
];
pub(super) const NATIVE_ISSUER_STATE_FILES: [&str; 3] = [
    "compiler-execution-issuer-v3.state",
    "compiler-execution-issuer-v3.redo",
    "compiler-execution-issuer-v3.recovery",
];
pub(super) const NATIVE_WORKER_STATE_FILES: [&str; 3] = [
    "compiler-execution-worker-v3.state",
    "compiler-execution-worker-v3.redo",
    "compiler-execution-worker-v3.recovery",
];
pub(super) const NATIVE_ANCHOR_STATE_FILES: [&str; 3] = [
    "compiler-execution-worker-anchor-v2.state",
    "compiler-execution-worker-anchor-v2.redo",
    "compiler-execution-worker-anchor-v2.recovery",
];

/// The issuer holds the directory singleton lock before this check. Standalone
/// Worker callers must also serialize writers. Never infer absence from decoding.
pub(super) fn reject_legacy_state(store: &RetainedDurableDirectoryV1) -> Result<(), IoError> {
    for name in LEGACY_STATE_FILES
        .into_iter()
        .chain(NATIVE_ISSUER_STATE_FILES)
        .chain(NATIVE_WORKER_STATE_FILES)
        .chain(NATIVE_ANCHOR_STATE_FILES)
    {
        store.require_absent(name)?;
    }
    Ok(())
}

pub(super) struct JournalNames {
    pub canonical: &'static str,
    pub redo: &'static str,
    pub recovery: &'static str,
    pub maximum_bytes: usize,
}

/// A decoded crash position. Planning performs no namespace mutation; consumers
/// joining several journals must validate every plan before applying any of them.
pub(super) struct RecoveryPlan<T> {
    pub record: Option<T>,
    canonical: Option<Vec<u8>>,
    redo: Option<Vec<u8>>,
    recovery: Option<Vec<u8>>,
}

/// Mechanical I/O and nominal decoding for the same recovery state machine.
/// Native implementations own a mutable borrow of the original resource ledger.
pub(super) trait Recovery<T, E> {
    fn read(&mut self, name: &'static str, maximum: usize) -> Result<Option<Vec<u8>>, E>;
    fn absent(&mut self, name: &'static str) -> Result<(), E>;
    fn decode(&mut self, bytes: &[u8]) -> Result<T, E>;
    fn successor(&mut self, prior: Option<&T>, next: &T) -> Result<(), E>;
    fn promote(
        &mut self,
        names: &JournalNames,
        staged: &'static str,
        prior: Option<&[u8]>,
        bytes: &[u8],
    ) -> Result<(), E>;
    fn stabilize(&mut self, names: &JournalNames, bytes: &[u8]) -> Result<(), E>;
}

impl JournalNames {
    /// Decoding and legal successor relations belong to the nominal journal.
    /// This adapter only shares crash ordering, exact-byte checks, and I/O.
    pub fn recover<T, E: From<IoError>>(
        &self,
        store: &RetainedDurableDirectoryV1,
        hooks: &mut impl RetainedDurableDirectoryHooksV1,
        decode: impl Fn(&[u8]) -> Result<T, E>,
        successor: impl Fn(Option<&T>, &T) -> Result<(), E>,
    ) -> Result<Option<T>, E> {
        self.recover_using(&mut LegacyRecovery {
            store,
            hooks,
            decode,
            successor,
        })
    }

    pub fn recover_using<T, E: From<IoError>>(
        &self,
        io: &mut impl Recovery<T, E>,
    ) -> Result<Option<T>, E> {
        let plan = self.plan_using(io)?;
        self.apply_using(plan, io)
    }

    pub fn plan_using<T, E: From<IoError>>(
        &self,
        io: &mut impl Recovery<T, E>,
    ) -> Result<RecoveryPlan<T>, E> {
        let canonical = io.read(self.canonical, self.maximum_bytes)?;
        let redo = io.read(self.redo, self.maximum_bytes)?;
        let recovery = io.read(self.recovery, self.maximum_bytes)?;
        let record = if let Some(bytes) = &recovery {
            // A recovery rename never coexists with either transaction name.
            // Even byte-identical duplicates are not a reachable crash position.
            if canonical.is_some() || redo.is_some() {
                return Err(IoError::ExistingEntry {
                    entry: self.recovery.to_owned(),
                }
                .into());
            }
            Some(io.decode(bytes)?)
        } else {
            match (&canonical, &redo) {
                (None, None) => None,
                (canonical, Some(bytes)) => {
                    let record = io.decode(bytes)?;
                    let prior = canonical
                        .as_deref()
                        .map(|bytes| io.decode(bytes))
                        .transpose()?;
                    io.successor(prior.as_ref(), &record)?;
                    Some(record)
                }
                (Some(bytes), None) => Some(io.decode(bytes)?),
            }
        };
        Ok(RecoveryPlan {
            record,
            canonical,
            redo,
            recovery,
        })
    }

    pub fn validate_plan<T, E: From<IoError>>(
        &self,
        plan: &RecoveryPlan<T>,
        io: &mut impl Recovery<T, E>,
    ) -> Result<(), E> {
        for (name, expected) in [
            (self.canonical, &plan.canonical),
            (self.redo, &plan.redo),
            (self.recovery, &plan.recovery),
        ] {
            if io.read(name, self.maximum_bytes)?.as_deref() != expected.as_deref() {
                return Err(IoError::ContentMismatch {
                    entry: name.to_owned(),
                }
                .into());
            }
        }
        Ok(())
    }

    pub fn apply_using<T, E: From<IoError>>(
        &self,
        plan: RecoveryPlan<T>,
        io: &mut impl Recovery<T, E>,
    ) -> Result<Option<T>, E> {
        let bytes = if let Some(bytes) = plan.recovery {
            io.promote(self, self.recovery, None, &bytes)?;
            bytes
        } else if let Some(bytes) = plan.redo {
            io.promote(self, self.redo, plan.canonical.as_deref(), &bytes)?;
            bytes
        } else if let Some(bytes) = plan.canonical {
            io.stabilize(self, &bytes)?;
            bytes
        } else {
            return Ok(None);
        };
        if io.read(self.canonical, self.maximum_bytes)?.as_deref() != Some(bytes.as_slice()) {
            return Err(IoError::ContentMismatch {
                entry: self.canonical.to_owned(),
            }
            .into());
        }
        io.absent(self.redo)?;
        io.absent(self.recovery)?;
        Ok(plan.record)
    }
}

struct LegacyRecovery<'a, H, D, S> {
    store: &'a RetainedDurableDirectoryV1,
    hooks: &'a mut H,
    decode: D,
    successor: S,
}
impl<T, E, H, D, S> Recovery<T, E> for LegacyRecovery<'_, H, D, S>
where
    E: From<IoError>,
    H: RetainedDurableDirectoryHooksV1,
    D: Fn(&[u8]) -> Result<T, E>,
    S: Fn(Option<&T>, &T) -> Result<(), E>,
{
    fn read(&mut self, name: &'static str, maximum: usize) -> Result<Option<Vec<u8>>, E> {
        Ok(self.store.read_private(name, maximum)?)
    }
    fn absent(&mut self, name: &'static str) -> Result<(), E> {
        Ok(self.store.require_absent(name)?)
    }
    fn decode(&mut self, bytes: &[u8]) -> Result<T, E> {
        (self.decode)(bytes)
    }
    fn successor(&mut self, prior: Option<&T>, next: &T) -> Result<(), E> {
        (self.successor)(prior, next)
    }
    fn promote(
        &mut self,
        names: &JournalNames,
        staged: &'static str,
        prior: Option<&[u8]>,
        bytes: &[u8],
    ) -> Result<(), E> {
        Ok(self.store.promote_validated_redo(
            names.canonical,
            staged,
            prior,
            bytes,
            names.maximum_bytes,
            self.hooks,
        )?)
    }
    fn stabilize(&mut self, names: &JournalNames, bytes: &[u8]) -> Result<(), E> {
        self.store.establish_recovered_record_durability(
            names.canonical,
            names.recovery,
            bytes,
            names.maximum_bytes,
            self.hooks,
        )?;
        Ok(())
    }
}
