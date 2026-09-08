use core::mem::size_of;

use sha2::{Digest as _, Sha256};

use super::*;

impl InertCapabilityObligationSetV1 {
    pub fn from_specs(
        subject: CapabilitySubjectV1,
        specs: Vec<CapabilityObligationSpecV1>,
    ) -> Result<Self, CapabilityCodecErrorV1> {
        validate_subject(subject)?;
        limit(
            CapabilityResourceV1::Obligations,
            specs.len(),
            MAX_CAPABILITY_OBLIGATIONS_V1,
        )?;
        if specs.is_empty() {
            return Err(CapabilityCodecErrorV1::EmptyObligations);
        }

        let mut obligations = reserve_vec(specs.len(), CapabilityResourceV1::Obligations)?;
        for (index, spec) in specs.into_iter().enumerate() {
            validate_property(spec.property, index)?;
            if !spec.statement.is_valid() {
                return Err(CapabilityCodecErrorV1::InvalidIdentity {
                    field: CapabilityIdentityFieldV1::Statement,
                    index: Some(index),
                });
            }
            obligations.push(CapabilityObligationV1 {
                identity: derive_obligation_identity(subject, spec.property, spec.statement),
                property: spec.property,
                statement: spec.statement,
            });
        }
        obligations.sort_unstable_by_key(|record| record.identity);
        validate_obligation_records(&obligations)?;
        encode_obligation_set(subject, obligations)
    }

    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, CapabilityCodecErrorV1> {
        if bytes.len() > MAX_CAPABILITY_OBLIGATION_SET_BYTES_V1 {
            return Err(CapabilityCodecErrorV1::LimitExceeded {
                resource: CapabilityResourceV1::ObligationSetBytes,
                actual: bytes.len(),
                maximum: MAX_CAPABILITY_OBLIGATION_SET_BYTES_V1,
            });
        }
        let mut reader = Reader::new(bytes);
        decode_header(
            &mut reader,
            CapabilityRecordKindV1::ObligationSet,
            OBLIGATION_MAGIC_V1,
            CAPABILITY_OBLIGATION_SET_VERSION_V1,
        )?;
        let subject = decode_subject(&mut reader)?;
        let count = reader.count(
            CapabilityResourceV1::Obligations,
            MAX_CAPABILITY_OBLIGATIONS_V1,
        )?;
        reader.require_bytes(
            count,
            OBLIGATION_RECORD_BYTES_V1,
            TERMINAL_IDENTITY_BYTES_V1,
        )?;
        if count == 0 {
            return Err(CapabilityCodecErrorV1::EmptyObligations);
        }

        let mut obligations = reserve_vec(count, CapabilityResourceV1::Obligations)?;
        for index in 0..count {
            let property = decode_property(&mut reader, index)?;
            let statement = StatementIdentityV1::from_untrusted_digest(reader.digest()?);
            if !statement.is_valid() {
                return Err(CapabilityCodecErrorV1::InvalidIdentity {
                    field: CapabilityIdentityFieldV1::Statement,
                    index: Some(index),
                });
            }
            let identity = CapabilityObligationIdentityV1::from_untrusted_digest(reader.digest()?);
            if !identity.is_valid() {
                return Err(CapabilityCodecErrorV1::InvalidIdentity {
                    field: CapabilityIdentityFieldV1::Obligation,
                    index: Some(index),
                });
            }
            if identity != derive_obligation_identity(subject, property, statement) {
                return Err(CapabilityCodecErrorV1::IdentityMismatch {
                    record: CapabilityRecordKindV1::Obligation,
                    index: Some(index),
                });
            }
            obligations.push(CapabilityObligationV1 {
                identity,
                property,
                statement,
            });
        }
        validate_obligation_records(&obligations)?;
        let terminal_offset = reader.offset();
        let identity =
            InertCapabilityObligationSetIdentityV1::from_untrusted_digest(reader.digest()?);
        if !identity.is_valid() {
            return Err(CapabilityCodecErrorV1::InvalidIdentity {
                field: CapabilityIdentityFieldV1::ObligationSet,
                index: None,
            });
        }
        reader.finish()?;
        if identity.digest()
            != derive_identity(OBLIGATION_SET_IDENTITY_DOMAIN_V1, &bytes[..terminal_offset])
        {
            return Err(CapabilityCodecErrorV1::IdentityMismatch {
                record: CapabilityRecordKindV1::ObligationSet,
                index: None,
            });
        }
        Ok(Self {
            subject,
            obligations,
            identity,
            canonical_bytes: copy_bytes(bytes, CapabilityResourceV1::ObligationSetBytes)?,
        })
    }

    pub const fn subject(&self) -> CapabilitySubjectV1 {
        self.subject
    }

    pub fn obligations(&self) -> &[CapabilityObligationV1] {
        &self.obligations
    }

    pub const fn identity(&self) -> InertCapabilityObligationSetIdentityV1 {
        self.identity
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

impl InertCapabilityResultSetV1 {
    pub fn from_specs(
        subject: CapabilitySubjectV1,
        obligation_set: InertCapabilityObligationSetIdentityV1,
        specs: Vec<CapabilityResultSpecV1>,
    ) -> Result<Self, CapabilityCodecErrorV1> {
        validate_subject(subject)?;
        if !obligation_set.is_valid() {
            return Err(CapabilityCodecErrorV1::InvalidIdentity {
                field: CapabilityIdentityFieldV1::ObligationSet,
                index: None,
            });
        }
        limit(
            CapabilityResourceV1::Results,
            specs.len(),
            MAX_CAPABILITY_OBLIGATIONS_V1,
        )?;

        let mut witness_bytes = 0_usize;
        let mut results = reserve_vec(specs.len(), CapabilityResourceV1::Results)?;
        for (index, spec) in specs.into_iter().enumerate() {
            if !spec.obligation.is_valid() {
                return Err(CapabilityCodecErrorV1::InvalidIdentity {
                    field: CapabilityIdentityFieldV1::Obligation,
                    index: Some(index),
                });
            }
            validate_outcome(&spec.outcome, index, &mut witness_bytes)?;
            let identity =
                derive_result_identity(subject, obligation_set, spec.obligation, &spec.outcome)?;
            results.push(CapabilityResultV1 {
                identity,
                obligation: spec.obligation,
                outcome: spec.outcome,
            });
        }
        results.sort_unstable_by_key(|record| record.obligation);
        validate_result_records(&results)?;
        encode_result_set(subject, obligation_set, results)
    }

    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, CapabilityCodecErrorV1> {
        if bytes.len() > MAX_CAPABILITY_RESULT_SET_BYTES_V1 {
            return Err(CapabilityCodecErrorV1::LimitExceeded {
                resource: CapabilityResourceV1::ResultSetBytes,
                actual: bytes.len(),
                maximum: MAX_CAPABILITY_RESULT_SET_BYTES_V1,
            });
        }
        let mut reader = Reader::new(bytes);
        decode_header(
            &mut reader,
            CapabilityRecordKindV1::ResultSet,
            RESULT_MAGIC_V1,
            CAPABILITY_RESULT_SET_VERSION_V1,
        )?;
        let subject = decode_subject(&mut reader)?;
        let obligation_set =
            InertCapabilityObligationSetIdentityV1::from_untrusted_digest(reader.digest()?);
        if !obligation_set.is_valid() {
            return Err(CapabilityCodecErrorV1::InvalidIdentity {
                field: CapabilityIdentityFieldV1::ObligationSet,
                index: None,
            });
        }
        let count = reader.count(CapabilityResourceV1::Results, MAX_CAPABILITY_OBLIGATIONS_V1)?;
        reader.require_bytes(
            count,
            MIN_RESULT_RECORD_BYTES_V1,
            TERMINAL_IDENTITY_BYTES_V1,
        )?;

        let mut witness_bytes = 0_usize;
        let mut results = reserve_vec(count, CapabilityResourceV1::Results)?;
        for index in 0..count {
            let obligation =
                CapabilityObligationIdentityV1::from_untrusted_digest(reader.digest()?);
            if !obligation.is_valid() {
                return Err(CapabilityCodecErrorV1::InvalidIdentity {
                    field: CapabilityIdentityFieldV1::Obligation,
                    index: Some(index),
                });
            }
            let identity = CapabilityResultIdentityV1::from_untrusted_digest(reader.digest()?);
            if !identity.is_valid() {
                return Err(CapabilityCodecErrorV1::InvalidIdentity {
                    field: CapabilityIdentityFieldV1::Result,
                    index: Some(index),
                });
            }
            let outcome = decode_outcome(&mut reader, index, &mut witness_bytes)?;
            if identity != derive_result_identity(subject, obligation_set, obligation, &outcome)? {
                return Err(CapabilityCodecErrorV1::IdentityMismatch {
                    record: CapabilityRecordKindV1::Result,
                    index: Some(index),
                });
            }
            results.push(CapabilityResultV1 {
                identity,
                obligation,
                outcome,
            });
        }
        validate_result_records(&results)?;
        let terminal_offset = reader.offset();
        let identity = InertCapabilityResultSetIdentityV1::from_untrusted_digest(reader.digest()?);
        if !identity.is_valid() {
            return Err(CapabilityCodecErrorV1::InvalidIdentity {
                field: CapabilityIdentityFieldV1::ResultSet,
                index: None,
            });
        }
        reader.finish()?;
        if identity.digest()
            != derive_identity(RESULT_SET_IDENTITY_DOMAIN_V1, &bytes[..terminal_offset])
        {
            return Err(CapabilityCodecErrorV1::IdentityMismatch {
                record: CapabilityRecordKindV1::ResultSet,
                index: None,
            });
        }
        Ok(Self {
            subject,
            obligation_set,
            results,
            identity,
            canonical_bytes: copy_bytes(bytes, CapabilityResourceV1::ResultSetBytes)?,
        })
    }

    pub const fn subject(&self) -> CapabilitySubjectV1 {
        self.subject
    }

    pub const fn obligation_set(&self) -> InertCapabilityObligationSetIdentityV1 {
        self.obligation_set
    }

    pub fn results(&self) -> &[CapabilityResultV1] {
        &self.results
    }

    pub const fn identity(&self) -> InertCapabilityResultSetIdentityV1 {
        self.identity
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
}

fn encode_obligation_set(
    subject: CapabilitySubjectV1,
    obligations: Vec<CapabilityObligationV1>,
) -> Result<InertCapabilityObligationSetV1, CapabilityCodecErrorV1> {
    let records_bytes = obligations
        .len()
        .checked_mul(OBLIGATION_RECORD_BYTES_V1)
        .ok_or(CapabilityCodecErrorV1::LengthOverflow)?;
    let total = OBLIGATION_PREAMBLE_BYTES_V1
        .checked_add(records_bytes)
        .and_then(|value| value.checked_add(TERMINAL_IDENTITY_BYTES_V1))
        .ok_or(CapabilityCodecErrorV1::LengthOverflow)?;
    limit(
        CapabilityResourceV1::ObligationSetBytes,
        total,
        MAX_CAPABILITY_OBLIGATION_SET_BYTES_V1,
    )?;
    let mut writer = Writer::new(total, CapabilityResourceV1::ObligationSetBytes)?;
    encode_header(
        &mut writer,
        OBLIGATION_MAGIC_V1,
        CAPABILITY_OBLIGATION_SET_VERSION_V1,
        total,
    )?;
    encode_subject(&mut writer, subject);
    writer.count(obligations.len())?;
    for obligation in &obligations {
        encode_property(&mut writer, obligation.property);
        writer.digest(obligation.statement.digest());
        writer.digest(obligation.identity.digest());
    }
    let identity = InertCapabilityObligationSetIdentityV1::from_untrusted_digest(derive_identity(
        OBLIGATION_SET_IDENTITY_DOMAIN_V1,
        writer.bytes(),
    ));
    writer.digest(identity.digest());
    let canonical_bytes = writer.finish(total)?;
    Ok(InertCapabilityObligationSetV1 {
        subject,
        obligations,
        identity,
        canonical_bytes,
    })
}

fn encode_result_set(
    subject: CapabilitySubjectV1,
    obligation_set: InertCapabilityObligationSetIdentityV1,
    results: Vec<CapabilityResultV1>,
) -> Result<InertCapabilityResultSetV1, CapabilityCodecErrorV1> {
    let mut body_bytes = 0_usize;
    for result in &results {
        body_bytes = body_bytes
            .checked_add(64 + outcome_encoded_len(&result.outcome)?)
            .ok_or(CapabilityCodecErrorV1::LengthOverflow)?;
    }
    let total = RESULT_PREAMBLE_BYTES_V1
        .checked_add(body_bytes)
        .and_then(|value| value.checked_add(TERMINAL_IDENTITY_BYTES_V1))
        .ok_or(CapabilityCodecErrorV1::LengthOverflow)?;
    limit(
        CapabilityResourceV1::ResultSetBytes,
        total,
        MAX_CAPABILITY_RESULT_SET_BYTES_V1,
    )?;
    let mut writer = Writer::new(total, CapabilityResourceV1::ResultSetBytes)?;
    encode_header(
        &mut writer,
        RESULT_MAGIC_V1,
        CAPABILITY_RESULT_SET_VERSION_V1,
        total,
    )?;
    encode_subject(&mut writer, subject);
    writer.digest(obligation_set.digest());
    writer.count(results.len())?;
    for result in &results {
        writer.digest(result.obligation.digest());
        writer.digest(result.identity.digest());
        encode_outcome(&mut writer, &result.outcome)?;
    }
    let identity = InertCapabilityResultSetIdentityV1::from_untrusted_digest(derive_identity(
        RESULT_SET_IDENTITY_DOMAIN_V1,
        writer.bytes(),
    ));
    writer.digest(identity.digest());
    let canonical_bytes = writer.finish(total)?;
    Ok(InertCapabilityResultSetV1 {
        subject,
        obligation_set,
        results,
        identity,
        canonical_bytes,
    })
}

fn validate_obligation_records(
    obligations: &[CapabilityObligationV1],
) -> Result<(), CapabilityCodecErrorV1> {
    for index in 1..obligations.len() {
        if obligations[index - 1].identity == obligations[index].identity {
            return Err(CapabilityCodecErrorV1::DuplicateObligation { index });
        }
        if obligations[index - 1].identity > obligations[index].identity {
            return Err(CapabilityCodecErrorV1::NonCanonicalOrder {
                record: CapabilityRecordKindV1::Obligation,
                index,
            });
        }
    }
    for index in 0..obligations.len() {
        if obligations[..index]
            .iter()
            .any(|prior| prior.property == obligations[index].property)
        {
            return Err(CapabilityCodecErrorV1::DuplicateProperty { index });
        }
    }
    Ok(())
}

fn validate_result_records(results: &[CapabilityResultV1]) -> Result<(), CapabilityCodecErrorV1> {
    for index in 1..results.len() {
        if results[index - 1].obligation == results[index].obligation {
            return Err(CapabilityCodecErrorV1::DuplicateResult { index });
        }
        if results[index - 1].obligation > results[index].obligation {
            return Err(CapabilityCodecErrorV1::NonCanonicalOrder {
                record: CapabilityRecordKindV1::Result,
                index,
            });
        }
    }
    Ok(())
}

pub(super) fn validate_subject(subject: CapabilitySubjectV1) -> Result<(), CapabilityCodecErrorV1> {
    let fields = [
        (subject.kernel.is_valid(), CapabilityIdentityFieldV1::Kernel),
        (subject.root.is_valid(), CapabilityIdentityFieldV1::Root),
        (
            subject.executable_kir.is_valid(),
            CapabilityIdentityFieldV1::ExecutableKir,
        ),
        (
            subject.target_model.is_valid(),
            CapabilityIdentityFieldV1::TargetModel,
        ),
        (
            subject.launch_contract.is_valid(),
            CapabilityIdentityFieldV1::LaunchContract,
        ),
    ];
    for (valid, field) in fields {
        if !valid {
            return Err(CapabilityCodecErrorV1::InvalidIdentity { field, index: None });
        }
    }
    if subject.executable_kir_epoch == 0 {
        return Err(CapabilityCodecErrorV1::InvalidKirEpoch);
    }
    Ok(())
}

fn validate_property(
    property: CapabilityPropertyIdV1,
    index: usize,
) -> Result<(), CapabilityCodecErrorV1> {
    if property.is_valid() {
        Ok(())
    } else {
        Err(CapabilityCodecErrorV1::InvalidStableIdentifier {
            field: CapabilityIdentityFieldV1::Property,
            index,
        })
    }
}

fn validate_diagnostic(
    diagnostic: CapabilityDiagnosticIdV1,
    index: usize,
) -> Result<(), CapabilityCodecErrorV1> {
    if diagnostic.is_valid() {
        Ok(())
    } else {
        Err(CapabilityCodecErrorV1::InvalidStableIdentifier {
            field: CapabilityIdentityFieldV1::Diagnostic,
            index,
        })
    }
}

fn validate_outcome(
    outcome: &CapabilityOutcomeV1,
    index: usize,
    aggregate_witness_bytes: &mut usize,
) -> Result<(), CapabilityCodecErrorV1> {
    match outcome {
        CapabilityOutcomeV1::Proven {
            evidence,
            tool,
            proof_artifact,
        } => {
            if !evidence.is_valid() {
                return Err(CapabilityCodecErrorV1::InvalidIdentity {
                    field: CapabilityIdentityFieldV1::Evidence,
                    index: Some(index),
                });
            }
            if !tool.is_valid() {
                return Err(CapabilityCodecErrorV1::InvalidIdentity {
                    field: CapabilityIdentityFieldV1::Tool,
                    index: Some(index),
                });
            }
            if !proof_artifact.is_valid() {
                return Err(CapabilityCodecErrorV1::InvalidIdentity {
                    field: CapabilityIdentityFieldV1::Artifact,
                    index: Some(index),
                });
            }
        }
        CapabilityOutcomeV1::Rejected {
            diagnostic,
            witness,
        } => {
            validate_diagnostic(*diagnostic, index)?;
            if witness.is_empty() {
                return Err(CapabilityCodecErrorV1::EmptyWitness { index });
            }
            limit(
                CapabilityResourceV1::RejectedWitnessBytes,
                witness.len(),
                MAX_CAPABILITY_REJECTED_WITNESS_BYTES_V1,
            )?;
            *aggregate_witness_bytes = aggregate_witness_bytes
                .checked_add(witness.len())
                .ok_or(CapabilityCodecErrorV1::LengthOverflow)?;
            limit(
                CapabilityResourceV1::AggregateWitnessBytes,
                *aggregate_witness_bytes,
                MAX_CAPABILITY_WITNESS_BYTES_V1,
            )?;
        }
        CapabilityOutcomeV1::Incomplete { diagnostic, detail }
        | CapabilityOutcomeV1::Unsupported { diagnostic, detail }
        | CapabilityOutcomeV1::Unreviewed { diagnostic, detail } => {
            validate_diagnostic(*diagnostic, index)?;
            if !detail.is_valid() {
                return Err(CapabilityCodecErrorV1::InvalidIdentity {
                    field: CapabilityIdentityFieldV1::Artifact,
                    index: Some(index),
                });
            }
        }
    }
    Ok(())
}

fn derive_obligation_identity(
    subject: CapabilitySubjectV1,
    property: CapabilityPropertyIdV1,
    statement: StatementIdentityV1,
) -> CapabilityObligationIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(OBLIGATION_IDENTITY_DOMAIN_V1);
    hash_subject(&mut digest, subject);
    hash_stable_id(
        &mut digest,
        property.namespace,
        property.schema_version,
        property.code,
    );
    digest.update(statement.digest().as_bytes());
    CapabilityObligationIdentityV1::from_untrusted_digest(DigestV1::from_untrusted_bytes(
        digest.finalize().into(),
    ))
}

fn derive_result_identity(
    subject: CapabilitySubjectV1,
    obligation_set: InertCapabilityObligationSetIdentityV1,
    obligation: CapabilityObligationIdentityV1,
    outcome: &CapabilityOutcomeV1,
) -> Result<CapabilityResultIdentityV1, CapabilityCodecErrorV1> {
    let mut writer = Writer::new(
        SUBJECT_BYTES_V1 + 64 + outcome_encoded_len(outcome)?,
        CapabilityResourceV1::ResultIdentityPreimageBytes,
    )?;
    encode_subject(&mut writer, subject);
    writer.digest(obligation_set.digest());
    writer.digest(obligation.digest());
    encode_outcome(&mut writer, outcome)?;
    Ok(CapabilityResultIdentityV1::from_untrusted_digest(
        derive_identity(RESULT_IDENTITY_DOMAIN_V1, writer.bytes()),
    ))
}

fn derive_identity(domain: &[u8], bytes: &[u8]) -> DigestV1 {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    DigestV1::from_untrusted_bytes(digest.finalize().into())
}

fn hash_subject(digest: &mut Sha256, subject: CapabilitySubjectV1) {
    digest.update(subject.kernel.digest().as_bytes());
    digest.update(subject.root.digest().as_bytes());
    digest.update(subject.executable_kir.digest().as_bytes());
    digest.update(subject.executable_kir_epoch.to_le_bytes());
    digest.update(subject.target_model.digest().as_bytes());
    digest.update(subject.launch_contract.digest().as_bytes());
}

fn hash_stable_id(digest: &mut Sha256, namespace: DigestV1, version: u16, code: u32) {
    digest.update(namespace.as_bytes());
    digest.update(version.to_le_bytes());
    digest.update(0_u16.to_le_bytes());
    digest.update(code.to_le_bytes());
}

fn encode_header(
    writer: &mut Writer,
    magic: [u8; 8],
    version: u16,
    total: usize,
) -> Result<(), CapabilityCodecErrorV1> {
    writer.raw(&magic);
    writer.u16(version);
    writer.u16(0);
    writer.u32(u32::try_from(total).map_err(|_| CapabilityCodecErrorV1::LengthOverflow)?);
    Ok(())
}

fn decode_header(
    reader: &mut Reader<'_>,
    record: CapabilityRecordKindV1,
    magic: [u8; 8],
    version: u16,
) -> Result<(), CapabilityCodecErrorV1> {
    if reader.fixed::<8>()? != magic {
        return Err(CapabilityCodecErrorV1::InvalidMagic { record });
    }
    let actual_version = reader.u16()?;
    if actual_version != version {
        return Err(CapabilityCodecErrorV1::UnsupportedVersion {
            record,
            version: actual_version,
        });
    }
    let flags = reader.u16()?;
    if flags != 0 {
        return Err(CapabilityCodecErrorV1::UnsupportedFlags { record, flags });
    }
    let declared = reader.u32()? as usize;
    if declared > reader.bytes.len() {
        return Err(CapabilityCodecErrorV1::Truncated);
    }
    if declared < reader.bytes.len() {
        return Err(CapabilityCodecErrorV1::TrailingBytes);
    }
    Ok(())
}

fn encode_subject(writer: &mut Writer, subject: CapabilitySubjectV1) {
    writer.digest(subject.kernel.digest());
    writer.digest(subject.root.digest());
    writer.digest(subject.executable_kir.digest());
    writer.u64(subject.executable_kir_epoch);
    writer.digest(subject.target_model.digest());
    writer.digest(subject.launch_contract.digest());
}

fn decode_subject(reader: &mut Reader<'_>) -> Result<CapabilitySubjectV1, CapabilityCodecErrorV1> {
    CapabilitySubjectV1::new(
        KernelIdentityV1::from_untrusted_digest(reader.digest()?),
        KernelRootIdentityV1::from_untrusted_digest(reader.digest()?),
        ExecutableKirIdentityV1::from_untrusted_digest(reader.digest()?),
        reader.u64()?,
        TargetModelIdentityV1::from_untrusted_digest(reader.digest()?),
        LaunchContractIdentityV1::from_untrusted_digest(reader.digest()?),
    )
}

fn encode_property(writer: &mut Writer, id: CapabilityPropertyIdV1) {
    writer.digest(id.namespace);
    writer.u16(id.schema_version);
    writer.u16(0);
    writer.u32(id.code);
}

fn decode_property(
    reader: &mut Reader<'_>,
    index: usize,
) -> Result<CapabilityPropertyIdV1, CapabilityCodecErrorV1> {
    let namespace = reader.digest()?;
    let version = reader.u16()?;
    if reader.u16()? != 0 {
        return Err(CapabilityCodecErrorV1::NonzeroReserved {
            record: CapabilityRecordKindV1::Obligation,
            index: Some(index),
        });
    }
    let id = CapabilityPropertyIdV1::new(namespace, version, reader.u32()?);
    validate_property(id, index)?;
    Ok(id)
}

fn decode_diagnostic(
    reader: &mut Reader<'_>,
    index: usize,
) -> Result<CapabilityDiagnosticIdV1, CapabilityCodecErrorV1> {
    let namespace = reader.digest()?;
    let version = reader.u16()?;
    if reader.u16()? != 0 {
        return Err(CapabilityCodecErrorV1::NonzeroReserved {
            record: CapabilityRecordKindV1::Result,
            index: Some(index),
        });
    }
    let id = CapabilityDiagnosticIdV1::new(namespace, version, reader.u32()?);
    validate_diagnostic(id, index)?;
    Ok(id)
}

fn outcome_encoded_len(outcome: &CapabilityOutcomeV1) -> Result<usize, CapabilityCodecErrorV1> {
    let payload = match outcome {
        CapabilityOutcomeV1::Proven { .. } => 32 + 64 + 64,
        CapabilityOutcomeV1::Rejected { witness, .. } => STABLE_ID_BYTES_V1 + 4 + witness.len(),
        CapabilityOutcomeV1::Incomplete { .. }
        | CapabilityOutcomeV1::Unsupported { .. }
        | CapabilityOutcomeV1::Unreviewed { .. } => STABLE_ID_BYTES_V1 + 64,
    };
    4_usize
        .checked_add(payload)
        .ok_or(CapabilityCodecErrorV1::LengthOverflow)
}

fn encode_outcome(
    writer: &mut Writer,
    outcome: &CapabilityOutcomeV1,
) -> Result<(), CapabilityCodecErrorV1> {
    match outcome {
        CapabilityOutcomeV1::Proven {
            evidence,
            tool,
            proof_artifact,
        } => {
            encode_outcome_header(writer, 1);
            writer.digest(evidence.digest());
            encode_tool(writer, *tool);
            encode_artifact(writer, *proof_artifact);
        }
        CapabilityOutcomeV1::Rejected {
            diagnostic,
            witness,
        } => {
            encode_outcome_header(writer, 2);
            encode_diagnostic(writer, *diagnostic);
            writer.u32(
                u32::try_from(witness.len()).map_err(|_| CapabilityCodecErrorV1::LengthOverflow)?,
            );
            writer.raw(witness);
        }
        CapabilityOutcomeV1::Incomplete { diagnostic, detail } => {
            encode_outcome_header(writer, 3);
            encode_diagnostic(writer, *diagnostic);
            encode_artifact(writer, *detail);
        }
        CapabilityOutcomeV1::Unsupported { diagnostic, detail } => {
            encode_outcome_header(writer, 4);
            encode_diagnostic(writer, *diagnostic);
            encode_artifact(writer, *detail);
        }
        CapabilityOutcomeV1::Unreviewed { diagnostic, detail } => {
            encode_outcome_header(writer, 5);
            encode_diagnostic(writer, *diagnostic);
            encode_artifact(writer, *detail);
        }
    }
    Ok(())
}

fn encode_outcome_header(writer: &mut Writer, tag: u8) {
    writer.u8(tag);
    writer.u8(0);
    writer.u16(0);
}

fn encode_diagnostic(writer: &mut Writer, id: CapabilityDiagnosticIdV1) {
    writer.digest(id.namespace);
    writer.u16(id.schema_version);
    writer.u16(0);
    writer.u32(id.code);
}

fn encode_tool(writer: &mut Writer, tool: ExactToolIdentityV1) {
    writer.digest(tool.executable);
    writer.digest(tool.configuration);
}

fn encode_artifact(writer: &mut Writer, artifact: ArtifactIdentityV1) {
    writer.digest(artifact.bytes);
    writer.digest(artifact.format);
}

fn decode_outcome(
    reader: &mut Reader<'_>,
    index: usize,
    aggregate_witness_bytes: &mut usize,
) -> Result<CapabilityOutcomeV1, CapabilityCodecErrorV1> {
    let tag = reader.u8()?;
    if reader.u8()? != 0 || reader.u16()? != 0 {
        return Err(CapabilityCodecErrorV1::NonzeroReserved {
            record: CapabilityRecordKindV1::Result,
            index: Some(index),
        });
    }
    let outcome = match tag {
        1 => CapabilityOutcomeV1::Proven {
            evidence: EvidenceIdentityV1::from_untrusted_digest(reader.digest()?),
            tool: decode_tool(reader)?,
            proof_artifact: decode_artifact(reader)?,
        },
        2 => {
            let diagnostic = decode_diagnostic(reader, index)?;
            let length = reader.bounded_length(
                CapabilityResourceV1::RejectedWitnessBytes,
                MAX_CAPABILITY_REJECTED_WITNESS_BYTES_V1,
            )?;
            *aggregate_witness_bytes = aggregate_witness_bytes
                .checked_add(length)
                .ok_or(CapabilityCodecErrorV1::LengthOverflow)?;
            limit(
                CapabilityResourceV1::AggregateWitnessBytes,
                *aggregate_witness_bytes,
                MAX_CAPABILITY_WITNESS_BYTES_V1,
            )?;
            CapabilityOutcomeV1::Rejected {
                diagnostic,
                witness: reader.owned(length, CapabilityResourceV1::RejectedWitnessBytes)?,
            }
        }
        3 => CapabilityOutcomeV1::Incomplete {
            diagnostic: decode_diagnostic(reader, index)?,
            detail: decode_artifact(reader)?,
        },
        4 => CapabilityOutcomeV1::Unsupported {
            diagnostic: decode_diagnostic(reader, index)?,
            detail: decode_artifact(reader)?,
        },
        5 => CapabilityOutcomeV1::Unreviewed {
            diagnostic: decode_diagnostic(reader, index)?,
            detail: decode_artifact(reader)?,
        },
        _ => return Err(CapabilityCodecErrorV1::UnknownOutcome { index, tag }),
    };
    let mut local_witness_bytes = 0;
    validate_outcome(&outcome, index, &mut local_witness_bytes)?;
    Ok(outcome)
}

fn decode_tool(reader: &mut Reader<'_>) -> Result<ExactToolIdentityV1, CapabilityCodecErrorV1> {
    Ok(ExactToolIdentityV1::new(reader.digest()?, reader.digest()?))
}

fn decode_artifact(reader: &mut Reader<'_>) -> Result<ArtifactIdentityV1, CapabilityCodecErrorV1> {
    Ok(ArtifactIdentityV1::new(reader.digest()?, reader.digest()?))
}

fn copy_bytes(
    bytes: &[u8],
    resource: CapabilityResourceV1,
) -> Result<Vec<u8>, CapabilityCodecErrorV1> {
    let mut copy = reserve_vec(bytes.len(), resource)?;
    copy.extend_from_slice(bytes);
    Ok(copy)
}

fn reserve_vec<T>(
    count: usize,
    resource: CapabilityResourceV1,
) -> Result<Vec<T>, CapabilityCodecErrorV1> {
    let _ = count
        .checked_mul(size_of::<T>())
        .ok_or(CapabilityCodecErrorV1::LengthOverflow)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| CapabilityCodecErrorV1::AllocationFailure { resource })?;
    Ok(values)
}

fn limit(
    resource: CapabilityResourceV1,
    actual: usize,
    maximum: usize,
) -> Result<(), CapabilityCodecErrorV1> {
    if actual > maximum {
        Err(CapabilityCodecErrorV1::LimitExceeded {
            resource,
            actual,
            maximum,
        })
    } else {
        Ok(())
    }
}

struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn new(
        capacity: usize,
        resource: CapabilityResourceV1,
    ) -> Result<Self, CapabilityCodecErrorV1> {
        Ok(Self {
            bytes: reserve_vec(capacity, resource)?,
        })
    }

    fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn finish(self, expected: usize) -> Result<Vec<u8>, CapabilityCodecErrorV1> {
        if self.bytes.len() == expected {
            Ok(self.bytes)
        } else {
            Err(CapabilityCodecErrorV1::LengthOverflow)
        }
    }

    fn raw(&mut self, bytes: &[u8]) {
        self.bytes.extend_from_slice(bytes);
    }

    fn digest(&mut self, digest: DigestV1) {
        self.raw(digest.as_bytes());
    }

    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.raw(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.raw(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.raw(&value.to_le_bytes());
    }

    fn count(&mut self, value: usize) -> Result<(), CapabilityCodecErrorV1> {
        self.u32(u32::try_from(value).map_err(|_| CapabilityCodecErrorV1::LengthOverflow)?);
        Ok(())
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    const fn offset(&self) -> usize {
        self.offset
    }

    fn finish(&self) -> Result<(), CapabilityCodecErrorV1> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            Err(CapabilityCodecErrorV1::TrailingBytes)
        }
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], CapabilityCodecErrorV1> {
        let end = self
            .offset
            .checked_add(N)
            .ok_or(CapabilityCodecErrorV1::LengthOverflow)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(CapabilityCodecErrorV1::Truncated)?;
        self.offset = end;
        bytes
            .try_into()
            .map_err(|_| CapabilityCodecErrorV1::Truncated)
    }

    fn digest(&mut self) -> Result<DigestV1, CapabilityCodecErrorV1> {
        Ok(DigestV1::from_untrusted_bytes(self.fixed()?))
    }

    fn u8(&mut self) -> Result<u8, CapabilityCodecErrorV1> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, CapabilityCodecErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, CapabilityCodecErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, CapabilityCodecErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn count(
        &mut self,
        resource: CapabilityResourceV1,
        maximum: usize,
    ) -> Result<usize, CapabilityCodecErrorV1> {
        let count = self.u32()? as usize;
        limit(resource, count, maximum)?;
        Ok(count)
    }

    fn bounded_length(
        &mut self,
        resource: CapabilityResourceV1,
        maximum: usize,
    ) -> Result<usize, CapabilityCodecErrorV1> {
        let length = self.u32()? as usize;
        limit(resource, length, maximum)?;
        Ok(length)
    }

    fn require_bytes(
        &self,
        count: usize,
        per_record: usize,
        terminal: usize,
    ) -> Result<(), CapabilityCodecErrorV1> {
        let minimum = count
            .checked_mul(per_record)
            .and_then(|value| value.checked_add(terminal))
            .ok_or(CapabilityCodecErrorV1::LengthOverflow)?;
        if self.bytes.len().saturating_sub(self.offset) < minimum {
            Err(CapabilityCodecErrorV1::Truncated)
        } else {
            Ok(())
        }
    }

    fn owned(
        &mut self,
        length: usize,
        resource: CapabilityResourceV1,
    ) -> Result<Vec<u8>, CapabilityCodecErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(CapabilityCodecErrorV1::LengthOverflow)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(CapabilityCodecErrorV1::Truncated)?;
        self.offset = end;
        copy_bytes(bytes, resource)
    }
}
