//! Complete host destinations remain coupled to the original charged invocation.

use fe2o3_resource_accounting::{ResourceReservationV1, RetainedResourceCreditsV1};

use super::*;
use GeneratedRuntimeArgumentErrorV1 as Error;

pub(crate) struct GeneratedRuntimeReadbackOwnerV1 {
    buffers: Vec<(Gfx942RuntimeBufferAccessV1, Vec<u8>)>,
    credit: Option<RetainedResourceCreditsV1>,
}

impl Drop for GeneratedRuntimeReadbackOwnerV1 {
    fn drop(&mut self) {
        drop(std::mem::take(&mut self.buffers));
        if let Some(credit) = self.credit.take() {
            let _ = credit.release_after_disposal();
        }
    }
}

#[cfg(test)]
impl GeneratedRuntimeReadbackOwnerV1 {
    pub(super) fn buffers(&self) -> &[(Gfx942RuntimeBufferAccessV1, Vec<u8>)] {
        &self.buffers
    }
}

struct PendingReadback {
    buffers: Vec<(Gfx942RuntimeBufferAccessV1, Vec<u8>)>,
    reservation: Option<ResourceReservationV1>,
}

impl Drop for PendingReadback {
    fn drop(&mut self) {
        drop(std::mem::take(&mut self.buffers));
        drop(self.reservation.take());
    }
}

impl GeneratedRuntimeStorageV1<PreparedGfx942PersistentDispatchV1> {
    pub(crate) fn prepare_readback(&self) -> Result<GeneratedRuntimeReadbackOwnerV1, Error> {
        self.prepare_readback_with(|length| {
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(length)
                .map_err(|_| Error::Allocation)?;
            bytes.resize(length, 0);
            Ok(bytes)
        })
    }

    pub(crate) fn install_readback(&mut self, readback: GeneratedRuntimeReadbackOwnerV1) {
        assert!(self.readback.is_none(), "one readback installation");
        self.readback = Some(readback);
    }

    pub(super) fn prepare_readback_with(
        &self,
        mut allocate: impl FnMut(usize) -> Result<Vec<u8>, Error>,
    ) -> Result<GeneratedRuntimeReadbackOwnerV1, Error> {
        if self.readback.is_some() {
            return Err(Error::StaleOrAliasedOutput);
        }
        let gate = self
            .decoder
            .result_gate
            .as_ref()
            .ok_or(Error::BindingMismatch)?;
        let mut ordinal = 0;
        let mut total = 0u64;
        for expected in &self.decoder.expectations {
            if expected.byte_len != 0 {
                let buffer = self
                    .payload
                    .buffers()
                    .get(ordinal)
                    .ok_or(Error::BindingMismatch)?;
                if buffer.bytes().len() != expected.byte_len
                    || self.payload.buffer_access(ordinal) != Some(expected.access)
                {
                    return Err(Error::BindingMismatch);
                }
                total = total
                    .checked_add(u64::try_from(expected.byte_len).map_err(|_| Error::ByteLength)?)
                    .ok_or(Error::ByteLength)?;
                ordinal += 1;
            }
            if let Some(custody) = &expected.custody {
                custody.bound_to(Some(gate))?;
            } else if !expected
                .read_credit
                .as_ref()
                .is_some_and(|credit| credit.bound_to(gate))
            {
                return Err(Error::BindingMismatch);
            }
        }
        if ordinal != self.payload.buffers().len() {
            return Err(Error::BindingMismatch);
        }
        // Reserve the full overlap before metadata or destination allocation.
        let mut pending = PendingReadback {
            buffers: Vec::new(),
            reservation: Some(gate.reserve_readback(total)?),
        };
        pending
            .buffers
            .try_reserve_exact(ordinal)
            .map_err(|_| Error::Allocation)?;
        for expected in self
            .decoder
            .expectations
            .iter()
            .filter(|expected| expected.byte_len != 0)
        {
            let bytes = allocate(expected.byte_len)?;
            if bytes.len() != expected.byte_len || bytes.capacity() != expected.byte_len {
                return Err(Error::BindingMismatch);
            }
            pending.buffers.push((expected.access, bytes));
        }
        let credit = pending
            .reservation
            .take()
            .expect("reserved complete readback")
            .retain();
        Ok(GeneratedRuntimeReadbackOwnerV1 {
            buffers: std::mem::take(&mut pending.buffers),
            credit: Some(credit),
        })
    }
}
