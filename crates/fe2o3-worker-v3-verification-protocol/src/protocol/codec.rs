use core::str;

use sha2::{Digest, Sha256};

use super::*;

pub(super) fn validate_payload_order(
    payloads: &[WorkerV3VerificationFdPayloadDescriptorV1; WORKER_V3_VERIFICATION_FD_PAYLOADS_V1],
) -> Result<(), WorkerV3VerificationProtocolErrorV1> {
    for (actual, expected) in payloads.iter().zip([
        WorkerV3VerificationFdPayloadKindV1::LoadEnvelopeV2,
        WorkerV3VerificationFdPayloadKindV1::FinalizedHsaco,
    ]) {
        if actual.kind != expected {
            return Err(WorkerV3VerificationProtocolErrorV1::UnexpectedPayloadKind {
                expected,
                actual: actual.kind,
            });
        }
    }
    Ok(())
}

pub(super) fn validate_entries(
    entries: &[WorkerV3VerificationEntryCoordinateV1],
) -> Result<(), WorkerV3VerificationProtocolErrorV1> {
    if entries.is_empty() || entries.len() > MAX_WORKER_V3_VERIFICATION_ENTRIES_V1 {
        return Err(WorkerV3VerificationProtocolErrorV1::EntryCountOutOfRange {
            actual: entries.len(),
            maximum: MAX_WORKER_V3_VERIFICATION_ENTRIES_V1,
        });
    }
    for (position, entry) in entries.iter().enumerate() {
        let expected = position as u32;
        if entry.ordinal != expected {
            return Err(
                WorkerV3VerificationProtocolErrorV1::UnexpectedEntryOrdinal {
                    expected,
                    actual: entry.ordinal,
                },
            );
        }
        for (first_position, first) in entries[..position].iter().enumerate() {
            let duplicate = if entry.logical_name == first.logical_name {
                Some(WorkerV3VerificationEntryIdentityFieldV1::LogicalName)
            } else if entry.export_name == first.export_name {
                Some(WorkerV3VerificationEntryIdentityFieldV1::ExportName)
            } else if entry.lineage_identity == first.lineage_identity {
                Some(WorkerV3VerificationEntryIdentityFieldV1::Lineage)
            } else if entry.marker_binding_identity == first.marker_binding_identity {
                Some(WorkerV3VerificationEntryIdentityFieldV1::MarkerBinding)
            } else {
                None
            };
            if let Some(field) = duplicate {
                return Err(
                    WorkerV3VerificationProtocolErrorV1::DuplicateEntryIdentity {
                        field,
                        first_ordinal: first_position as u32,
                        duplicate_ordinal: entry.ordinal,
                    },
                );
            }
        }
    }
    Ok(())
}

pub(super) fn derive_identity(domain: &[u8], bytes: &[u8]) -> [u8; SHA256_BYTES] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

pub(super) fn put<const N: usize>(output: &mut [u8], offset: &mut usize, bytes: &[u8; N]) {
    output[*offset..*offset + N].copy_from_slice(bytes);
    *offset += N;
}

pub(super) struct Reader<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
}

impl<'bytes> Reader<'bytes> {
    pub(super) const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    pub(super) fn take(
        &mut self,
        length: usize,
    ) -> Result<&'bytes [u8], WorkerV3VerificationProtocolErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(WorkerV3VerificationProtocolErrorV1::LengthOverflow)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(WorkerV3VerificationProtocolErrorV1::Truncated)?;
        self.offset = end;
        Ok(bytes)
    }

    pub(super) fn fixed<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], WorkerV3VerificationProtocolErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| WorkerV3VerificationProtocolErrorV1::Truncated)
    }

    pub(super) fn u16(&mut self) -> Result<u16, WorkerV3VerificationProtocolErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    pub(super) fn u32(&mut self) -> Result<u32, WorkerV3VerificationProtocolErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    pub(super) fn u64(&mut self) -> Result<u64, WorkerV3VerificationProtocolErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    pub(super) fn entry_name_len(
        &mut self,
        ordinal: u32,
        field: WorkerV3VerificationEntryNameFieldV1,
    ) -> Result<usize, WorkerV3VerificationProtocolErrorV1> {
        let length = usize::from(self.u16()?);
        if length > MAX_NAME_BYTES {
            return Err(WorkerV3VerificationProtocolErrorV1::InvalidEntryName {
                ordinal,
                field,
                source: ValidationError::TooLong {
                    field: "name",
                    max: MAX_NAME_BYTES,
                },
            });
        }
        Ok(length)
    }

    pub(super) fn entry_name(
        &mut self,
        ordinal: u32,
        field: WorkerV3VerificationEntryNameFieldV1,
        length: usize,
    ) -> Result<&'bytes str, WorkerV3VerificationProtocolErrorV1> {
        str::from_utf8(self.take(length)?).map_err(|_| {
            WorkerV3VerificationProtocolErrorV1::InvalidEntryName {
                ordinal,
                field,
                source: ValidationError::InvalidText { field: "name" },
            }
        })
    }

    pub(super) const fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
