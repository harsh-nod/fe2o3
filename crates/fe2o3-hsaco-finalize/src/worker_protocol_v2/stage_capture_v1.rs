//! Exact worker checkpoints. Only response decoding can create this borrowed view.

use std::ops::Range;

use super::{Cursor, WorkerDerivationEvidenceV1, WorkerProtocolError};

/// Aggregate retained checkpoint payload bound, independent of the HSACO bound.
pub const MAX_WORKER_STAGE_CAPTURE_BYTES_V1: usize = 1024 * 1024;
pub(super) const MAX_STAGE_CAPTURE_BODY_BYTES: usize = MAX_WORKER_STAGE_CAPTURE_BYTES_V1 + 16;

/// Inert bytes from one response, checked against its derivation record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerStageCaptureV1<'a> {
    linked_bitcode: &'a [u8],
    optimized_bitcode: &'a [u8],
    generated_object: &'a [u8],
}

impl<'a> WorkerStageCaptureV1<'a> {
    #[cfg(test)]
    pub(crate) const fn from_test_contents(
        linked_bitcode: &'a [u8],
        optimized_bitcode: &'a [u8],
        generated_object: &'a [u8],
    ) -> Self {
        Self {
            linked_bitcode,
            optimized_bitcode,
            generated_object,
        }
    }

    pub const fn linked_bitcode(self) -> &'a [u8] {
        self.linked_bitcode
    }

    pub const fn optimized_bitcode(self) -> &'a [u8] {
        self.optimized_bitcode
    }

    pub const fn generated_object(self) -> &'a [u8] {
        self.generated_object
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StageCaptureRangesV1([Range<usize>; 3]);

impl StageCaptureRangesV1 {
    pub(super) fn decode(
        body: &[u8],
        body_offset: usize,
        derivation: &WorkerDerivationEvidenceV1,
    ) -> Result<Self, WorkerProtocolError> {
        if body.len() > MAX_STAGE_CAPTURE_BODY_BYTES {
            return Err(WorkerProtocolError::FieldTooLarge(10));
        }
        let mut cursor = Cursor::new(body);
        if cursor.byte()? != 1 {
            return Err(WorkerProtocolError::UnknownEnum("stage capture version"));
        }
        let identities = [
            derivation.linked_module(),
            derivation.optimized_module(),
            derivation.generated_object(),
        ];
        let mut ranges = std::array::from_fn(|_| 0..0);
        let mut remaining = MAX_WORKER_STAGE_CAPTURE_BYTES_V1;
        for (index, identity) in identities.into_iter().enumerate() {
            if usize::from(cursor.byte()?) != index + 1 {
                return Err(WorkerProtocolError::NonCanonicalEncoding);
            }
            let len = cursor.u32()? as usize;
            if len == 0 || len > remaining {
                return Err(WorkerProtocolError::FieldTooLarge(10));
            }
            remaining -= len;
            let start = body_offset
                .checked_add(cursor.position)
                .ok_or(WorkerProtocolError::IntegerOverflow)?;
            let bytes = cursor.take(len)?;
            if !identity.matches(bytes) {
                return Err(WorkerProtocolError::ContentIdentityMismatch);
            }
            let end = start
                .checked_add(len)
                .ok_or(WorkerProtocolError::IntegerOverflow)?;
            ranges[index] = start..end;
        }
        cursor.finish()?;
        Ok(Self(ranges))
    }

    pub(super) fn view<'a>(&self, canonical_response: &'a [u8]) -> WorkerStageCaptureV1<'a> {
        WorkerStageCaptureV1 {
            linked_bitcode: &canonical_response[self.0[0].clone()],
            optimized_bitcode: &canonical_response[self.0[1].clone()],
            generated_object: &canonical_response[self.0[2].clone()],
        }
    }
}
