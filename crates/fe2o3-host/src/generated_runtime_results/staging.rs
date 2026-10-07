//! Completed typed values become a new ordinary host-write version, not a peer producer.

use super::*;
use fe2o3_runtime::{
    RuntimeAllocationIdV1, RuntimeBackendV1, RuntimeContextV1, RuntimeErrorV1,
    RuntimeGeneratedCompletionReceiptV1,
};

#[derive(Debug)]
#[non_exhaustive]
pub enum GeneratedRuntimeStagingErrorV1<E> {
    Output(Error),
    Context(RuntimeErrorV1<E>),
}

impl<E: fmt::Display> fmt::Display for GeneratedRuntimeStagingErrorV1<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Output(error) => error.fmt(f),
            Self::Context(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for GeneratedRuntimeStagingErrorV1<E> {}

impl<T: GeneratedDeviceScalarV1> ChargedTypedResultV1<T> {
    /// Encodes data into caller-owned exact-size scratch without taking the result
    /// or its credit. Empty and mismatched lengths reject before changing scratch.
    /// This is data conversion only, not invocation or completion authority.
    /// Scratch is caller-owned and outside result-credit accounting.
    pub fn encode_into_v1(&self, scratch: &mut [u8]) -> Result<(), Error> {
        let width = size_of::<T>();
        if self.is_empty() || self.len().checked_mul(width) != Some(scratch.len()) {
            return Err(Error::ByteLength);
        }
        for (value, destination) in self.as_slice().iter().zip(scratch.chunks_exact_mut(width)) {
            let (bytes, len) = value.encode_le_bytes_v1();
            // The scalar trait is sealed to primitives with exact LE encodings.
            assert_eq!(usize::from(len), width);
            destination.copy_from_slice(&bytes[..width]);
        }
        Ok(())
    }

    /// Writes charged typed data to a complete ordinary HostVisible allocation.
    /// This data-only operation grants no invocation or completion authority.
    /// The original result, its credit and the caller's allocation remain owned
    /// by their original owners on success, rejection, uncertain failure or panic.
    ///
    /// Scratch must have exactly `len() * size_of::<T>()` bytes. It is caller-owned
    /// and outside result-credit accounting; no hidden encoding buffer is allocated.
    /// It may contain encoded values even when Context rejects the write. Only
    /// `Ok(())` establishes successful staging. Native storage/backing retains
    /// its ordinary Context accounting, independently of the result reservation.
    ///
    /// Then use an ordinary async copy to DeviceLocal. Keep staging until copy
    /// quiescence and require copy success before admitting an ordinary peer.
    pub fn write_staging_v1<B: RuntimeBackendV1>(
        &self,
        context: &mut RuntimeContextV1<B>,
        staging: RuntimeAllocationIdV1,
        scratch: &mut [u8],
    ) -> Result<(), GeneratedRuntimeStagingErrorV1<B::Error>> {
        self.encode_into_v1(scratch)
            .map_err(GeneratedRuntimeStagingErrorV1::Output)?;
        context
            .write_host_visible_allocation_v1(staging, scratch)
            .map_err(GeneratedRuntimeStagingErrorV1::Context)
    }
}

impl<T: GeneratedDeviceScalarV1> GeneratedRuntimeChargedResultV1<T> {
    /// Encodes only this original completion's committed output without consuming
    /// its observer, typed storage or charge. `None` means slot contention with no
    /// scratch effects. Scratch is caller-owned and not charged to the result budget.
    ///
    /// For owner-engine use, encode on the caller and enqueue only scratch and
    /// staging handles, keeping the original output recoverable on queue rejection.
    /// This observer path is unavailable after taking or binding the typed result.
    ///
    /// ```no_run
    /// use fe2o3_host::{GeneratedRuntimeArgumentErrorV1, GeneratedRuntimeChargedResultV1};
    /// use fe2o3_runtime::RuntimeGeneratedCompletionReceiptV1;
    ///
    /// fn encode(
    ///     output: &mut GeneratedRuntimeChargedResultV1<u32>,
    ///     completion: &RuntimeGeneratedCompletionReceiptV1,
    ///     scratch: &mut [u8],
    /// ) -> Result<Option<()>, GeneratedRuntimeArgumentErrorV1> {
    ///     output.encode_completed_into_v1(completion, scratch)
    /// }
    /// ```
    pub fn encode_completed_into_v1(
        &mut self,
        receipt: &RuntimeGeneratedCompletionReceiptV1,
        scratch: &mut [u8],
    ) -> Result<Option<()>, Error> {
        self.encode_completed_matching_v1(|gate| receipt.matches_owner(gate), scratch)
    }

    /// Stages only the output authenticated by its original completion receipt.
    /// Unlike taking the result, this preserves the original observer and charged
    /// typed storage. A repeated call is another ordinary host write, not a retry
    /// of native execution; Context still rejects retained readers/Unknown writers.
    ///
    /// `None` means transient slot contention with no scratch or Context effects.
    /// A foreign receipt, unready gate or unavailable output rejects before encoding.
    /// Scratch ownership and failure behavior match `ChargedTypedResultV1::write_staging_v1`.
    ///
    /// ```no_run
    /// use fe2o3_host::{GeneratedRuntimeChargedResultV1, GeneratedRuntimeStagingErrorV1};
    /// use fe2o3_runtime::{RuntimeAllocationIdV1, RuntimeBackendV1, RuntimeContextV1,
    ///     RuntimeGeneratedCompletionReceiptV1};
    ///
    /// fn stage<B: RuntimeBackendV1>(
    ///     output: &mut GeneratedRuntimeChargedResultV1<u32>,
    ///     completion: &RuntimeGeneratedCompletionReceiptV1,
    ///     context: &mut RuntimeContextV1<B>,
    ///     host: RuntimeAllocationIdV1,
    ///     scratch: &mut [u8],
    /// ) -> Result<Option<()>, GeneratedRuntimeStagingErrorV1<B::Error>> {
    ///     output.try_stage_completed_v1(completion, context, host, scratch)
    /// }
    /// ```
    pub fn try_stage_completed_v1<B: RuntimeBackendV1>(
        &mut self,
        receipt: &RuntimeGeneratedCompletionReceiptV1,
        context: &mut RuntimeContextV1<B>,
        staging: RuntimeAllocationIdV1,
        scratch: &mut [u8],
    ) -> Result<Option<()>, GeneratedRuntimeStagingErrorV1<B::Error>> {
        self.stage_completed_matching_v1(
            |gate| receipt.matches_owner(gate),
            context,
            staging,
            scratch,
        )
    }

    fn stage_completed_matching_v1<B: RuntimeBackendV1>(
        &mut self,
        matches: impl FnOnce(&Arc<ResultReadyGateV1>) -> bool,
        context: &mut RuntimeContextV1<B>,
        staging: RuntimeAllocationIdV1,
        scratch: &mut [u8],
    ) -> Result<Option<()>, GeneratedRuntimeStagingErrorV1<B::Error>> {
        let Some(()) = self
            .encode_completed_matching_v1(matches, scratch)
            .map_err(GeneratedRuntimeStagingErrorV1::Output)?
        else {
            return Ok(None);
        };
        // No backend code runs under the output mutex or poisons it on panic.
        context
            .write_host_visible_allocation_v1(staging, scratch)
            .map_err(GeneratedRuntimeStagingErrorV1::Context)?;
        Ok(Some(()))
    }

    fn encode_completed_matching_v1(
        &mut self,
        matches: impl FnOnce(&Arc<ResultReadyGateV1>) -> bool,
        scratch: &mut [u8],
    ) -> Result<Option<()>, Error> {
        let state = match self.slot.state.try_lock() {
            Ok(state) => state,
            Err(std::sync::TryLockError::WouldBlock) => return Ok(None),
            Err(std::sync::TryLockError::Poisoned(_)) => {
                return Err(Error::Custody);
            }
        };
        Self::validate_completed_state_v1(&state, matches)?;
        let OutputState::Prepared { result, .. } = &*state else {
            unreachable!("validated output")
        };
        result.encode_into_v1(scratch)?;
        Ok(Some(()))
    }
}

#[cfg(test)]
mod tests;
