//! Native policy/session custody for the conditional production migration.
//! The installed driver is switched only with its parent and artifact consumers.
use super::InheritedExecutionSlots;
use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV3 as Subject;
use fe2o3_compiler_closure_capability::{
    COMPILER_EXECUTION_POLICY_CHILD_FD_V1 as POLICY_FD,
    CompilerExecutionCapabilityErrorV2 as PolicyError,
    CompilerExecutionPolicyCapabilityV3 as Policy,
};
use fe2o3_compiler_execution_client::{
    CompilerExecutionClientErrorV3 as ClientError, CompilerExecutionClientV3 as Client,
};
use fe2o3_compiler_execution_protocol::CompilerExecutionReceiptCarriageV3 as Carriage;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{fmt, io};

/// The same exclusive account borrow covers preparation through receipt transport.
/// The public policy is trust configuration; independent parent pinning is required.
pub(crate) struct Admitted<'b, 'w> {
    policy: Policy,
    client: Client<'b, 'w>,
}

impl<'b, 'w> Admitted<'b, 'w> {
    pub(crate) const INPUT_STORAGE: usize = Policy::FILE_STORAGE + Client::PEER_STORAGE;

    /// Consumes the loader's private copies, never its caller-owned raw slots.
    /// Both inputs must already be charged to this original account. Failure
    /// closes the owned copies but keeps all terminal charges on that account.
    pub(super) fn from_owned(
        inputs: super::OwnedExecutionInputs,
        b: &'b mut Budget<'w>,
    ) -> Result<Self, Error> {
        if b.storage() < Self::INPUT_STORAGE {
            return Err(Resource::Accounting.into());
        }
        b.charge_work(2)?;
        let (policy, charge) = Policy::from_file(inputs.policy.into(), b)?;
        b.reserve_storage(charge.additional_storage())?;
        policy.revalidate(b)?;
        let client = Client::admit(inputs.service, super::RECEIPT_ACQUISITION_TIMEOUT_V1, b)?;
        Ok(Self { policy, client })
    }

    /// Consumes both fixed inherited slots, even on resource/admission refusal.
    /// Inputs must be prepaid. Successful policy admission replaces its file
    /// charge with full capability storage; that charge remains caller-owned.
    /// The native client owns and retires its own peer reservation.
    ///
    /// # Safety
    /// Transfer unique ownership of FDs 202 and 195, inherited without Rust owners
    /// or explicitly relinquished for this transfer. There must be no existing Rust
    /// owner or outstanding borrow. No thread, handler, or foreign code may close,
    /// replace, or acquire either slot until this call returns; absent slots must
    /// remain unallocated. This consumes the transfer once, including quota
    /// refusal or unwind. Presence and flag checks cannot establish ownership.
    pub(crate) unsafe fn admit(b: &'b mut Budget<'w>) -> Result<Self, Error> {
        // SAFETY: the caller transfers both slots, including on unpaid refusal.
        let mut slots = unsafe { InheritedExecutionSlots::new() };
        if b.storage() < Self::INPUT_STORAGE {
            return Err(Resource::Accounting.into());
        }
        b.charge_work(2)?;
        slots.validate()?;
        let (policy, charge) = Policy::from_inherited_at(POLICY_FD, b)?;
        b.reserve_storage(charge.additional_storage())?;
        slots.close_policy()?;
        b.release_storage(Policy::FILE_STORAGE)?;
        policy.revalidate(b)?;
        // Client admission consumes the fixed slot on every exit, including
        // failure before inspection. Disarm our guard before handing it over.
        slots.service = None;
        // SAFETY: startup custody remains exclusive, both slots were checked
        // before policy duplication, and the sole service closer is disarmed above.
        let client =
            unsafe { Client::admit_inherited_child(super::RECEIPT_ACQUISITION_TIMEOUT_V1, b) }?;
        Ok(Self { policy, client })
    }

    pub(crate) fn prepare_and_acquire<P, R, T, E>(
        self,
        prepare: impl FnOnce(&mut Budget<'w>) -> Result<P, E>,
        publish: impl FnOnce(P, &mut Budget<'w>) -> Result<(Subject, R), E>,
        finish: impl FnOnce(Carriage, R, &mut Budget<'w>) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<Error> + From<ClientError>,
    {
        let Self { policy, client } = self;
        client.prepare_and_acquire(
            policy.policy(),
            |b| {
                policy.revalidate(b).map_err(Error::from)?;
                prepare(b)
            },
            |prepared, b| {
                policy.revalidate(b).map_err(Error::from)?;
                publish(prepared, b)
            },
            |carriage, retained, b| {
                policy.revalidate(b).map_err(Error::from)?;
                finish(carriage, retained, b)
            },
        )
    }
}

#[derive(Debug)]
pub(crate) enum Error {
    Resource(Resource),
    Policy(PolicyError),
    Client(ClientError),
    Descriptor(io::Error),
    Startup(super::ProtectedCompilerExecutionErrorV1),
}
macro_rules! causes {
    ($($ty:ty => $variant:ident),+ $(,)?) => {
        $(impl From<$ty> for Error { fn from(e: $ty) -> Self { Self::$variant(e) } })+
        impl fmt::Display for Error { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self { $(Self::$variant(e) => e.fmt(f),)+ }
        } }
        impl std::error::Error for Error { fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            match self { $(Self::$variant(e) => Some(e),)+ }
        } }
    };
}
causes!(Resource=>Resource, PolicyError=>Policy, ClientError=>Client, io::Error=>Descriptor,
    super::ProtectedCompilerExecutionErrorV1=>Startup);

#[cfg(test)]
#[path = "protected_compiler_execution_native_v3_tests.rs"]
mod tests;
