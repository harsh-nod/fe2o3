//! One namespace state machine for issuer, Worker, and anchor journals.
//!
//! A redo proposes a successor; a recovery file retains an already canonical
//! record. Distinct names preserve that distinction if recovery itself crashes.
use fe2o3_artifact_transaction::{
    RetainedDurableDirectoryErrorV1 as IoError, RetainedDurableDirectoryHooksV1,
    RetainedDurableDirectoryV1,
};

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

/// The issuer holds the directory singleton lock before this check. Standalone
/// Worker callers must also serialize writers. Never infer absence from decoding.
pub(super) fn reject_legacy_state(store: &RetainedDurableDirectoryV1) -> Result<(), IoError> {
    for name in LEGACY_STATE_FILES {
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
        let canonical = store.read_private(self.canonical, self.maximum_bytes)?;
        let redo = store.read_private(self.redo, self.maximum_bytes)?;
        let recovery = store.read_private(self.recovery, self.maximum_bytes)?;
        let (record, bytes) = if let Some(bytes) = recovery {
            // A recovery rename never coexists with either transaction name.
            // Even byte-identical duplicates are not a reachable crash position.
            if canonical.is_some() || redo.is_some() {
                return Err(IoError::ExistingEntry {
                    entry: self.recovery.to_owned(),
                }
                .into());
            }
            let record = decode(&bytes)?;
            store.promote_validated_redo(
                self.canonical,
                self.recovery,
                None,
                &bytes,
                self.maximum_bytes,
                hooks,
            )?;
            (record, bytes)
        } else {
            match (canonical, redo) {
                (None, None) => return Ok(None),
                (canonical, Some(bytes)) => {
                    let record = decode(&bytes)?;
                    let prior = canonical.as_deref().map(&decode).transpose()?;
                    successor(prior.as_ref(), &record)?;
                    store.promote_validated_redo(
                        self.canonical,
                        self.redo,
                        canonical.as_deref(),
                        &bytes,
                        self.maximum_bytes,
                        hooks,
                    )?;
                    (record, bytes)
                }
                (Some(bytes), None) => {
                    let record = decode(&bytes)?;
                    store.establish_recovered_record_durability(
                        self.canonical,
                        self.recovery,
                        &bytes,
                        self.maximum_bytes,
                        hooks,
                    )?;
                    (record, bytes)
                }
            }
        };
        if store
            .read_private(self.canonical, self.maximum_bytes)?
            .as_deref()
            != Some(bytes.as_slice())
        {
            return Err(IoError::ContentMismatch {
                entry: self.canonical.to_owned(),
            }
            .into());
        }
        store.require_absent(self.redo)?;
        store.require_absent(self.recovery)?;
        Ok(Some(record))
    }
}
