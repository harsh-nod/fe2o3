/// Exact canonical Kernel IR wire version retained by production lowering.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ProductionCanonicalKernelIrVersionV1 {
    /// Exact canonical Kernel IR V8.
    V8,
    /// Exact canonical Kernel IR V9.
    V9,
}

/// Version-bound identity of canonical Kernel IR bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductionCanonicalKernelIrIdentityV1 {
    version: ProductionCanonicalKernelIrVersionV1,
    digest: [u8; 32],
    canonical_length: u64,
}

impl ProductionCanonicalKernelIrIdentityV1 {
    /// Constructs an identity from an exact wire version, digest, and byte length.
    pub const fn from_canonical_parts(
        version: ProductionCanonicalKernelIrVersionV1,
        digest: [u8; 32],
        canonical_length: u64,
    ) -> Self {
        Self {
            version,
            digest,
            canonical_length,
        }
    }

    /// Returns the exact canonical wire version committed by this identity.
    pub const fn version(&self) -> ProductionCanonicalKernelIrVersionV1 {
        self.version
    }

    /// Returns the version-domain-separated SHA-256 digest.
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    /// Returns the exact retained canonical byte length.
    pub const fn canonical_length(&self) -> u64 {
        self.canonical_length
    }
}
