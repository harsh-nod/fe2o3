//! Fixed framing shared by nominal publication families; no authority or I/O.
use crate::receipt_publication::{
    CompilerExecutionReceiptPublicationErrorV1 as Error, Reader, decode_header_version,
    derive_identity, encode_header_version, put, require_identity, require_length,
};

pub(crate) const PUBLICATION_BYTES: usize = 584;
pub(crate) const ACK_BYTES: usize = 288;
pub(crate) const CARRIAGE_BYTES: usize = 2090;
pub(crate) struct Schema {
    version: u16,
    publication: Wire,
    ack: Wire,
    carriage: Wire,
}
struct Wire {
    magic: [u8; 8],
    domain: &'static [u8],
}
pub(crate) const V1: Schema = Schema {
    version: 1,
    publication: Wire {
        magic: *b"F2O3CES1",
        domain: b"FE2O3/COMPILER-EXECUTION-RECEIPT-PUBLICATION/V1\0",
    },
    ack: Wire {
        magic: *b"F2O3CEA1",
        domain: b"FE2O3/COMPILER-EXECUTION-RECEIPT-PUBLICATION-ACK/V1\0",
    },
    carriage: Wire {
        magic: *b"F2O3CRG1",
        domain: b"FE2O3/COMPILER-EXECUTION-RECEIPT-CARRIAGE/V1\0",
    },
};
pub(crate) const V2: Schema = Schema {
    version: 2,
    publication: Wire {
        magic: *b"F2O3CES2",
        domain: b"FE2O3/COMPILER-EXECUTION-RECEIPT-PUBLICATION/V2\0",
    },
    ack: Wire {
        magic: *b"F2O3CEA2",
        domain: b"FE2O3/COMPILER-EXECUTION-RECEIPT-PUBLICATION-ACK/V2\0",
    },
    carriage: Wire {
        magic: *b"F2O3CRG2",
        domain: b"FE2O3/COMPILER-EXECUTION-RECEIPT-CARRIAGE/V2\0",
    },
};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct Bindings {
    pub policy: [u8; 32],
    pub journal: [u8; 32],
    pub occurrence: [u8; 32],
    pub receipt: [u8; 32],
}
impl Bindings {
    pub fn matches(self, expected: Self) -> Result<(), Error> {
        if self.policy != expected.policy {
            return Err(Error::PolicyMismatch);
        }
        if self.journal != expected.journal {
            return Err(Error::IssuerJournalMismatch);
        }
        if self.occurrence != expected.occurrence {
            return Err(Error::OccurrenceMismatch);
        }
        if self.receipt != expected.receipt {
            return Err(Error::ReceiptMismatch);
        }
        Ok(())
    }
    fn encode(self, bytes: &mut [u8], offset: &mut usize) {
        for field in [self.policy, self.journal, self.occurrence, self.receipt] {
            put(bytes, offset, &field);
        }
    }
    fn decode(reader: &mut Reader<'_>) -> Result<Self, Error> {
        Ok(Self {
            policy: reader.fixed()?,
            journal: reader.fixed()?,
            occurrence: reader.fixed()?,
            receipt: reader.fixed()?,
        })
    }
}
#[derive(Clone, Eq, PartialEq)]
pub(crate) struct Record<const N: usize> {
    pub identity: [u8; 32],
    pub bytes: [u8; N],
}
#[derive(Clone, Eq, PartialEq)]
pub(crate) struct Publication {
    pub bindings: Bindings,
    pub wire: Record<PUBLICATION_BYTES>,
}
pub(crate) struct PublicationParts<'a> {
    pub bindings: Bindings,
    pub receipt: &'a [u8],
    identity: [u8; 32],
}
#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct AckFields {
    pub bindings: Bindings,
    pub publication: [u8; 32],
    pub worker: [u8; 32],
    pub sequence: u64,
    pub current: [u8; 32],
}
#[derive(Clone, Eq, PartialEq)]
pub(crate) struct Ack {
    pub fields: AckFields,
    pub wire: Record<ACK_BYTES>,
}
pub(crate) struct CarriageParts<'a> {
    pub policy: &'a [u8],
    pub request: &'a [u8],
    pub publication: &'a [u8],
    pub ack: &'a [u8],
    pub identity: [u8; 32],
}

impl Schema {
    pub fn publication(
        &self,
        bindings: Bindings,
        receipt: &[u8; 400],
    ) -> Result<Publication, Error> {
        require_identity(bindings.journal, "issuer journal")?;
        require_identity(bindings.occurrence, "compiler occurrence")?;
        let mut bytes = [0; PUBLICATION_BYTES];
        let mut offset = encode_header_version(&mut bytes, self.publication.magic, self.version);
        bindings.encode(&mut bytes, &mut offset);
        put(&mut bytes, &mut offset, receipt);
        Ok(Publication {
            bindings,
            wire: self.publication.finish(bytes, offset),
        })
    }
    pub fn publication_parts<'a>(&self, bytes: &'a [u8]) -> Result<PublicationParts<'a>, Error> {
        let mut reader = self.publication.reader(
            bytes,
            self.version,
            PUBLICATION_BYTES,
            "receipt publication",
        )?;
        let bindings = Bindings::decode(&mut reader)?;
        let receipt = reader.take(400)?;
        let identity = reader.fixed()?;
        Ok(PublicationParts {
            bindings,
            receipt,
            identity,
        })
    }
    /// Call only after the nested receipt decoder; preserve nested-first errors.
    pub fn finish_publication(
        &self,
        parts: PublicationParts<'_>,
        policy: [u8; 32],
        receipt_identity: [u8; 32],
        receipt: &[u8; 400],
        bytes: &[u8],
    ) -> Result<Publication, Error> {
        require_identity(parts.bindings.journal, "issuer journal")?;
        require_identity(parts.bindings.occurrence, "compiler occurrence")?;
        if parts.bindings.policy != policy {
            return Err(Error::PolicyMismatch);
        }
        if parts.bindings.receipt != receipt_identity {
            return Err(Error::ReceiptMismatch);
        }
        let record = self.publication(parts.bindings, receipt)?;
        record
            .wire
            .check(parts.identity, bytes, "receipt publication")?;
        Ok(record)
    }
    pub fn ack(&self, fields: AckFields) -> Result<Ack, Error> {
        require_identity(fields.worker, "Worker ledger record")?;
        if fields.sequence == 0 {
            return Err(Error::ZeroValue("receipt sequence"));
        }
        require_identity(fields.current, "current rollback anchor")?;
        let mut bytes = [0; ACK_BYTES];
        let mut offset = encode_header_version(&mut bytes, self.ack.magic, self.version);
        fields.bindings.encode(&mut bytes, &mut offset);
        put(&mut bytes, &mut offset, &fields.publication);
        put(&mut bytes, &mut offset, &fields.worker);
        put(&mut bytes, &mut offset, &fields.sequence.to_le_bytes());
        put(&mut bytes, &mut offset, &fields.current);
        Ok(Ack {
            fields,
            wire: self.ack.finish(bytes, offset),
        })
    }
    pub fn decode_ack(&self, bytes: &[u8]) -> Result<Ack, Error> {
        let mut reader = self.ack.reader(
            bytes,
            self.version,
            ACK_BYTES,
            "receipt publication acknowledgment",
        )?;
        let fields = AckFields {
            bindings: Bindings::decode(&mut reader)?,
            publication: reader.fixed()?,
            worker: reader.fixed()?,
            sequence: reader.u64()?,
            current: reader.fixed()?,
        };
        let identity = reader.fixed()?;
        for (value, label) in [
            (fields.bindings.policy, "issuer policy"),
            (fields.bindings.journal, "issuer journal"),
            (fields.bindings.occurrence, "compiler occurrence"),
            (fields.bindings.receipt, "receipt"),
            (fields.publication, "receipt publication"),
        ] {
            require_identity(value, label)?;
        }
        let record = self.ack(fields)?;
        record
            .wire
            .check(identity, bytes, "receipt publication acknowledgment")?;
        Ok(record)
    }
    pub fn carriage(
        &self,
        policy: &[u8; 216],
        request: &[u8; 946],
        publication: &[u8; PUBLICATION_BYTES],
        ack: &[u8; ACK_BYTES],
    ) -> Record<CARRIAGE_BYTES> {
        let mut bytes = [0; CARRIAGE_BYTES];
        let mut offset = encode_header_version(&mut bytes, self.carriage.magic, self.version);
        for part in [
            policy.as_slice(),
            request.as_slice(),
            publication.as_slice(),
            ack.as_slice(),
        ] {
            put(&mut bytes, &mut offset, part);
        }
        self.carriage.finish(bytes, offset)
    }
    pub fn carriage_parts<'a>(&self, bytes: &'a [u8]) -> Result<CarriageParts<'a>, Error> {
        let mut reader = self.carriage.reader(
            bytes,
            self.version,
            CARRIAGE_BYTES,
            "compiler receipt carriage",
        )?;
        Ok(CarriageParts {
            policy: reader.take(216)?,
            request: reader.take(946)?,
            publication: reader.take(PUBLICATION_BYTES)?,
            ack: reader.take(ACK_BYTES)?,
            identity: reader.fixed()?,
        })
    }
    pub fn matches_publication(&self, id: [u8; 32], bytes: &[u8]) -> bool {
        self.publication.matches(id, bytes, PUBLICATION_BYTES)
    }
    pub fn matches_ack(&self, id: [u8; 32], bytes: &[u8]) -> bool {
        self.ack.matches(id, bytes, ACK_BYTES)
    }
    pub fn matches_carriage(&self, id: [u8; 32], bytes: &[u8]) -> bool {
        self.carriage.matches(id, bytes, CARRIAGE_BYTES)
    }
}
impl Ack {
    pub fn matches(
        &self,
        publication: &Publication,
        sequence: u64,
        current: [u8; 32],
    ) -> Result<(), Error> {
        self.fields.bindings.matches(publication.bindings)?;
        if self.fields.publication != publication.wire.identity {
            return Err(Error::PublicationMismatch);
        }
        if self.fields.sequence != sequence {
            return Err(Error::SequenceMismatch);
        }
        if self.fields.current != current {
            return Err(Error::RollbackAnchorMismatch);
        }
        Ok(())
    }
    pub fn matches_worker(&self, worker: [u8; 32]) -> Result<(), Error> {
        if self.fields.worker == worker {
            Ok(())
        } else {
            Err(Error::WorkerLedgerMismatch)
        }
    }
}
impl<const N: usize> Record<N> {
    pub fn check(&self, id: [u8; 32], bytes: &[u8], field: &'static str) -> Result<(), Error> {
        if self.identity != id || self.bytes.as_slice() != bytes {
            Err(Error::IdentityMismatch(field))
        } else {
            Ok(())
        }
    }
}
impl Wire {
    fn reader<'a>(
        &self,
        bytes: &'a [u8],
        version: u16,
        len: usize,
        label: &'static str,
    ) -> Result<Reader<'a>, Error> {
        require_length(bytes, len, label)?;
        let mut reader = Reader::new(bytes);
        decode_header_version(&mut reader, self.magic, version, len, label)?;
        Ok(reader)
    }
    fn finish<const N: usize>(&self, mut bytes: [u8; N], offset: usize) -> Record<N> {
        debug_assert_eq!(offset, N - 32);
        let identity = derive_identity(self.domain, &bytes[..offset]);
        bytes[offset..].copy_from_slice(&identity);
        Record { identity, bytes }
    }
    fn matches(&self, id: [u8; 32], bytes: &[u8], length: usize) -> bool {
        bytes.len() == length
            && bytes[length - 32..] == id
            && derive_identity(self.domain, &bytes[..length - 32]) == id
    }
}
