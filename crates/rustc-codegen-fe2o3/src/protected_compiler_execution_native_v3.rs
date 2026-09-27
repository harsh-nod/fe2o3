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

    /// Consumes both fixed inherited slots, even on resource/admission refusal.
    /// Inputs must be prepaid. Successful policy admission replaces its file
    /// charge with full capability storage; that charge remains caller-owned.
    /// The native client owns and retires its own peer reservation.
    pub(crate) fn admit(b: &'b mut Budget<'w>) -> Result<Self, Error> {
        let mut slots = InheritedExecutionSlots::new();
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
        let client = Client::admit_inherited_child(super::RECEIPT_ACQUISITION_TIMEOUT_V1, b)?;
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
causes!(Resource=>Resource, PolicyError=>Policy, ClientError=>Client, io::Error=>Descriptor);

#[cfg(test)]
#[path = "protected_compiler_execution_native_v3_tests.rs"]
mod tests;
