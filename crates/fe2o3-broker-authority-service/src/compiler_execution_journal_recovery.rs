//! One namespace state machine for issuer, Worker, and anchor journals.
//!
//! A redo proposes a successor; a recovery file retains an already canonical
//! record. Distinct names preserve that distinction if recovery itself crashes.
use fe2o3_artifact_transaction::{
    MeteredRetainedDurableDirectoryV2 as MeteredDirectory,
    RetainedDurableDirectoryErrorV1 as IoError, RetainedDurableDirectoryErrorV2 as NativeIoError,
    RetainedDurableDirectoryHooksV1, RetainedDurableDirectoryV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
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
        self.recover_using(&mut LegacyRecovery {
            store,
            hooks,
            decode,
            successor,
        })
    }

    /// Uses the same crash-state relation with native I/O and nominal decoders.
    /// All decoded owners and read buffers coexist in a prepaid outer frame;
    /// the caller reserves the returned record's charge before retaining it.
    pub fn recover_native<T, E>(
        &self,
        store: &RetainedDurableDirectoryV1,
        hooks: &mut impl RetainedDurableDirectoryHooksV1,
        budget: &mut Budget<'_>,
        decode: impl FnMut(&[u8], &mut Budget<'_>) -> Result<T, E>,
        successor: impl Fn(Option<&T>, &T) -> Result<(), E>,
        decoded_storage: usize,
    ) -> Result<Option<T>, E>
    where
        E: From<IoError> + From<NativeIoError> + From<Resource>,
    {
        let scratch = self
            .maximum_bytes
            .checked_mul(6)
            .and_then(|v| {
                decoded_storage
                    .checked_mul(4)
                    .and_then(|records| v.checked_add(records))
            })
            .and_then(|v| v.checked_add(4096))
            .ok_or(Resource::Arithmetic)?;
        budget.with_prepaid_scope(
            MeteredDirectory::DIRECTORY_STORAGE,
            8,
            8,
            scratch,
            |budget| {
                self.recover_using(&mut NativeRecovery {
                    store,
                    hooks,
                    budget,
                    decode,
                    successor,
                })
            },
        )
    }

    fn recover_using<T, E: From<IoError>>(
        &self,
        context: &mut impl Recovery<T, E>,
    ) -> Result<Option<T>, E> {
        let canonical = context.read(self.canonical, self.maximum_bytes)?;
        let redo = context.read(self.redo, self.maximum_bytes)?;
        let recovery = context.read(self.recovery, self.maximum_bytes)?;
        let (record, bytes) = if let Some(bytes) = recovery {
            // A recovery rename never coexists with either transaction name.
            // Even byte-identical duplicates are not a reachable crash position.
            if canonical.is_some() || redo.is_some() {
                return Err(IoError::ExistingEntry {
                    entry: self.recovery.to_owned(),
                }
                .into());
            }
            let record = context.decode(&bytes)?;
            context.promote(
                self.canonical,
                self.recovery,
                None,
                &bytes,
                self.maximum_bytes,
            )?;
            (record, bytes)
        } else {
            match (canonical, redo) {
                (None, None) => return Ok(None),
                (canonical, Some(bytes)) => {
                    let record = context.decode(&bytes)?;
                    let prior = canonical
                        .as_deref()
                        .map(|bytes| context.decode(bytes))
                        .transpose()?;
                    context.successor(prior.as_ref(), &record)?;
                    context.promote(
                        self.canonical,
                        self.redo,
                        canonical.as_deref(),
                        &bytes,
                        self.maximum_bytes,
                    )?;
                    (record, bytes)
                }
                (Some(bytes), None) => {
                    let record = context.decode(&bytes)?;
                    context.stabilize(self.canonical, self.recovery, &bytes, self.maximum_bytes)?;
                    (record, bytes)
                }
            }
        };
        if context.read(self.canonical, self.maximum_bytes)?.as_deref() != Some(bytes.as_slice()) {
            return Err(IoError::ContentMismatch {
                entry: self.canonical.to_owned(),
            }
            .into());
        }
        context.absent(self.redo)?;
        context.absent(self.recovery)?;
        Ok(Some(record))
    }
}

trait Recovery<T, E> {
    fn read(&mut self, name: &str, maximum: usize) -> Result<Option<Vec<u8>>, E>;
    fn decode(&mut self, bytes: &[u8]) -> Result<T, E>;
    fn successor(&self, prior: Option<&T>, next: &T) -> Result<(), E>;
    fn promote(
        &mut self,
        canonical: &str,
        redo: &str,
        prior: Option<&[u8]>,
        next: &[u8],
        maximum: usize,
    ) -> Result<(), E>;
    fn stabilize(
        &mut self,
        canonical: &str,
        recovery: &str,
        bytes: &[u8],
        maximum: usize,
    ) -> Result<(), E>;
    fn absent(&mut self, name: &str) -> Result<(), E>;
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
    fn read(&mut self, name: &str, maximum: usize) -> Result<Option<Vec<u8>>, E> {
        Ok(self.store.read_private(name, maximum)?)
    }
    fn decode(&mut self, bytes: &[u8]) -> Result<T, E> {
        (self.decode)(bytes)
    }
    fn successor(&self, prior: Option<&T>, next: &T) -> Result<(), E> {
        (self.successor)(prior, next)
    }
    fn promote(
        &mut self,
        canonical: &str,
        redo: &str,
        prior: Option<&[u8]>,
        next: &[u8],
        maximum: usize,
    ) -> Result<(), E> {
        Ok(self
            .store
            .promote_validated_redo(canonical, redo, prior, next, maximum, self.hooks)?)
    }
    fn stabilize(
        &mut self,
        canonical: &str,
        recovery: &str,
        bytes: &[u8],
        maximum: usize,
    ) -> Result<(), E> {
        self.store.establish_recovered_record_durability(
            canonical, recovery, bytes, maximum, self.hooks,
        )?;
        Ok(())
    }
    fn absent(&mut self, name: &str) -> Result<(), E> {
        Ok(self.store.require_absent(name)?)
    }
}

struct NativeRecovery<'a, 'work, H, D, S> {
    store: &'a RetainedDurableDirectoryV1,
    hooks: &'a mut H,
    budget: &'a mut Budget<'work>,
    decode: D,
    successor: S,
}
impl<T, E, H, D, S> Recovery<T, E> for NativeRecovery<'_, '_, H, D, S>
where
    E: From<NativeIoError>,
    H: RetainedDurableDirectoryHooksV1,
    D: FnMut(&[u8], &mut Budget<'_>) -> Result<T, E>,
    S: Fn(Option<&T>, &T) -> Result<(), E>,
{
    fn read(&mut self, name: &str, maximum: usize) -> Result<Option<Vec<u8>>, E> {
        Ok(MeteredDirectory::new(self.store, self.budget).read_private(name, maximum)?)
    }
    fn decode(&mut self, bytes: &[u8]) -> Result<T, E> {
        (self.decode)(bytes, self.budget)
    }
    fn successor(&self, prior: Option<&T>, next: &T) -> Result<(), E> {
        (self.successor)(prior, next)
    }
    fn promote(
        &mut self,
        canonical: &str,
        redo: &str,
        prior: Option<&[u8]>,
        next: &[u8],
        maximum: usize,
    ) -> Result<(), E> {
        Ok(MeteredDirectory::new(self.store, self.budget)
            .promote_validated_redo(canonical, redo, prior, next, maximum, self.hooks)?)
    }
    fn stabilize(
        &mut self,
        canonical: &str,
        recovery: &str,
        bytes: &[u8],
        maximum: usize,
    ) -> Result<(), E> {
        MeteredDirectory::new(self.store, self.budget).establish_recovered_record_durability(
            canonical, recovery, bytes, maximum, self.hooks,
        )?;
        Ok(())
    }
    fn absent(&mut self, name: &str) -> Result<(), E> {
        Ok(MeteredDirectory::new(self.store, self.budget).require_absent(name)?)
    }
}
