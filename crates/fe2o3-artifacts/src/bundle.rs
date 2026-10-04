use std::fmt;

use crate::{
    ArtifactContainerV1, Capability, CodeObjectFormat, DigestAlgorithm, DigestBytes,
    MAX_CODE_OBJECT_BYTES, ManifestV1, Name, TargetIdentity,
};

pub const MAX_BUNDLE_TARGET_ASSOCIATIONS: usize = 128;
pub const MAX_BUNDLE_PAYLOAD_REFERENCES: usize = 1024;
pub const MAX_BUNDLE_KERNELS: usize = 1024;
pub const MAX_KERNEL_PAYLOAD_REFERENCES: usize = 16;
pub const BUNDLE_INDEX_DIGEST_ALGORITHM: DigestAlgorithm = DigestAlgorithm::Sha256;

/// A content-addressed manifest and the target declared by that manifest.
///
/// This is an index association, not evidence that the manifest was produced by
/// a trusted compiler or that the target is compatible with an observed device.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleTargetAssociationV1 {
    manifest_digest: DigestBytes,
    target: TargetIdentity,
}

impl BundleTargetAssociationV1 {
    pub const fn new(manifest_digest: DigestBytes, target: TargetIdentity) -> Self {
        Self {
            manifest_digest,
            target,
        }
    }

    pub fn from_manifest(manifest: &ManifestV1) -> Self {
        Self::new(
            BUNDLE_INDEX_DIGEST_ALGORITHM
                .calculate(&manifest.to_bytes())
                .bytes(),
            manifest.target().clone(),
        )
    }

    pub const fn manifest_digest(&self) -> DigestBytes {
        self.manifest_digest
    }

    pub const fn target(&self) -> &TargetIdentity {
        &self.target
    }
}

/// A content-addressed payload known to the bundle index.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundlePayloadReferenceV1 {
    digest: DigestBytes,
    format: CodeObjectFormat,
    byte_len: u64,
}

impl BundlePayloadReferenceV1 {
    pub fn new(
        digest: DigestBytes,
        format: CodeObjectFormat,
        byte_len: u64,
    ) -> Result<Self, BundleValidationError> {
        if byte_len == 0 || byte_len > MAX_CODE_OBJECT_BYTES as u64 {
            return Err(BundleValidationError::InvalidPayloadLength {
                digest,
                max: MAX_CODE_OBJECT_BYTES,
            });
        }
        Ok(Self {
            digest,
            format,
            byte_len,
        })
    }

    pub const fn digest(&self) -> DigestBytes {
        self.digest
    }

    pub const fn format(&self) -> CodeObjectFormat {
        self.format
    }

    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }
}

/// One kernel's stable index entry and its descriptive artifact references.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleKernelIndexEntryV1 {
    kernel_id: DigestBytes,
    symbol: Name,
    manifest_digest: DigestBytes,
    payload_digests: Vec<DigestBytes>,
}

impl BundleKernelIndexEntryV1 {
    pub fn new(
        kernel_id: DigestBytes,
        symbol: Name,
        manifest_digest: DigestBytes,
        mut payload_digests: Vec<DigestBytes>,
    ) -> Result<Self, BundleValidationError> {
        require_count(
            payload_digests.len(),
            "kernel payload references",
            MAX_KERNEL_PAYLOAD_REFERENCES,
        )?;
        payload_digests.sort_unstable();
        if payload_digests.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(BundleValidationError::Duplicate {
                field: "kernel payload reference",
            });
        }
        Ok(Self {
            kernel_id,
            symbol,
            manifest_digest,
            payload_digests,
        })
    }

    pub const fn kernel_id(&self) -> DigestBytes {
        self.kernel_id
    }

    pub const fn symbol(&self) -> &Name {
        &self.symbol
    }

    pub const fn manifest_digest(&self) -> DigestBytes {
        self.manifest_digest
    }

    pub fn payload_digests(&self) -> &[DigestBytes] {
        &self.payload_digests
    }
}

/// Canonical multi-kernel index over existing manifest and payload identities.
///
/// The index deliberately contains no payload bytes and grants no runtime
/// authority. A consumer must still obtain and validate the referenced
/// container, payload, target, ABI, and any required proof evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleIndexV1 {
    target_associations: Vec<BundleTargetAssociationV1>,
    payloads: Vec<BundlePayloadReferenceV1>,
    kernels: Vec<BundleKernelIndexEntryV1>,
}

impl BundleIndexV1 {
    pub fn new(
        mut target_associations: Vec<BundleTargetAssociationV1>,
        mut payloads: Vec<BundlePayloadReferenceV1>,
        mut kernels: Vec<BundleKernelIndexEntryV1>,
    ) -> Result<Self, BundleValidationError> {
        require_count(
            target_associations.len(),
            "bundle target associations",
            MAX_BUNDLE_TARGET_ASSOCIATIONS,
        )?;
        require_count(
            payloads.len(),
            "bundle payload references",
            MAX_BUNDLE_PAYLOAD_REFERENCES,
        )?;
        require_count(kernels.len(), "bundle kernels", MAX_BUNDLE_KERNELS)?;

        target_associations.sort_unstable_by_key(BundleTargetAssociationV1::manifest_digest);
        reject_duplicate_by(
            &target_associations,
            BundleTargetAssociationV1::manifest_digest,
            "bundle manifest digest",
        )?;

        payloads.sort_unstable_by_key(BundlePayloadReferenceV1::digest);
        reject_duplicate_by(
            &payloads,
            BundlePayloadReferenceV1::digest,
            "bundle payload digest",
        )?;

        kernels.sort_unstable_by_key(BundleKernelIndexEntryV1::kernel_id);
        reject_duplicate_by(
            &kernels,
            BundleKernelIndexEntryV1::kernel_id,
            "bundle kernel ID",
        )?;
        reject_duplicate_symbols(&kernels)?;

        for kernel in &kernels {
            if target_associations
                .binary_search_by_key(
                    &kernel.manifest_digest(),
                    BundleTargetAssociationV1::manifest_digest,
                )
                .is_err()
            {
                return Err(BundleValidationError::MissingTargetAssociation(
                    kernel.manifest_digest(),
                ));
            }
            for digest in kernel.payload_digests() {
                if payloads
                    .binary_search_by_key(digest, BundlePayloadReferenceV1::digest)
                    .is_err()
                {
                    return Err(BundleValidationError::MissingPayload(*digest));
                }
            }
        }

        Ok(Self {
            target_associations,
            payloads,
            kernels,
        })
    }

    /// Builds an index from already validated v1 containers.
    ///
    /// This derives references from each container's manifest closure but does
    /// not retain payload bytes or elevate the resulting index to runtime
    /// authority.
    pub fn from_containers(
        containers: &[ArtifactContainerV1],
    ) -> Result<Self, BundleValidationError> {
        require_count(
            containers.len(),
            "artifact containers",
            MAX_BUNDLE_TARGET_ASSOCIATIONS,
        )?;

        let mut target_associations = Vec::with_capacity(containers.len());
        let mut payloads = Vec::new();
        let mut kernels = Vec::new();
        let mut logical_names = Vec::new();

        for container in containers {
            let manifest = container.manifest();
            let association = BundleTargetAssociationV1::from_manifest(manifest);
            let manifest_digest = association.manifest_digest();
            target_associations.push(association);

            payloads.extend(manifest.code_objects().iter().map(|code_object| {
                BundlePayloadReferenceV1::new(
                    code_object.digest(),
                    code_object.format(),
                    code_object.byte_len(),
                )
                .expect("validated container payload length must satisfy bundle bounds")
            }));
            for kernel in manifest.kernels() {
                logical_names.push(kernel.name());
                kernels.push(BundleKernelIndexEntryV1::new(
                    kernel.kernel_id(),
                    kernel.symbol().clone(),
                    manifest_digest,
                    vec![kernel.code_object_digest()],
                )?);
            }
        }

        logical_names.sort_unstable();
        reject_duplicate_values(&logical_names, "bundle kernel logical name")?;
        deduplicate_identical_payloads(&mut payloads)?;
        Self::new(target_associations, payloads, kernels)
    }

    /// Selects one descriptive kernel reference under an exact declared profile.
    ///
    /// `kernel_id` is the stable logical identity. Requiring its export symbol as
    /// well prevents a caller from silently accepting a renamed entry. The
    /// payload profile is set-like: order is ignored, while digest membership,
    /// format, and byte length must match exactly. Candidate target capabilities
    /// may be a superset of the indexed target declaration.
    ///
    /// This only returns index metadata. It does not authenticate or return
    /// payload bytes, inspect a device, or grant load or launch authority.
    pub fn select_kernel_reference(
        &self,
        kernel_id: DigestBytes,
        expected_export_symbol: &Name,
        expected_payload_profile: &[BundlePayloadReferenceV1],
        candidate_target: &TargetIdentity,
    ) -> Result<&BundleKernelIndexEntryV1, BundleValidationError> {
        require_count(
            expected_payload_profile.len(),
            "selection payload profile",
            MAX_KERNEL_PAYLOAD_REFERENCES,
        )?;

        let mut expected_payload_profile = expected_payload_profile.to_vec();
        expected_payload_profile.sort_unstable_by_key(BundlePayloadReferenceV1::digest);
        reject_duplicate_by(
            &expected_payload_profile,
            BundlePayloadReferenceV1::digest,
            "selection payload digest",
        )?;

        let kernel = self
            .kernels
            .binary_search_by_key(&kernel_id, BundleKernelIndexEntryV1::kernel_id)
            .ok()
            .map(|index| &self.kernels[index])
            .ok_or(BundleValidationError::UnknownKernel(kernel_id))?;
        if kernel.symbol() != expected_export_symbol {
            return Err(BundleValidationError::ExportSymbolMismatch { kernel_id });
        }

        if !kernel
            .payload_digests()
            .iter()
            .copied()
            .eq(expected_payload_profile
                .iter()
                .map(BundlePayloadReferenceV1::digest))
        {
            return Err(BundleValidationError::PayloadMembershipMismatch { kernel_id });
        }

        for expected in &expected_payload_profile {
            let indexed = self
                .payloads
                .binary_search_by_key(&expected.digest(), BundlePayloadReferenceV1::digest)
                .ok()
                .map(|index| &self.payloads[index])
                .expect("validated kernel references must resolve to bundle payloads");
            if indexed.format() != expected.format() || indexed.byte_len() != expected.byte_len() {
                return Err(BundleValidationError::PayloadProfileMismatch(
                    expected.digest(),
                ));
            }
        }

        let association = self
            .target_associations
            .binary_search_by_key(
                &kernel.manifest_digest(),
                BundleTargetAssociationV1::manifest_digest,
            )
            .ok()
            .map(|index| &self.target_associations[index])
            .expect("validated kernel references must resolve to a target association");
        check_target_profile(
            association.manifest_digest(),
            association.target(),
            candidate_target,
        )?;

        Ok(kernel)
    }

    pub fn target_associations(&self) -> &[BundleTargetAssociationV1] {
        &self.target_associations
    }

    pub fn payloads(&self) -> &[BundlePayloadReferenceV1] {
        &self.payloads
    }

    pub fn kernels(&self) -> &[BundleKernelIndexEntryV1] {
        &self.kernels
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum BundleValidationError {
    EmptyCollection {
        field: &'static str,
    },
    TooMany {
        field: &'static str,
        max: usize,
    },
    Duplicate {
        field: &'static str,
    },
    InvalidPayloadLength {
        digest: DigestBytes,
        max: usize,
    },
    ConflictingPayload(DigestBytes),
    MissingTargetAssociation(DigestBytes),
    MissingPayload(DigestBytes),
    UnknownKernel(DigestBytes),
    ExportSymbolMismatch {
        kernel_id: DigestBytes,
    },
    PayloadMembershipMismatch {
        kernel_id: DigestBytes,
    },
    PayloadProfileMismatch(DigestBytes),
    TargetProfileMismatch {
        manifest_digest: DigestBytes,
        field: &'static str,
    },
    MissingTargetCapability {
        manifest_digest: DigestBytes,
        capability: Capability,
    },
}

impl fmt::Display for BundleValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyCollection { field } => write!(formatter, "{field} must not be empty"),
            Self::TooMany { field, max } => write!(formatter, "{field} exceeds {max} entries"),
            Self::Duplicate { field } => write!(formatter, "duplicate {field}"),
            Self::InvalidPayloadLength { digest, max } => write!(
                formatter,
                "bundle payload {digest:?} has a length outside 1..={max}"
            ),
            Self::ConflictingPayload(digest) => write!(
                formatter,
                "bundle payload {digest:?} has conflicting format or length metadata"
            ),
            Self::MissingTargetAssociation(digest) => write!(
                formatter,
                "bundle kernel references unknown manifest {digest:?}"
            ),
            Self::MissingPayload(digest) => {
                write!(
                    formatter,
                    "bundle kernel references unknown payload {digest:?}"
                )
            }
            Self::UnknownKernel(_) => write!(formatter, "bundle does not contain the kernel"),
            Self::ExportSymbolMismatch { .. } => {
                write!(formatter, "bundle kernel export symbol does not match")
            }
            Self::PayloadMembershipMismatch { .. } => {
                write!(formatter, "bundle kernel payload membership does not match")
            }
            Self::PayloadProfileMismatch(digest) => write!(
                formatter,
                "bundle payload {digest:?} format or byte length does not match"
            ),
            Self::TargetProfileMismatch { field, .. } => {
                write!(
                    formatter,
                    "candidate target {field} does not match bundle target"
                )
            }
            Self::MissingTargetCapability { capability, .. } => write!(
                formatter,
                "candidate target lacks bundle capability {}",
                capability.name()
            ),
        }
    }
}

impl std::error::Error for BundleValidationError {}

fn require_count(
    count: usize,
    field: &'static str,
    max: usize,
) -> Result<(), BundleValidationError> {
    if count == 0 {
        Err(BundleValidationError::EmptyCollection { field })
    } else if count > max {
        Err(BundleValidationError::TooMany { field, max })
    } else {
        Ok(())
    }
}

fn reject_duplicate_by<T>(
    values: &[T],
    key: impl Fn(&T) -> DigestBytes,
    field: &'static str,
) -> Result<(), BundleValidationError> {
    if values.windows(2).any(|pair| key(&pair[0]) == key(&pair[1])) {
        Err(BundleValidationError::Duplicate { field })
    } else {
        Ok(())
    }
}

fn reject_duplicate_symbols(
    kernels: &[BundleKernelIndexEntryV1],
) -> Result<(), BundleValidationError> {
    let mut symbols = kernels
        .iter()
        .map(BundleKernelIndexEntryV1::symbol)
        .collect::<Vec<_>>();
    symbols.sort_unstable();
    if symbols.windows(2).any(|pair| pair[0] == pair[1]) {
        Err(BundleValidationError::Duplicate {
            field: "bundle kernel symbol",
        })
    } else {
        Ok(())
    }
}

fn reject_duplicate_values<T: Eq>(
    values: &[T],
    field: &'static str,
) -> Result<(), BundleValidationError> {
    if values.windows(2).any(|pair| pair[0] == pair[1]) {
        Err(BundleValidationError::Duplicate { field })
    } else {
        Ok(())
    }
}

fn check_target_profile(
    manifest_digest: DigestBytes,
    required: &TargetIdentity,
    candidate: &TargetIdentity,
) -> Result<(), BundleValidationError> {
    if required.triple() != candidate.triple() {
        return Err(BundleValidationError::TargetProfileMismatch {
            manifest_digest,
            field: "triple",
        });
    }
    if required.architecture() != candidate.architecture() {
        return Err(BundleValidationError::TargetProfileMismatch {
            manifest_digest,
            field: "architecture",
        });
    }
    if required.pointer_width() != candidate.pointer_width() {
        return Err(BundleValidationError::TargetProfileMismatch {
            manifest_digest,
            field: "pointer width",
        });
    }
    if required.endianness() != candidate.endianness() {
        return Err(BundleValidationError::TargetProfileMismatch {
            manifest_digest,
            field: "endianness",
        });
    }
    for capability in required.capabilities() {
        if candidate.capabilities().binary_search(capability).is_err() {
            return Err(BundleValidationError::MissingTargetCapability {
                manifest_digest,
                capability: *capability,
            });
        }
    }
    Ok(())
}

fn deduplicate_identical_payloads(
    payloads: &mut Vec<BundlePayloadReferenceV1>,
) -> Result<(), BundleValidationError> {
    payloads.sort_unstable_by_key(BundlePayloadReferenceV1::digest);
    let mut deduplicated: Vec<BundlePayloadReferenceV1> = Vec::with_capacity(payloads.len());
    for payload in payloads.drain(..) {
        if let Some(previous) = deduplicated.last()
            && previous.digest() == payload.digest()
        {
            if previous.format() != payload.format() || previous.byte_len() != payload.byte_len() {
                return Err(BundleValidationError::ConflictingPayload(payload.digest()));
            }
            continue;
        }
        deduplicated.push(payload);
    }
    *payloads = deduplicated;
    Ok(())
}
