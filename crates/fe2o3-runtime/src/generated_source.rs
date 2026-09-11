//! Descriptive generated custody. No source view or reservation authorizes issue.

use fe2o3_kfd::GFX942_MAX_FIXED_DISPATCH_DATA_V1;
use fe2o3_resource_accounting::ResourceCreditErrorV1;
use sha2::{Digest, Sha256};
use std::{error::Error, fmt};

use crate::{
    GeneratedGfx942PersistentStorageV1, Gfx942RuntimeBufferAccessV1, KfdRuntimeBackendErrorV1,
    PreparedGfx942PersistentDispatchV1, RuntimeAsyncEngineCallErrorV1, RuntimeErrorV1,
    WorkerV3Gfx942ExecutionAuthorityV1,
};

/// A simultaneous borrow of the original projection, artifact and existing authority.
/// This is descriptive data, not native admission or permission to publish.
#[doc(hidden)]
pub struct RuntimeGfx942GeneratedSourceV1<'a, E> {
    projection: &'a crate::persistent_projection::PersistentDispatchDataV1,
    hsaco: &'a [u8],
    authority: &'a dyn WorkerV3Gfx942ExecutionAuthorityV1<CurrentnessError = E>,
}

impl<'a, E> RuntimeGfx942GeneratedSourceV1<'a, E> {
    pub fn new(
        projection: &'a PreparedGfx942PersistentDispatchV1,
        hsaco: &'a [u8],
        authority: &'a dyn WorkerV3Gfx942ExecutionAuthorityV1<CurrentnessError = E>,
    ) -> Self {
        Self {
            projection: projection.data(),
            hsaco,
            authority,
        }
    }

    pub fn from_generated_storage(
        storage: &'a GeneratedGfx942PersistentStorageV1,
        hsaco: &'a [u8],
        authority: &'a dyn WorkerV3Gfx942ExecutionAuthorityV1<CurrentnessError = E>,
    ) -> Self {
        Self {
            projection: storage.data(),
            hsaco,
            authority,
        }
    }

    pub(crate) fn validate(
        &self,
        device_unique_id: u64,
    ) -> Result<GeneratedHostRosterV1, RuntimeGfx942GeneratedReservationErrorV1> {
        use RuntimeGfx942GeneratedReservationErrorV1 as Error;
        let projection = self.projection;
        if u64::try_from(self.hsaco.len()).ok() != Some(projection.finalized_hsaco_length())
            || <[u8; 32]>::from(Sha256::digest(self.hsaco)) != projection.identity().object_sha256()
        {
            return Err(Error::ArtifactMismatch);
        }
        crate::authorized_execution::validate_authority_bindings_v1(
            self.authority,
            projection.identity().object_sha256(),
            projection.finalized_hsaco_length(),
            projection.kernel_name(),
            projection.dispatch_contract_sha256(),
            device_unique_id,
        )
        .map_err(|_| Error::AuthorityMismatch)?;
        self.revalidate()?;
        GeneratedHostRosterV1::from_data(projection)
    }

    pub(crate) fn revalidate(&self) -> Result<(), RuntimeGfx942GeneratedReservationErrorV1> {
        self.authority
            .revalidate_currentness()
            .map_err(|_| RuntimeGfx942GeneratedReservationErrorV1::AuthorityNotCurrent)
    }
}

/// Closed simultaneous mutable storage and immutable authority borrow.
/// No public method extracts control or changes the source representation.
///
/// ```compile_fail,E0599
/// use fe2o3_runtime::RuntimeGfx942GeneratedSourceMutV1;
/// fn clone_view(value: RuntimeGfx942GeneratedSourceMutV1<'_, ()>) { let _ = value.clone(); }
/// ```
/// ```compile_fail,E0624
/// use fe2o3_runtime::RuntimeGfx942GeneratedSourceMutV1;
/// fn extract(mut value: RuntimeGfx942GeneratedSourceMutV1<'_, ()>) {
///     value.transfer_control_into(&mut None);
/// }
/// ```
/// ```compile_fail,E0515
/// use fe2o3_runtime::{GeneratedGfx942PersistentStorageV1, RuntimeGfx942GeneratedSourceMutV1,
///                     WorkerV3Gfx942ExecutionAuthorityV1};
/// fn escape<'a>(mut storage: GeneratedGfx942PersistentStorageV1, hsaco: &'a [u8],
///     authority: &'a dyn WorkerV3Gfx942ExecutionAuthorityV1<CurrentnessError = ()>)
///     -> RuntimeGfx942GeneratedSourceMutV1<'a, ()> {
///     RuntimeGfx942GeneratedSourceMutV1::new(&mut storage, hsaco, authority)
/// }
/// ```
#[doc(hidden)]
pub struct RuntimeGfx942GeneratedSourceMutV1<'a, E> {
    storage: &'a mut GeneratedGfx942PersistentStorageV1,
    hsaco: &'a [u8],
    authority: &'a dyn WorkerV3Gfx942ExecutionAuthorityV1<CurrentnessError = E>,
}

impl<'a, E> RuntimeGfx942GeneratedSourceMutV1<'a, E> {
    pub fn new(
        storage: &'a mut GeneratedGfx942PersistentStorageV1,
        hsaco: &'a [u8],
        authority: &'a dyn WorkerV3Gfx942ExecutionAuthorityV1<CurrentnessError = E>,
    ) -> Self {
        Self {
            storage,
            hsaco,
            authority,
        }
    }

    pub(crate) fn validate(
        &self,
        device_unique_id: u64,
    ) -> Result<GeneratedHostRosterV1, RuntimeGfx942GeneratedReservationErrorV1> {
        if !self.storage.control_available() {
            return Err(RuntimeGfx942GeneratedReservationErrorV1::UnsupportedPreparation);
        }
        RuntimeGfx942GeneratedSourceV1::from_generated_storage(
            self.storage,
            self.hsaco,
            self.authority,
        )
        .validate(device_unique_id)
    }

    pub(crate) fn transfer_control_into(
        &mut self,
        destination: &mut Option<fe2o3_kfd::Gfx942FixedDispatchPacketV1>,
    ) -> bool {
        self.storage.transfer_control_into(destination)
    }

    pub(crate) fn matches_roster(&self, expected: &GeneratedHostRosterV1) -> bool {
        std::sync::Arc::ptr_eq(
            self.storage.data().source_identity(),
            &expected.source_identity,
        ) && self.storage.control_available()
            && GeneratedHostRosterV1::from_data(self.storage.data())
                .is_ok_and(|actual| actual.matches(expected))
    }
}

/// Host integration for inert generated storage, not an execution-authority trait.
/// Runtime independently joins source identity and retained Context currentness.
/// `Readback` is staged without mutating the carrier; installation occurs only
/// after the closing checks. Neither method may submit native work. A future
/// native adapter must re-establish authority at adoption and actual publication.
#[doc(hidden)]
pub trait RuntimeGfx942GeneratedCarrierV1 {
    type CurrentnessError;
    type Readback;
    fn source(&self) -> RuntimeGfx942GeneratedSourceV1<'_, Self::CurrentnessError>;
    fn source_mut(
        &mut self,
    ) -> Option<RuntimeGfx942GeneratedSourceMutV1<'_, Self::CurrentnessError>> {
        None
    }
    fn prepare_readback(&self) -> Result<Self::Readback, RuntimeGfx942ReadbackErrorV1>;
    fn install_readback(&mut self, readback: Self::Readback);
}

#[derive(Debug)]
pub enum RuntimeGfx942ReadbackErrorV1 {
    InvalidStorage,
    AlreadyReserved,
    Allocation,
    Credit(ResourceCreditErrorV1),
}

impl fmt::Display for RuntimeGfx942ReadbackErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "readback reservation: {self:?}")
    }
}
impl Error for RuntimeGfx942ReadbackErrorV1 {}

#[derive(Debug)]
pub enum RuntimeGfx942GeneratedReservationErrorV1 {
    Engine(RuntimeAsyncEngineCallErrorV1),
    UnsupportedPreparation,
    Context(RuntimeErrorV1<KfdRuntimeBackendErrorV1>),
    ArtifactMismatch,
    AuthorityMismatch,
    AuthorityNotCurrent,
    InvalidRoster,
    Readback(RuntimeGfx942ReadbackErrorV1),
}

impl fmt::Display for RuntimeGfx942GeneratedReservationErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "generated reservation: {self:?}")
    }
}
impl Error for RuntimeGfx942GeneratedReservationErrorV1 {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GeneratedBufferSlotV1 {
    pub ordinal: usize,
    pub bytes: u64,
    pub access: Gfx942RuntimeBufferAccessV1,
}

// Retained descriptive metadata for the next native adoption transition, not a permit.
pub(crate) struct GeneratedHostRosterV1 {
    pub source_identity: std::sync::Arc<()>,
    pub buffers: [Option<GeneratedBufferSlotV1>; GFX942_MAX_FIXED_DISPATCH_DATA_V1],
    pub count: usize,
    pub readback_bytes: u64,
    pub fixup_count: usize,
    pub dispatch_contract_sha256: [u8; 32],
}

impl GeneratedHostRosterV1 {
    pub(crate) fn matches(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.source_identity, &other.source_identity)
            && self.buffers == other.buffers
            && self.count == other.count
            && self.readback_bytes == other.readback_bytes
            && self.fixup_count == other.fixup_count
            && self.dispatch_contract_sha256 == other.dispatch_contract_sha256
    }

    #[cfg(test)]
    pub(crate) fn from_projection(
        projection: &PreparedGfx942PersistentDispatchV1,
    ) -> Result<Self, RuntimeGfx942GeneratedReservationErrorV1> {
        Self::from_data(projection.data())
    }

    fn from_data(
        projection: &crate::persistent_projection::PersistentDispatchDataV1,
    ) -> Result<Self, RuntimeGfx942GeneratedReservationErrorV1> {
        use RuntimeGfx942GeneratedReservationErrorV1 as Error;
        if projection.buffers().is_empty()
            || projection.buffers().len() > GFX942_MAX_FIXED_DISPATCH_DATA_V1
            || !projection.complete_buffer_policies_match()
        {
            return Err(Error::InvalidRoster);
        }
        let mut roster = Self {
            source_identity: std::sync::Arc::clone(projection.source_identity()),
            buffers: [None; GFX942_MAX_FIXED_DISPATCH_DATA_V1],
            count: projection.buffers().len(),
            readback_bytes: 0,
            fixup_count: projection.pointer_fixups().len(),
            dispatch_contract_sha256: projection.dispatch_contract_sha256(),
        };
        for (ordinal, buffer) in projection.buffers().iter().enumerate() {
            let bytes = u64::try_from(buffer.bytes().len()).map_err(|_| Error::InvalidRoster)?;
            let access = projection
                .buffer_access(ordinal)
                .ok_or(Error::InvalidRoster)?;
            roster.readback_bytes = roster
                .readback_bytes
                .checked_add(bytes)
                .ok_or(Error::InvalidRoster)?;
            roster.buffers[ordinal] = Some(GeneratedBufferSlotV1 {
                ordinal,
                bytes,
                access,
            });
        }
        Ok(roster)
    }
}
