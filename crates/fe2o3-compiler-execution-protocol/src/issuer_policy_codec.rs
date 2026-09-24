//! Fixed policy mechanics shared by nominal, version-specific public adapters.
use crate::attestation::{
    CompilerExecutionAttestationErrorV1 as Error,
    CompilerExecutionIssuerMeasurementV1 as Measurement, Reader, decode_header_version,
    decode_measurement, derive_identity, encode_header_version, encode_measurement, put,
    require_identity, require_length, validate_verifying_key,
};

pub(crate) const BYTES: usize = 216;
pub(crate) const PREIMAGE_BYTES: usize = BYTES - 32;

pub(crate) struct Schema {
    magic: [u8; 8],
    version: u16,
    subject_version: u16,
    domain: &'static [u8],
}

pub(crate) const V1: Schema = Schema {
    magic: *b"F2O3CEP1",
    version: 1,
    subject_version: fe2o3_artifact_transaction::INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1,
    domain: b"FE2O3/COMPILER-EXECUTION-ISSUER-POLICY/V1\0",
};
pub(crate) const V2: Schema = Schema {
    magic: *b"F2O3CEP2",
    version: 2,
    subject_version: fe2o3_artifact_transaction::INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V2,
    domain: b"FE2O3/COMPILER-EXECUTION-ISSUER-POLICY/V2\0",
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Fields {
    pub generation: u64,
    pub executable: Measurement,
    pub runtime: Measurement,
    pub verifying_key: [u8; 32],
    pub external_anchor_verifying_key: [u8; 32],
}

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct Record {
    pub fields: Fields,
    pub identity: [u8; 32],
    pub bytes: [u8; BYTES],
}

impl Schema {
    pub fn encode(&self, fields: Fields) -> Result<Record, Error> {
        if fields.generation == 0 {
            return Err(Error::ZeroValue("issuer policy generation"));
        }
        validate_verifying_key(fields.verifying_key)?;
        validate_verifying_key(fields.external_anchor_verifying_key)?;
        if fields.external_anchor_verifying_key == fields.verifying_key {
            return Err(Error::NonDistinctVerifyingKeys);
        }
        let mut bytes = [0; BYTES];
        let mut offset = encode_header_version(&mut bytes, self.magic, self.version);
        put(&mut bytes, &mut offset, &fields.generation.to_le_bytes());
        encode_measurement(&mut bytes, &mut offset, fields.executable);
        encode_measurement(&mut bytes, &mut offset, fields.runtime);
        put(&mut bytes, &mut offset, &fields.verifying_key);
        put(
            &mut bytes,
            &mut offset,
            &fields.external_anchor_verifying_key,
        );
        put(&mut bytes, &mut offset, &self.subject_version.to_le_bytes());
        offset += 6;
        debug_assert_eq!(offset, PREIMAGE_BYTES);
        let identity = derive_identity(self.domain, &bytes[..PREIMAGE_BYTES]);
        put(&mut bytes, &mut offset, &identity);
        debug_assert_eq!(offset, BYTES);
        Ok(Record {
            fields,
            identity,
            bytes,
        })
    }

    // Retain the V1 diagnostic order, including reserved/declared-ID checks
    // before generation/key validation. Version selection is never length-based.
    pub fn decode(&self, bytes: &[u8]) -> Result<Record, Error> {
        require_length(bytes, BYTES, "issuer policy")?;
        let mut reader = Reader::new(bytes);
        decode_header_version(
            &mut reader,
            self.magic,
            self.version,
            BYTES,
            "issuer policy",
        )?;
        let fields = Fields {
            generation: reader.u64()?,
            executable: decode_measurement(&mut reader, "issuer executable")?,
            runtime: decode_measurement(&mut reader, "issuer runtime")?,
            verifying_key: reader.fixed()?,
            external_anchor_verifying_key: reader.fixed()?,
        };
        let subject_version = reader.u16()?;
        if subject_version != self.subject_version {
            return Err(Error::UnsupportedSubjectVersion(subject_version));
        }
        if reader.fixed::<6>()? != [0; 6] {
            return Err(Error::NonzeroReserved);
        }
        let declared_identity = reader.fixed()?;
        require_identity(declared_identity, "issuer policy")?;
        let decoded = self.encode(fields)?;
        if decoded.identity != declared_identity || decoded.bytes.as_slice() != bytes {
            return Err(Error::IdentityMismatch("issuer policy"));
        }
        Ok(decoded)
    }

    pub fn matches(&self, identity: [u8; 32], bytes: &[u8]) -> bool {
        bytes.len() == BYTES
            && bytes[PREIMAGE_BYTES..] == identity
            && derive_identity(self.domain, &bytes[..PREIMAGE_BYTES]) == identity
    }
}
