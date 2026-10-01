//! Opt-in byte/work accounting for graph presentation hashing.
//! No whole printer, Context, constructor, or allocator bound is claimed.
//! The selected session owns this non-Clone account until it is dropped.
use sha2::{Digest as _, Sha256};
use std::fmt;
use std::panic::{AssertUnwindSafe, catch_unwind};

use super::OPERATION_GRAPH_DIGEST_DOMAIN_V1 as DOMAIN;
use crate::OperationHandleError as Error;

pub(crate) struct SnapshotPolicyV1 {
    max_bytes: usize,
    max_work: usize,
    work: usize,
    completed: usize,
    failure: Option<Error>,
}

impl SnapshotPolicyV1 {
    // Reuse existing ceilings, not a new permissive global resource budget.
    // Zero is a valid fail-closed selection.
    #[allow(dead_code)] // Selected by the private bounded route; no public policy yet.
    pub(crate) fn new(max_bytes: usize, max_work: usize) -> Option<Self> {
        let hard_work = crate::production_analysis::ProductionAnalysisResourceLimitsV1
            ::production_hard_ceiling().max_work();
        if max_bytes > crate::HARD_MAX_OPERATION_IMPORT_BYTES || max_work > hard_work {
            return None;
        }
        Some(Self {
            max_bytes,
            max_work,
            work: 0,
            completed: 0,
            failure: None,
        })
    }

    fn refuse(&mut self, error: Error) -> Error {
        *self.failure.get_or_insert(error)
    }

    fn charge(&mut self, units: usize) -> Result<(), Error> {
        if let Some(error) = self.failure {
            return Err(error);
        }
        let Some(next) = self.work.checked_add(units).filter(|n| *n <= self.max_work) else {
            return Err(self.refuse(Error::OperationGraphSnapshotResourceLimit {
                resource: "presentation hash work",
            }));
        };
        self.work = next;
        Ok(())
    }

    fn pass<F>(
        &mut self,
        render: &mut F,
        primary: Sha256,
        raw: Option<Sha256>,
    ) -> Result<(usize, Sha256, Option<Sha256>), Error>
    where
        F: FnMut(&mut dyn fmt::Write) -> fmt::Result,
    {
        let mut sink = Sink {
            policy: self,
            bytes: 0,
            primary,
            raw,
        };
        let rendered = catch_unwind(AssertUnwindSafe(|| render(&mut sink)));
        let Sink {
            policy,
            bytes,
            primary,
            raw,
        } = sink;
        // A formatter cannot swallow an admitted sink refusal and return success.
        if let Some(error) = policy.failure {
            return Err(error);
        }
        match rendered {
            Err(_) => return Err(policy.refuse(Error::UpstreamPanicked)),
            Ok(Err(_)) => return Err(policy.refuse(Error::OperationGraphPresentationRejected)),
            Ok(Ok(())) => {}
        }
        Ok((bytes, primary, raw))
    }

    /// Hashes exactly DOMAIN || u64_le(UTF8 length) || UTF8 bytes.
    /// Both passes use the same owner-held graph. The raw digest and exact
    /// length must agree before the framed digest may escape.
    ///
    /// Work is admitted before each sink/hash operation, not after it. It
    /// accounts UTF8 bytes absorbed by SHA256, sink calls, and fixed framing/
    /// finalization operations. It does NOT meter traversal or allocation
    /// performed by a Display implementation before it reaches the sink.
    pub(crate) fn digest(
        &mut self,
        mut render: impl FnMut(&mut dyn fmt::Write) -> fmt::Result,
    ) -> Result<[u8; 32], Error> {
        self.charge(1)?;
        let (bytes, first, _) = self.pass(&mut render, Sha256::new(), None)?;
        self.charge(1)?;
        let first: [u8; 32] = first.finalize().into();
        let length = u64::try_from(bytes).map_err(|_| {
            self.refuse(Error::OperationGraphSnapshotResourceLimit {
                resource: "presentation byte length",
            })
        })?;
        let mut framed = Sha256::new();
        self.charge(1 + DOMAIN.len())?;
        framed.update(DOMAIN);
        self.charge(1 + std::mem::size_of::<u64>())?;
        framed.update(length.to_le_bytes());
        let (second_bytes, framed, raw) = self.pass(&mut render, framed, Some(Sha256::new()))?;
        self.charge(1)?;
        let second: [u8; 32] = raw
            .expect("second pass retains its raw hash")
            .finalize()
            .into();
        if bytes != second_bytes || first != second {
            return Err(self.refuse(Error::OperationGraphPresentationChanged));
        }
        self.charge(1)?;
        let Some(completed) = self.completed.checked_add(1) else {
            return Err(self.refuse(Error::OperationGraphSnapshotResourceLimit {
                resource: "presentation count",
            }));
        };
        self.completed = completed;
        Ok(framed.finalize().into())
    }

    #[cfg(test)]
    pub(crate) fn observation(&self) -> (usize, usize, Option<Error>) {
        (self.work, self.completed, self.failure)
    }
}

struct Sink<'a> {
    policy: &'a mut SnapshotPolicyV1,
    bytes: usize,
    primary: Sha256,
    raw: Option<Sha256>,
}

impl fmt::Write for Sink<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        if self.policy.failure.is_some() {
            return Err(fmt::Error);
        }
        let Some(next) = self
            .bytes
            .checked_add(text.len())
            .filter(|n| *n <= self.policy.max_bytes)
        else {
            self.policy
                .refuse(Error::OperationGraphSnapshotResourceLimit {
                    resource: "presentation UTF8 bytes",
                });
            return Err(fmt::Error);
        };
        let copies = if self.raw.is_some() { 2 } else { 1 };
        let Some(work) = text
            .len()
            .checked_add(1)
            .and_then(|n| n.checked_mul(copies))
        else {
            self.policy
                .refuse(Error::OperationGraphSnapshotResourceLimit {
                    resource: "presentation hash work",
                });
            return Err(fmt::Error);
        };
        self.policy.charge(work).map_err(|_| fmt::Error)?;
        self.primary.update(text.as_bytes());
        if let Some(raw) = &mut self.raw {
            raw.update(text.as_bytes());
        }
        self.bytes = next;
        Ok(())
    }
}
