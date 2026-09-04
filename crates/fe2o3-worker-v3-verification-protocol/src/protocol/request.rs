use super::*;

impl WorkerV3VerificationRequestV1 {
    /// Constructs one canonical authority-free request frame.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        challenge: WorkerV3VerificationFreshChallengeV1,
        roster_identity: WorkerV3VerificationRosterIdentityV1,
        policy_identity: WorkerV3VerificationPolicyIdentityV1,
        measurement_identity: WorkerV3VerificationMeasurementIdentityV1,
        load_envelope: WorkerV3VerificationFdPayloadDescriptorV1,
        finalized_hsaco: WorkerV3VerificationFdPayloadDescriptorV1,
        entries: Vec<WorkerV3VerificationEntryCoordinateV1>,
    ) -> Result<Self, WorkerV3VerificationProtocolErrorV1> {
        let payloads = [load_envelope, finalized_hsaco];
        validate_payload_order(&payloads)?;
        validate_entries(&entries)?;
        let entry_bytes = entries.iter().try_fold(0_usize, |total, entry| {
            total
                .checked_add(entry.encoded_len())
                .ok_or(WorkerV3VerificationProtocolErrorV1::LengthOverflow)
        })?;
        let total_len = REQUEST_FIXED_BYTES
            .checked_add(entry_bytes)
            .ok_or(WorkerV3VerificationProtocolErrorV1::LengthOverflow)?;
        let mut canonical_bytes = Vec::new();
        canonical_bytes.try_reserve_exact(total_len).map_err(|_| {
            WorkerV3VerificationProtocolErrorV1::AllocationFailed {
                requested: total_len,
            }
        })?;
        canonical_bytes.extend_from_slice(&WORKER_V3_VERIFICATION_REQUEST_MAGIC_V1);
        canonical_bytes.extend_from_slice(&WORKER_V3_VERIFICATION_REQUEST_VERSION_V1.to_le_bytes());
        canonical_bytes.extend_from_slice(&0_u16.to_le_bytes());
        canonical_bytes.extend_from_slice(&(total_len as u64).to_le_bytes());
        canonical_bytes.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        canonical_bytes
            .extend_from_slice(&(WORKER_V3_VERIFICATION_FD_PAYLOADS_V1 as u16).to_le_bytes());
        canonical_bytes.extend_from_slice(&[0; 6]);
        canonical_bytes.extend_from_slice(challenge.as_bytes());
        canonical_bytes.extend_from_slice(roster_identity.as_bytes());
        canonical_bytes.extend_from_slice(policy_identity.as_bytes());
        canonical_bytes.extend_from_slice(measurement_identity.as_bytes());
        for payload in &payloads {
            payload.encode_into(&mut canonical_bytes);
        }
        for entry in &entries {
            entry.encode_into(&mut canonical_bytes);
        }
        debug_assert_eq!(canonical_bytes.len(), total_len - SHA256_BYTES);
        let identity = WorkerV3VerificationRequestIdentityV1(derive_identity(
            REQUEST_IDENTITY_DOMAIN,
            &canonical_bytes,
        ));
        canonical_bytes.extend_from_slice(identity.as_bytes());
        debug_assert_eq!(canonical_bytes.len(), total_len);
        Ok(Self {
            challenge,
            roster_identity,
            policy_identity,
            measurement_identity,
            payloads,
            entries,
            identity,
            canonical_bytes,
        })
    }

    /// Strictly decodes one complete bounded canonical request frame.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, WorkerV3VerificationProtocolErrorV1> {
        if !(MIN_WORKER_V3_VERIFICATION_REQUEST_BYTES_V1
            ..=MAX_WORKER_V3_VERIFICATION_REQUEST_BYTES_V1)
            .contains(&bytes.len())
        {
            return Err(
                WorkerV3VerificationProtocolErrorV1::RequestLengthOutOfRange {
                    actual: bytes.len(),
                    minimum: MIN_WORKER_V3_VERIFICATION_REQUEST_BYTES_V1,
                    maximum: MAX_WORKER_V3_VERIFICATION_REQUEST_BYTES_V1,
                },
            );
        }
        let mut reader = Reader::new(bytes);
        if reader.fixed::<8>()? != WORKER_V3_VERIFICATION_REQUEST_MAGIC_V1 {
            return Err(WorkerV3VerificationProtocolErrorV1::BadRequestMagic);
        }
        let version = reader.u16()?;
        if version != WORKER_V3_VERIFICATION_REQUEST_VERSION_V1 {
            return Err(WorkerV3VerificationProtocolErrorV1::UnsupportedVersion {
                actual: version,
            });
        }
        let flags = reader.u16()?;
        if flags != 0 {
            return Err(WorkerV3VerificationProtocolErrorV1::UnsupportedFlags { actual: flags });
        }
        let declared_len = reader.u64()?;
        if declared_len != bytes.len() as u64 {
            return Err(WorkerV3VerificationProtocolErrorV1::InvalidTotalLength {
                declared: declared_len,
                actual: bytes.len(),
            });
        }
        let entry_count = reader.u32()? as usize;
        if entry_count == 0 || entry_count > MAX_WORKER_V3_VERIFICATION_ENTRIES_V1 {
            return Err(WorkerV3VerificationProtocolErrorV1::EntryCountOutOfRange {
                actual: entry_count,
                maximum: MAX_WORKER_V3_VERIFICATION_ENTRIES_V1,
            });
        }
        let payload_count = reader.u16()? as usize;
        if payload_count != WORKER_V3_VERIFICATION_FD_PAYLOADS_V1 {
            return Err(WorkerV3VerificationProtocolErrorV1::InvalidPayloadCount {
                actual: payload_count,
            });
        }
        if reader.fixed::<6>()? != [0; 6] {
            return Err(WorkerV3VerificationProtocolErrorV1::NoncanonicalReservedBytes);
        }
        let minimum_len = REQUEST_FIXED_BYTES
            .checked_add(
                entry_count
                    .checked_mul(MIN_ENTRY_COORDINATE_BYTES)
                    .ok_or(WorkerV3VerificationProtocolErrorV1::LengthOverflow)?,
            )
            .ok_or(WorkerV3VerificationProtocolErrorV1::LengthOverflow)?;
        let maximum_len = REQUEST_FIXED_BYTES
            .checked_add(
                entry_count
                    .checked_mul(MAX_ENTRY_COORDINATE_BYTES)
                    .ok_or(WorkerV3VerificationProtocolErrorV1::LengthOverflow)?,
            )
            .ok_or(WorkerV3VerificationProtocolErrorV1::LengthOverflow)?;
        if !(minimum_len..=maximum_len).contains(&bytes.len()) {
            return Err(
                WorkerV3VerificationProtocolErrorV1::InvalidEntrySectionLength {
                    entry_count,
                    actual: bytes.len(),
                    minimum: minimum_len,
                    maximum: maximum_len,
                },
            );
        }
        let challenge = WorkerV3VerificationFreshChallengeV1::new(reader.fixed()?)?;
        let roster_identity = WorkerV3VerificationRosterIdentityV1::new(reader.fixed()?)?;
        let policy_identity = WorkerV3VerificationPolicyIdentityV1::new(reader.fixed()?)?;
        let measurement_identity = WorkerV3VerificationMeasurementIdentityV1::new(reader.fixed()?)?;
        let payloads = [
            WorkerV3VerificationFdPayloadDescriptorV1::decode(&mut reader)?,
            WorkerV3VerificationFdPayloadDescriptorV1::decode(&mut reader)?,
        ];
        let mut entries = Vec::new();
        entries.try_reserve_exact(entry_count).map_err(|_| {
            WorkerV3VerificationProtocolErrorV1::AllocationFailed {
                requested: entry_count,
            }
        })?;
        for _ in 0..entry_count {
            entries.push(WorkerV3VerificationEntryCoordinateV1::decode(&mut reader)?);
        }
        let declared_identity = WorkerV3VerificationRequestIdentityV1(reader.fixed()?);
        if !reader.is_empty() {
            return Err(WorkerV3VerificationProtocolErrorV1::TrailingBytes);
        }
        let decoded = Self::new(
            challenge,
            roster_identity,
            policy_identity,
            measurement_identity,
            payloads[0],
            payloads[1],
            entries,
        )?;
        if decoded.identity != declared_identity || decoded.canonical_bytes.as_slice() != bytes {
            return Err(WorkerV3VerificationProtocolErrorV1::RequestIdentityMismatch);
        }
        Ok(decoded)
    }

    /// Returns the complete canonical request encoding.
    pub fn encode_canonical(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the request identity.
    pub const fn identity(&self) -> WorkerV3VerificationRequestIdentityV1 {
        self.identity
    }

    /// Returns the caller challenge. The caller must separately enforce freshness.
    pub const fn challenge(&self) -> WorkerV3VerificationFreshChallengeV1 {
        self.challenge
    }

    /// Returns the exact roster identity.
    pub const fn roster_identity(&self) -> WorkerV3VerificationRosterIdentityV1 {
        self.roster_identity
    }

    /// Returns the caller-pinned policy identity.
    pub const fn policy_identity(&self) -> WorkerV3VerificationPolicyIdentityV1 {
        self.policy_identity
    }

    /// Returns the caller-pinned verifier measurement identity.
    pub const fn measurement_identity(&self) -> WorkerV3VerificationMeasurementIdentityV1 {
        self.measurement_identity
    }

    /// Returns the two canonical fd payload descriptors in fd order.
    pub const fn payloads(
        &self,
    ) -> &[WorkerV3VerificationFdPayloadDescriptorV1; WORKER_V3_VERIFICATION_FD_PAYLOADS_V1] {
        &self.payloads
    }

    /// Returns the descriptor-table-ordered entry coordinates.
    pub fn entries(&self) -> &[WorkerV3VerificationEntryCoordinateV1] {
        &self.entries
    }

    /// Reports that this inert frame grants no theorem, load, or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}
