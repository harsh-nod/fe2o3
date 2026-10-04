use super::*;

impl WorkerV3VerificationResponseDispositionV1 {
    const fn wire_tag(self) -> u16 {
        match self {
            Self::RequestFramed => 1,
            Self::RequestRejected => 2,
        }
    }

    const fn from_wire_tag(tag: u16) -> Option<Self> {
        match tag {
            1 => Some(Self::RequestFramed),
            2 => Some(Self::RequestRejected),
            _ => None,
        }
    }
}

impl WorkerV3VerificationResponseV1 {
    /// Constructs a framing-only response bound to one exact decoded request.
    pub fn new(
        request: &WorkerV3VerificationRequestV1,
        disposition: WorkerV3VerificationResponseDispositionV1,
        transcript_identity: WorkerV3VerificationTranscriptIdentityV1,
    ) -> Self {
        Self::encode(ResponseFields {
            disposition,
            entry_count: request.entries.len() as u32,
            request_identity: request.identity,
            challenge: request.challenge,
            roster_identity: request.roster_identity,
            policy_identity: request.policy_identity,
            measurement_identity: request.measurement_identity,
            transcript_identity,
        })
    }

    /// Strictly decodes one complete canonical response frame.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, WorkerV3VerificationProtocolErrorV1> {
        if bytes.len() != WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1 {
            return Err(WorkerV3VerificationProtocolErrorV1::InvalidResponseLength {
                actual: bytes.len(),
            });
        }
        let mut reader = Reader::new(bytes);
        if reader.fixed::<8>()? != WORKER_V3_VERIFICATION_RESPONSE_MAGIC_V1 {
            return Err(WorkerV3VerificationProtocolErrorV1::BadResponseMagic);
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
        if declared_len != WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1 as u64 {
            return Err(WorkerV3VerificationProtocolErrorV1::InvalidTotalLength {
                declared: declared_len,
                actual: bytes.len(),
            });
        }
        let disposition_tag = reader.u16()?;
        let disposition = WorkerV3VerificationResponseDispositionV1::from_wire_tag(disposition_tag)
            .ok_or(
                WorkerV3VerificationProtocolErrorV1::UnknownResponseDisposition {
                    actual: disposition_tag,
                },
            )?;
        if reader.u16()? != 0 {
            return Err(WorkerV3VerificationProtocolErrorV1::NoncanonicalReservedBytes);
        }
        let entry_count = reader.u32()?;
        if entry_count == 0 || entry_count as usize > MAX_WORKER_V3_VERIFICATION_ENTRIES_V1 {
            return Err(WorkerV3VerificationProtocolErrorV1::EntryCountOutOfRange {
                actual: entry_count as usize,
                maximum: MAX_WORKER_V3_VERIFICATION_ENTRIES_V1,
            });
        }
        if reader.u32()? != 0 {
            return Err(WorkerV3VerificationProtocolErrorV1::NoncanonicalReservedBytes);
        }
        let fields = ResponseFields {
            disposition,
            entry_count,
            request_identity: WorkerV3VerificationRequestIdentityV1(reader.fixed()?),
            challenge: WorkerV3VerificationFreshChallengeV1::new(reader.fixed()?)?,
            roster_identity: WorkerV3VerificationRosterIdentityV1::new(reader.fixed()?)?,
            policy_identity: WorkerV3VerificationPolicyIdentityV1::new(reader.fixed()?)?,
            measurement_identity: WorkerV3VerificationMeasurementIdentityV1::new(reader.fixed()?)?,
            transcript_identity: WorkerV3VerificationTranscriptIdentityV1::new(reader.fixed()?)?,
        };
        let declared_identity = WorkerV3VerificationResponseIdentityV1(reader.fixed()?);
        if !reader.is_empty() {
            return Err(WorkerV3VerificationProtocolErrorV1::TrailingBytes);
        }
        let decoded = Self::encode(fields);
        if decoded.identity != declared_identity || decoded.canonical_bytes.as_slice() != bytes {
            return Err(WorkerV3VerificationProtocolErrorV1::ResponseIdentityMismatch);
        }
        Ok(decoded)
    }

    fn encode(fields: ResponseFields) -> Self {
        let mut canonical_bytes = [0_u8; WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1];
        let mut offset = 0;
        put(
            &mut canonical_bytes,
            &mut offset,
            &WORKER_V3_VERIFICATION_RESPONSE_MAGIC_V1,
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            &WORKER_V3_VERIFICATION_REQUEST_VERSION_V1.to_le_bytes(),
        );
        put(&mut canonical_bytes, &mut offset, &0_u16.to_le_bytes());
        put(
            &mut canonical_bytes,
            &mut offset,
            &(WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1 as u64).to_le_bytes(),
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            &fields.disposition.wire_tag().to_le_bytes(),
        );
        put(&mut canonical_bytes, &mut offset, &0_u16.to_le_bytes());
        put(
            &mut canonical_bytes,
            &mut offset,
            &fields.entry_count.to_le_bytes(),
        );
        put(&mut canonical_bytes, &mut offset, &0_u32.to_le_bytes());
        put(
            &mut canonical_bytes,
            &mut offset,
            fields.request_identity.as_bytes(),
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            fields.challenge.as_bytes(),
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            fields.roster_identity.as_bytes(),
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            fields.policy_identity.as_bytes(),
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            fields.measurement_identity.as_bytes(),
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            fields.transcript_identity.as_bytes(),
        );
        debug_assert_eq!(offset, RESPONSE_PREIMAGE_BYTES);
        let identity = WorkerV3VerificationResponseIdentityV1(derive_identity(
            RESPONSE_IDENTITY_DOMAIN,
            &canonical_bytes[..offset],
        ));
        put(&mut canonical_bytes, &mut offset, identity.as_bytes());
        debug_assert_eq!(offset, canonical_bytes.len());
        Self {
            disposition: fields.disposition,
            entry_count: fields.entry_count,
            request_identity: fields.request_identity,
            challenge: fields.challenge,
            roster_identity: fields.roster_identity,
            policy_identity: fields.policy_identity,
            measurement_identity: fields.measurement_identity,
            transcript_identity: fields.transcript_identity,
            identity,
            canonical_bytes,
        }
    }

    /// Returns the complete canonical response encoding.
    pub const fn encode_canonical(&self) -> &[u8; WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1] {
        &self.canonical_bytes
    }

    /// Returns the framing disposition, never a theorem result.
    pub const fn disposition(&self) -> WorkerV3VerificationResponseDispositionV1 {
        self.disposition
    }

    /// Returns the roster entry count copied from the exact request.
    pub const fn entry_count(&self) -> u32 {
        self.entry_count
    }

    /// Returns the exact request-frame identity.
    pub const fn request_identity(&self) -> WorkerV3VerificationRequestIdentityV1 {
        self.request_identity
    }

    /// Returns the caller challenge copied from the exact request.
    pub const fn challenge(&self) -> WorkerV3VerificationFreshChallengeV1 {
        self.challenge
    }

    /// Returns the roster identity copied from the exact request.
    pub const fn roster_identity(&self) -> WorkerV3VerificationRosterIdentityV1 {
        self.roster_identity
    }

    /// Returns the policy identity copied from the exact request.
    pub const fn policy_identity(&self) -> WorkerV3VerificationPolicyIdentityV1 {
        self.policy_identity
    }

    /// Returns the expected measurement identity copied from the exact request.
    pub const fn measurement_identity(&self) -> WorkerV3VerificationMeasurementIdentityV1 {
        self.measurement_identity
    }

    /// Returns the authority-free service transcript identity.
    pub const fn transcript_identity(&self) -> WorkerV3VerificationTranscriptIdentityV1 {
        self.transcript_identity
    }

    /// Returns the exact response-frame identity.
    pub const fn identity(&self) -> WorkerV3VerificationResponseIdentityV1 {
        self.identity
    }

    /// Checks every copied request coordinate and the exact request identity.
    pub fn matches_request(&self, request: &WorkerV3VerificationRequestV1) -> bool {
        self.request_identity == request.identity
            && self.entry_count as usize == request.entries.len()
            && self.challenge == request.challenge
            && self.roster_identity == request.roster_identity
            && self.policy_identity == request.policy_identity
            && self.measurement_identity == request.measurement_identity
    }

    /// Reports that this inert frame grants no theorem, load, or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Reports that canonical decoding does not authenticate the service peer.
    pub const fn authenticates_service(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy)]
struct ResponseFields {
    disposition: WorkerV3VerificationResponseDispositionV1,
    entry_count: u32,
    request_identity: WorkerV3VerificationRequestIdentityV1,
    challenge: WorkerV3VerificationFreshChallengeV1,
    roster_identity: WorkerV3VerificationRosterIdentityV1,
    policy_identity: WorkerV3VerificationPolicyIdentityV1,
    measurement_identity: WorkerV3VerificationMeasurementIdentityV1,
    transcript_identity: WorkerV3VerificationTranscriptIdentityV1,
}
