//! Closed native CWSR layouts. Geometry is not target or queue admission.

use super::{CWSR_HEADER_BYTES, NativeAqlSubmissionErrorV1};
use fe2o3_kfd_uapi::{
    KfdContextSaveAreaHeaderV1, KfdQueueExceptionPayloadAddressV1, KfdSignalEventIdV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CwsrGeometryV1 {
    Gfx942,
    #[cfg(feature = "engineering-gfx950")]
    Gfx950Observed,
}

#[cfg(all(test, feature = "engineering-gfx950"))]
mod tests {
    use super::*;
    #[test]
    fn observed_950_headers_have_exact_tail_and_event_fields_at_every_stride() {
        let geometry = CwsrGeometryV1::Gfx950Observed;
        let payload = KfdQueueExceptionPayloadAddressV1::new(0x12340000).unwrap();
        let event = KfdSignalEventIdV1::new(7).unwrap();
        assert_eq!(geometry.total_bytes(), 8 * 0x15a3000 + 0x50000);
        for xcc in 0..8 {
            let header = geometry.header(xcc, payload, event).unwrap();
            assert_eq!(&header[..16], &[0; 16]);
            assert_eq!(
                u32::from_le_bytes(header[16..20].try_into().unwrap()),
                ((8 - xcc) * 0x15a3000) as u32
            );
            assert_eq!(&header[20..24], &0x50000u32.to_le_bytes());
            assert_eq!(&header[24..32], &0x12340000u64.to_le_bytes());
            assert_eq!(&header[32..36], &7u32.to_le_bytes());
            assert_eq!(&header[36..], &[0; 4]);
            assert_ne!(
                header,
                CwsrGeometryV1::Gfx942.header(xcc, payload, event).unwrap()
            );
        }
        assert!(geometry.header(8, payload, event).is_err());
        assert!(geometry.initialize(&mut [0; 4096], payload, event).is_err());
    }
}

impl CwsrGeometryV1 {
    pub(crate) const fn xccs(self) -> usize {
        8
    }
    pub(crate) const fn control_bytes(self) -> usize {
        0x3000
    }
    pub(crate) const fn context_bytes(self) -> usize {
        match self {
            Self::Gfx942 => super::GFX942_CWSR_CONTEXT_BYTES_PER_XCC_V1,
            #[cfg(feature = "engineering-gfx950")]
            Self::Gfx950Observed => 0x15a_3000,
        }
    }
    pub(crate) const fn total_bytes(self) -> usize {
        match self {
            Self::Gfx942 => super::GFX942_CWSR_TOTAL_BYTES_V1,
            #[cfg(feature = "engineering-gfx950")]
            Self::Gfx950Observed => 0xad6_8000,
        }
    }
    pub(crate) const fn debug_bytes(self) -> u32 {
        match self {
            Self::Gfx942 => super::GFX942_CWSR_DEBUG_BYTES_TOTAL_V1,
            #[cfg(feature = "engineering-gfx950")]
            Self::Gfx950Observed => 0x5_0000,
        }
    }
    pub(crate) fn header(
        self,
        xcc: usize,
        payload: KfdQueueExceptionPayloadAddressV1,
        event_id: KfdSignalEventIdV1,
    ) -> Result<[u8; CWSR_HEADER_BYTES], NativeAqlSubmissionErrorV1> {
        if xcc >= self.xccs() {
            return Err(NativeAqlSubmissionErrorV1::InvalidCwsr("XCC index"));
        }
        let debug_offset = u32::try_from(
            (self.xccs() - xcc)
                .checked_mul(self.context_bytes())
                .ok_or(NativeAqlSubmissionErrorV1::InvalidCwsr("debug offset"))?,
        )
        .map_err(|_| NativeAqlSubmissionErrorV1::InvalidCwsr("debug offset width"))?;
        let header = KfdContextSaveAreaHeaderV1::new_queue_exception(
            debug_offset,
            self.debug_bytes(),
            payload,
            event_id,
        )
        .map_err(|_| NativeAqlSubmissionErrorV1::InvalidCwsr("typed header"))?;
        let mut bytes = [0; CWSR_HEADER_BYTES];
        for (index, word) in header.wave_state_words().iter().enumerate() {
            bytes[index * 4..index * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        bytes[16..20].copy_from_slice(&header.debug_offset().to_le_bytes());
        bytes[20..24].copy_from_slice(&header.debug_size().to_le_bytes());
        bytes[24..32].copy_from_slice(&header.error_payload_address().to_le_bytes());
        bytes[32..36].copy_from_slice(&header.error_event_id().to_le_bytes());
        bytes[36..40].copy_from_slice(&header.reserved().to_le_bytes());
        Ok(bytes)
    }
    pub(crate) fn initialize(
        self,
        bytes: &mut [u8],
        payload: KfdQueueExceptionPayloadAddressV1,
        event_id: KfdSignalEventIdV1,
    ) -> Result<(), NativeAqlSubmissionErrorV1> {
        if bytes.len() != self.total_bytes() {
            return Err(NativeAqlSubmissionErrorV1::InvalidCwsr("mapping length"));
        }
        for xcc in 0..self.xccs() {
            let offset = xcc
                .checked_mul(self.context_bytes())
                .ok_or(NativeAqlSubmissionErrorV1::InvalidCwsr("header offset"))?;
            let end = offset
                .checked_add(CWSR_HEADER_BYTES)
                .ok_or(NativeAqlSubmissionErrorV1::InvalidCwsr("header end"))?;
            bytes
                .get_mut(offset..end)
                .ok_or(NativeAqlSubmissionErrorV1::InvalidCwsr("header range"))?
                .copy_from_slice(&self.header(xcc, payload, event_id)?);
        }
        Ok(())
    }
}
