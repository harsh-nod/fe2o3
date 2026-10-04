//! Domain-separated identities supplied by an external authenticated producer.

/// Width of every opaque V1 commitment.
pub const DIGEST_BYTES_V1: usize = 32;

/// An opaque caller-supplied commitment.
///
/// Construction does not calculate or authenticate a hash. All-zero values are
/// rejected by contract-set validation so absent identities cannot look exact.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct DigestV1([u8; DIGEST_BYTES_V1]);

impl DigestV1 {
    pub const ZERO: Self = Self([0; DIGEST_BYTES_V1]);

    pub const fn from_untrusted_bytes(bytes: [u8; DIGEST_BYTES_V1]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(&self) -> &[u8; DIGEST_BYTES_V1] {
        &self.0
    }

    pub const fn is_zero(self) -> bool {
        let mut index = 0;
        while index < DIGEST_BYTES_V1 {
            if self.0[index] != 0 {
                return false;
            }
            index += 1;
        }
        true
    }
}

macro_rules! opaque_identity {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name(DigestV1);

        impl $name {
            pub const fn from_untrusted_digest(digest: DigestV1) -> Self {
                Self(digest)
            }

            pub const fn digest(self) -> DigestV1 {
                self.0
            }

            pub(crate) const fn is_valid(self) -> bool {
                !self.0.is_zero()
            }
        }
    };
}

opaque_identity!(
    /// Identity of one property without implying relationships to other properties.
    PropertyIdentityV1
);
opaque_identity!(
    /// Identity of the exact statement attached to a property.
    StatementIdentityV1
);
opaque_identity!(
    /// Identity of one evidence record.
    EvidenceIdentityV1
);
opaque_identity!(
    /// Identity of one explicit proof or checking obligation.
    ObligationIdentityV1
);
opaque_identity!(
    /// Identity of one trusted-computing-base entry.
    TcbEntryIdentityV1
);
opaque_identity!(
    /// Identity of one proof-erasure or semantic-correspondence reference.
    CorrespondenceIdentityV1
);

/// Exact input bytes and the schema used to interpret them.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExactInputIdentityV1 {
    pub bytes: DigestV1,
    pub interpretation: DigestV1,
}

impl ExactInputIdentityV1 {
    pub const fn new(bytes: DigestV1, interpretation: DigestV1) -> Self {
        Self {
            bytes,
            interpretation,
        }
    }

    pub(crate) const fn is_valid(self) -> bool {
        !self.bytes.is_zero() && !self.interpretation.is_zero()
    }
}

/// Exact formal or executable model and its explicit assumptions.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExactModelIdentityV1 {
    pub semantics: DigestV1,
    pub assumptions: DigestV1,
}

impl ExactModelIdentityV1 {
    pub const fn new(semantics: DigestV1, assumptions: DigestV1) -> Self {
        Self {
            semantics,
            assumptions,
        }
    }

    pub(crate) const fn is_valid(self) -> bool {
        !self.semantics.is_zero() && !self.assumptions.is_zero()
    }
}

/// Exact tool executable and complete authority-relevant configuration.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ExactToolIdentityV1 {
    pub executable: DigestV1,
    pub configuration: DigestV1,
}

impl ExactToolIdentityV1 {
    pub const fn new(executable: DigestV1, configuration: DigestV1) -> Self {
        Self {
            executable,
            configuration,
        }
    }

    pub(crate) const fn is_valid(self) -> bool {
        !self.executable.is_zero() && !self.configuration.is_zero()
    }
}

/// Exact artifact bytes and the format/schema needed to interpret them.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ArtifactIdentityV1 {
    pub bytes: DigestV1,
    pub format: DigestV1,
}

impl ArtifactIdentityV1 {
    pub const fn new(bytes: DigestV1, format: DigestV1) -> Self {
        Self { bytes, format }
    }

    pub(crate) const fn is_valid(self) -> bool {
        !self.bytes.is_zero() && !self.format.is_zero()
    }
}
