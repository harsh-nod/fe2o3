//! Private ordering of inert records, not issuer admission, authority,
//! retirement evidence, or durable recovery. A cached reply proves none of these.
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ROOT_CONTROL_BYTES_V3 as RECORD_BYTES,
    COMPILER_EXECUTION_ROOT_CONTROL_STORAGE_V3 as PROTOCOL_FRAME,
    COMPILER_EXECUTION_ROOT_CONTROL_WORK_V3 as PROTOCOL_WORK,
    CompilerExecutionAttestationStorageV3 as ProtocolStorage,
    CompilerExecutionRootControlBindingV3 as Binding,
    CompilerExecutionRootControlErrorV3 as ProtocolError,
    CompilerExecutionRootControlRecordV3 as Record,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use rustix::process;
use std::{fmt, marker::PhantomData, mem::size_of, rc::Rc};

const ENTRY: usize = 8;
// Bounded comparisons, moves and owner checks; nested protocol calls additionally
// charge their own work and scratch on the same budget.
const WORK: usize = ENTRY + 8 * RECORD_BYTES;
const FRAME: usize = 4 * size_of::<RootControlReplayWindowV3<'static>>() + 4096;

/// Full unreserved maximum, including binding, two records and owner overhead.
/// This is never a state-dependent delta, including for an empty window.
#[derive(Debug)]
pub(crate) struct RootControlReplayStorageV3(usize);
impl RootControlReplayStorageV3 {
    pub(crate) const fn additional_storage(&self) -> usize {
        self.0
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum RootControlRequestDispositionV3 {
    /// Newly retained inert request; conveys no permission to execute it.
    Accepted,
    /// Exact pending duplicate: the caller MUST NOT rerun the request.
    Pending,
    /// Exact completed duplicate: use the retained reply.
    Replay,
}
use RootControlRequestDispositionV3 as Disposition;

enum State {
    Empty,
    Pending(Record),
    Replied { request: Record, reply: Record },
}

/// Move-only, !Send/!Sync, single-exchange replay window. Keep the original Work
/// borrow live and Budget at its original address on the creator PID/kernel TID.
/// Prepay STORAGE throughout this owner's lifetime, even when empty. Consumed
/// inputs stay prepaid during calls; release their separate charges afterward.
/// All calls restore entry storage without refunding work or denial history.
///
/// Completion caches both records before sending. Send failure/retry must leave
/// this owner intact; only an accepted next sequence replaces the cached pair.
/// This per-connection cache is NOT RootSession's cross-connection retirement
/// tombstone. Original occurrence custody and the durable exact retirement
/// tombstone must be retained independently and survive connection replacement.
/// There is no reset, retirement operation, authority conversion or recovery API.
pub(crate) struct RootControlReplayWindowV3<'work> {
    binding: Binding,
    state: State,
    account: Ledger,
    budget_address: usize,
    process: process::Pid,
    creator: process::Pid,
    _work: PhantomData<(&'work Work, Rc<()>)>,
}

impl<'work> RootControlReplayWindowV3<'work> {
    /// Conservative per-call totals, including one nested protocol comparison.
    /// Constructor and accessors use only the smaller local scope.
    pub(crate) const WORK: usize = WORK + PROTOCOL_WORK;
    pub(crate) const SCRATCH: usize = FRAME + PROTOCOL_FRAME;
    /// Fixed full maximum, deliberately conservative about inline owner bytes.
    pub(crate) const STORAGE: usize = size_of::<(Self, RootControlReplayStorageV3)>()
        + size_of::<(Binding, ProtocolStorage)>()
        + 2 * size_of::<(Record, ProtocolStorage)>();

    pub(crate) fn new(
        binding: Binding,
        b: &mut Budget<'work>,
    ) -> Result<(Self, RootControlReplayStorageV3)> {
        b.with_prepaid_scope(binding.retained_storage(), ENTRY, WORK, FRAME, |b| {
            Ok((
                Self {
                    binding,
                    state: State::Empty,
                    account: b.work_ledger_identity_v1(),
                    budget_address: b as *const Budget<'_> as usize,
                    process: process::getpid(),
                    creator: rustix::thread::gettid(),
                    _work: PhantomData,
                },
                RootControlReplayStorageV3(Self::STORAGE),
            ))
        })
    }

    pub(crate) fn accept(&mut self, request: Record, b: &mut Budget<'_>) -> Result<Disposition> {
        let floor = Self::STORAGE + request.retained_storage();
        b.with_prepaid_scope(floor, ENTRY, WORK, FRAME, |b| {
            self.check_account(b)?;
            if request.is_reply() || !request.matches_binding(&self.binding, b)? {
                return Err(Error::Refused("root control request binding or direction"));
            }
            let next = match &self.state {
                State::Empty => 1,
                State::Pending(pending) => {
                    return if request.canonical_bytes() == pending.canonical_bytes() {
                        Ok(Disposition::Pending)
                    } else {
                        Err(Error::Refused("root control request already pending"))
                    };
                }
                State::Replied { request: prior, .. } => {
                    if request.canonical_bytes() == prior.canonical_bytes() {
                        return Ok(Disposition::Replay);
                    }
                    prior
                        .sequence()
                        .checked_add(1)
                        .ok_or(Error::Refused("root control sequence exhausted"))?
                }
            };
            if request.sequence() != next {
                return Err(Error::Refused("root control request sequence"));
            }
            self.state = State::Pending(request);
            Ok(Disposition::Accepted)
        })
    }

    pub(crate) fn complete(&mut self, reply: Record, b: &mut Budget<'_>) -> Result<()> {
        let floor = Self::STORAGE + reply.retained_storage();
        b.with_prepaid_scope(floor, ENTRY, WORK, FRAME, |b| {
            self.check_account(b)?;
            let State::Pending(request) = &self.state else {
                return Err(Error::Refused("no pending root control request"));
            };
            if !reply.matches_reply(request, b)? {
                return Err(Error::Refused("root control reply association"));
            }
            // All fallible validation and funding precede this ownership move.
            let State::Pending(request) = std::mem::replace(&mut self.state, State::Empty) else {
                unreachable!("pending state checked above");
            };
            self.state = State::Replied { request, reply };
            Ok(())
        })
    }

    pub(crate) fn pending(&self, b: &mut Budget<'_>) -> Result<Option<&Record>> {
        b.with_prepaid_scope(Self::STORAGE, ENTRY, WORK, FRAME, |b| {
            self.check_account(b)?;
            Ok(match &self.state {
                State::Pending(request) => Some(request),
                _ => None,
            })
        })
    }

    pub(crate) fn request(&self, b: &mut Budget<'_>) -> Result<Option<&Record>> {
        b.with_prepaid_scope(Self::STORAGE, ENTRY, WORK, FRAME, |b| {
            self.check_account(b)?;
            Ok(match &self.state {
                State::Empty => None,
                State::Pending(request) | State::Replied { request, .. } => Some(request),
            })
        })
    }

    pub(crate) fn reply(&self, b: &mut Budget<'_>) -> Result<Option<&Record>> {
        b.with_prepaid_scope(Self::STORAGE, ENTRY, WORK, FRAME, |b| {
            self.check_account(b)?;
            Ok(match &self.state {
                State::Replied { reply, .. } => Some(reply),
                _ => None,
            })
        })
    }

    pub(crate) const fn retained_storage(&self) -> usize {
        Self::STORAGE
    }

    fn check_account(&self, b: &Budget<'_>) -> Result<()> {
        if self.account != b.work_ledger_identity_v1()
            || self.budget_address != b as *const Budget<'_> as usize
            || self.process != process::getpid()
            || self.creator != rustix::thread::gettid()
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) enum RootControlReplayErrorV3 {
    Resource(Resource),
    Protocol(ProtocolError),
    Refused(&'static str),
}
use RootControlReplayErrorV3 as Error;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(e: Resource) -> Self {
        Self::Resource(e)
    }
}
impl From<ProtocolError> for Error {
    fn from(e: ProtocolError) -> Self {
        match e {
            ProtocolError::Resource(e) => Self::Resource(e),
            e => Self::Protocol(e),
        }
    }
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Protocol(e) => e.fmt(f),
            Self::Refused(s) => f.write_str(s),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Protocol(e) => Some(e),
            Self::Refused(_) => None,
        }
    }
}

#[cfg(test)]
#[path = "compiler_execution_root_exchange_tests.rs"]
mod tests;
