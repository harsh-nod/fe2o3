use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrVerificationResourceErrorV1,
    CanonicalKernelIrVerificationResourceReceiptV1, CanonicalKernelIrWorkBudgetV1,
    CanonicalKernelIrWorkLimitV1, KERNEL_IR_MAGIC_V1, KERNEL_IR_VERSION_V12, KernelIrDecodeError,
    KernelIrEncodeError, MAX_MODULE_BYTES_V1, MeteredKernelIrVerificationErrorV1, Module,
    VerificationErrors, count_module_v12_wire_extent_with_work_v1, decode_module_v12,
    decode_module_v12_with_work_v1, encode_module_v12, encode_module_v12_with_work_v1,
    verify_exact_decoded_module_with_budget_v1, verify_module,
};

/// Exact domain bytes for verified canonical Kernel IR V12 policy identities.
pub const VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V12\0";
pub const VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_POLICY_V1: u16 = 1;

const VERSION_OFFSET: usize = 8;
const VERSION_END: usize = VERSION_OFFSET + 2;

/// Typed identity minted only for exact canonical Kernel IR V12 bytes accepted
/// by the semantic verifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VerifiedCanonicalKernelIrIdentityV12 {
    digest: [u8; 32],
    canonical_length: u64,
}

impl VerifiedCanonicalKernelIrIdentityV12 {
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    pub const fn canonical_length(&self) -> u64 {
        self.canonical_length
    }
}

/// Move-only owner of one exact V12 encoding whose decoded module passed
/// semantic verification.
#[derive(Debug, Eq, PartialEq)]
pub struct VerifiedCanonicalKernelIrV12 {
    canonical_bytes: Vec<u8>,
    identity: VerifiedCanonicalKernelIrIdentityV12,
}

/// Move-only custody of exact canonical V12 bytes and their freshly decoded,
/// semantically verified Module. Neither representation can be mutated or
/// separated through this API.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<VerifiedCanonicalKernelIrModuleV12>();
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12;
/// fn mutate(owner: &mut VerifiedCanonicalKernelIrModuleV12) {
///     owner.module().functions.clear();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12;
/// fn separate(owner: VerifiedCanonicalKernelIrModuleV12) {
///     let _ = owner.into_parts();
/// }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct VerifiedCanonicalKernelIrModuleV12 {
    canonical: VerifiedCanonicalKernelIrV12,
    module: Module,
}

impl VerifiedCanonicalKernelIrModuleV12 {
    /// Freshly encodes, inverse-decodes, verifies, compares and hashes the source
    /// under one shared ledger. The source is borrowed, never cloned or retained.
    ///
    /// The returned receipt transfers both canonical and decoded ownership:
    /// before another allocation, reserve its retained payload while this owner
    /// lives. All Result paths restore the caller's storage floor and preserve
    /// work, peak and first-failure histories. Failed construction drops its
    /// canonical and decoded owners before restoring that floor; owned semantic
    /// diagnostics, when returned, transfer to the caller as in bytes-only
    /// admission. Decoder tree bounds remain conservative payload bounds, not RSS.
    pub fn from_module_ref_with_verification_budget_v12(
        module: &Module,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(Self, CanonicalKernelIrReplayStorageV12), CanonicalKernelIrReplayAdmissionErrorV12>
    {
        let floor = budget.storage_checkpoint();
        let result = canonical_module_inner(module, budget);
        budget.rollback_storage(floor)?;
        result
    }

    pub const fn module(&self) -> &Module {
        &self.module
    }

    pub const fn canonical(&self) -> &VerifiedCanonicalKernelIrV12 {
        &self.canonical
    }
}

impl VerifiedCanonicalKernelIrV12 {
    pub fn from_module(module: Module) -> Result<Self, VerifiedCanonicalKernelIrErrorV12> {
        let canonical_bytes =
            encode_module_v12(&module).map_err(VerifiedCanonicalKernelIrErrorV12::Encode)?;
        let decoded = decode_exact_v12(&canonical_bytes)?;
        verify_module(&decoded).map_err(VerifiedCanonicalKernelIrErrorV12::Verification)?;
        if decoded != module {
            return Err(VerifiedCanonicalKernelIrErrorV12::RoundTripMismatch);
        }
        Ok(Self::from_validated_bytes(canonical_bytes))
    }

    /// Performs the exact V12 canonicalization flow while charging canonical
    /// byte reads, writes, comparisons, and identity hashing before execution.
    ///
    /// Semantic-verifier work remains the caller's separately admitted
    /// responsibility. This meter covers both encoder schema traversals and
    /// the data-volume work known only after canonical encoding is produced.
    pub fn from_module_with_work_budget_v1(
        module: Module,
        budget: &mut CanonicalKernelIrWorkBudgetV1,
    ) -> Result<Self, MeteredVerifiedCanonicalKernelIrErrorV12> {
        let canonical_bytes =
            encode_module_v12_with_work_v1(&module, budget).map_err(metered_encode_error_v12)?;
        let decoded = decode_exact_v12_with_work_v1(&canonical_bytes, budget)?;
        verify_module(&decoded).map_err(|error| {
            MeteredVerifiedCanonicalKernelIrErrorV12::Canonical(
                VerifiedCanonicalKernelIrErrorV12::Verification(error),
            )
        })?;
        // The canonical encoding injectively covers every compared structural
        // and dynamic module payload, so its exact length bounds this equality.
        budget
            .charge_work(canonical_bytes.len())
            .map_err(MeteredVerifiedCanonicalKernelIrErrorV12::WorkLimit)?;
        if decoded != module {
            return Err(MeteredVerifiedCanonicalKernelIrErrorV12::Canonical(
                VerifiedCanonicalKernelIrErrorV12::RoundTripMismatch,
            ));
        }
        Self::from_validated_bytes_with_work_v1(canonical_bytes, budget)
    }

    /// Performs canonical V12 construction with semantic verification charged
    /// to the shared work meter and a separate verifier-local storage limit.
    ///
    /// The returned receipt excludes caller-owned source, decoded module, and
    /// canonical wire buffers. A verification error transfers its diagnostic
    /// owners to the returned error after their in-verifier peak was observed.
    pub fn from_module_with_resource_budget_v1(
        module: Module,
        budget: &mut CanonicalKernelIrWorkBudgetV1,
        verification_storage_limit: usize,
    ) -> Result<
        (Self, CanonicalKernelIrVerificationResourceReceiptV1),
        MeteredVerifiedCanonicalKernelIrErrorV12,
    > {
        Self::from_module_ref_with_resource_budget_v1(&module, budget, verification_storage_limit)
    }

    /// Constructs an owned canonical V12 encoding from an immutable module
    /// borrow, without cloning the source module or retaining its reference.
    ///
    /// Runs the same fresh encoding, exact decoding, semantic verification,
    /// source equality and identity hashing as the owned resource-budget API.
    /// Work charges, verifier-local storage receipts and error order are identical.
    /// The caller remains responsible for source, decoded and wire owners.
    pub fn from_module_ref_with_resource_budget_v1(
        module: &Module,
        budget: &mut CanonicalKernelIrWorkBudgetV1,
        verification_storage_limit: usize,
    ) -> Result<
        (Self, CanonicalKernelIrVerificationResourceReceiptV1),
        MeteredVerifiedCanonicalKernelIrErrorV12,
    > {
        let canonical_bytes =
            encode_module_v12_with_work_v1(module, budget).map_err(metered_encode_error_v12)?;
        let decoded = decode_exact_v12_with_work_v1(&canonical_bytes, budget)?;
        let verification_start = budget.work();
        let mut verification_budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(budget, verification_storage_limit);
        let verification =
            verify_exact_decoded_module_with_budget_v1(&decoded, None, &mut verification_budget);
        let verification_end = verification_budget.work();
        let verification_peak = verification_budget.peak_storage();
        let Some(verification_work) = verification_end.checked_sub(verification_start) else {
            return Err(
                MeteredVerifiedCanonicalKernelIrErrorV12::VerificationResource {
                    error: CanonicalKernelIrVerificationResourceErrorV1::Accounting,
                    receipt: CanonicalKernelIrVerificationResourceReceiptV1::new(
                        0,
                        verification_peak,
                    ),
                },
            );
        };
        let receipt = CanonicalKernelIrVerificationResourceReceiptV1::new(
            verification_work,
            verification_peak,
        );
        match verification {
            Ok(_) => {}
            Err(MeteredKernelIrVerificationErrorV1::Verification(error)) => {
                return Err(MeteredVerifiedCanonicalKernelIrErrorV12::Verification {
                    error,
                    receipt,
                });
            }
            Err(MeteredKernelIrVerificationErrorV1::Resource(
                CanonicalKernelIrVerificationResourceErrorV1::Work(error),
            )) => {
                return Err(
                    MeteredVerifiedCanonicalKernelIrErrorV12::VerificationWorkLimit {
                        error,
                        receipt,
                    },
                );
            }
            Err(MeteredKernelIrVerificationErrorV1::Resource(error)) => {
                return Err(
                    MeteredVerifiedCanonicalKernelIrErrorV12::VerificationResource {
                        error,
                        receipt,
                    },
                );
            }
        }
        budget.charge_work(canonical_bytes.len()).map_err(|error| {
            MeteredVerifiedCanonicalKernelIrErrorV12::VerificationWorkLimit { error, receipt }
        })?;
        if &decoded != module {
            return Err(
                MeteredVerifiedCanonicalKernelIrErrorV12::PostVerificationCanonical {
                    error: VerifiedCanonicalKernelIrErrorV12::RoundTripMismatch,
                    receipt,
                },
            );
        }
        let identity = canonical_identity_v12_with_work_v1(&canonical_bytes, budget)
            .map_err(|error| metered_post_verification_error_v12(error, receipt))?;
        Ok((
            Self {
                canonical_bytes,
                identity,
            },
            receipt,
        ))
    }

    pub fn from_canonical_bytes(
        canonical_bytes: Vec<u8>,
    ) -> Result<Self, VerifiedCanonicalKernelIrErrorV12> {
        Self::from_canonical_bytes_with_module(canonical_bytes).map(|(owner, _)| owner)
    }

    pub fn from_canonical_bytes_with_module(
        canonical_bytes: Vec<u8>,
    ) -> Result<(Self, Module), VerifiedCanonicalKernelIrErrorV12> {
        let decoded = decode_exact_v12(&canonical_bytes)?;
        verify_module(&decoded).map_err(VerifiedCanonicalKernelIrErrorV12::Verification)?;
        Ok((Self::from_validated_bytes(canonical_bytes), decoded))
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity(&self) -> &VerifiedCanonicalKernelIrIdentityV12 {
        &self.identity
    }

    pub fn revalidate(&self) -> Result<(), VerifiedCanonicalKernelIrErrorV12> {
        let decoded = decode_exact_v12(&self.canonical_bytes)?;
        verify_module(&decoded).map_err(VerifiedCanonicalKernelIrErrorV12::Verification)?;
        if canonical_identity_v12(&self.canonical_bytes) != self.identity {
            return Err(VerifiedCanonicalKernelIrErrorV12::IdentityMismatch);
        }
        Ok(())
    }

    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.canonical_bytes
    }

    fn from_validated_bytes(canonical_bytes: Vec<u8>) -> Self {
        let identity = canonical_identity_v12(&canonical_bytes);
        Self {
            canonical_bytes,
            identity,
        }
    }

    fn from_validated_bytes_with_work_v1(
        canonical_bytes: Vec<u8>,
        budget: &mut CanonicalKernelIrWorkBudgetV1,
    ) -> Result<Self, MeteredVerifiedCanonicalKernelIrErrorV12> {
        let identity = canonical_identity_v12_with_work_v1(&canonical_bytes, budget)?;
        Ok(Self {
            canonical_bytes,
            identity,
        })
    }
}

fn decode_exact_v12(bytes: &[u8]) -> Result<Module, VerifiedCanonicalKernelIrErrorV12> {
    if bytes.len() > MAX_MODULE_BYTES_V1 {
        return Err(VerifiedCanonicalKernelIrErrorV12::Decode(
            KernelIrDecodeError::TooLarge {
                max: MAX_MODULE_BYTES_V1,
            },
        ));
    }
    let magic =
        bytes
            .get(..KERNEL_IR_MAGIC_V1.len())
            .ok_or(VerifiedCanonicalKernelIrErrorV12::Decode(
                KernelIrDecodeError::Truncated,
            ))?;
    if magic != KERNEL_IR_MAGIC_V1 {
        return Err(VerifiedCanonicalKernelIrErrorV12::Decode(
            KernelIrDecodeError::InvalidMagic,
        ));
    }
    let version_bytes =
        bytes
            .get(VERSION_OFFSET..VERSION_END)
            .ok_or(VerifiedCanonicalKernelIrErrorV12::Decode(
                KernelIrDecodeError::Truncated,
            ))?;
    let version = u16::from_le_bytes([version_bytes[0], version_bytes[1]]);
    if version != KERNEL_IR_VERSION_V12 {
        return Err(VerifiedCanonicalKernelIrErrorV12::NotExactV12 { version });
    }
    decode_module_v12(bytes).map_err(VerifiedCanonicalKernelIrErrorV12::Decode)
}

fn decode_exact_v12_with_work_v1(
    bytes: &[u8],
    budget: &mut CanonicalKernelIrWorkBudgetV1,
) -> Result<Module, MeteredVerifiedCanonicalKernelIrErrorV12> {
    if bytes.len() > MAX_MODULE_BYTES_V1 {
        return Err(MeteredVerifiedCanonicalKernelIrErrorV12::Canonical(
            VerifiedCanonicalKernelIrErrorV12::Decode(KernelIrDecodeError::TooLarge {
                max: MAX_MODULE_BYTES_V1,
            }),
        ));
    }
    let magic = bytes.get(..KERNEL_IR_MAGIC_V1.len()).ok_or(
        MeteredVerifiedCanonicalKernelIrErrorV12::Canonical(
            VerifiedCanonicalKernelIrErrorV12::Decode(KernelIrDecodeError::Truncated),
        ),
    )?;
    budget
        .charge_work(KERNEL_IR_MAGIC_V1.len())
        .map_err(MeteredVerifiedCanonicalKernelIrErrorV12::WorkLimit)?;
    if magic != KERNEL_IR_MAGIC_V1 {
        return Err(MeteredVerifiedCanonicalKernelIrErrorV12::Canonical(
            VerifiedCanonicalKernelIrErrorV12::Decode(KernelIrDecodeError::InvalidMagic),
        ));
    }
    let version_bytes = bytes.get(VERSION_OFFSET..VERSION_END).ok_or(
        MeteredVerifiedCanonicalKernelIrErrorV12::Canonical(
            VerifiedCanonicalKernelIrErrorV12::Decode(KernelIrDecodeError::Truncated),
        ),
    )?;
    budget
        .charge_work(version_bytes.len())
        .map_err(MeteredVerifiedCanonicalKernelIrErrorV12::WorkLimit)?;
    let version = u16::from_le_bytes([version_bytes[0], version_bytes[1]]);
    if version != KERNEL_IR_VERSION_V12 {
        return Err(MeteredVerifiedCanonicalKernelIrErrorV12::Canonical(
            VerifiedCanonicalKernelIrErrorV12::NotExactV12 { version },
        ));
    }
    decode_module_v12_with_work_v1(bytes, budget).map_err(metered_decode_error_v12)
}

fn canonical_identity_v12(bytes: &[u8]) -> VerifiedCanonicalKernelIrIdentityV12 {
    let canonical_length =
        u64::try_from(bytes.len()).expect("hard-bounded canonical Kernel IR length fits u64");
    let domain_length = u32::try_from(VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_DOMAIN_V1.len())
        .expect("frozen canonical Kernel IR identity domain length fits u32");
    let mut digest = Sha256::new();
    digest.update(domain_length.to_le_bytes());
    digest.update(VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_DOMAIN_V1);
    digest.update(VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_POLICY_V1.to_le_bytes());
    digest.update(canonical_length.to_le_bytes());
    digest.update(bytes);
    VerifiedCanonicalKernelIrIdentityV12 {
        digest: digest.finalize().into(),
        canonical_length,
    }
}

fn canonical_identity_v12_with_work_v1(
    bytes: &[u8],
    budget: &mut CanonicalKernelIrWorkBudgetV1,
) -> Result<VerifiedCanonicalKernelIrIdentityV12, MeteredVerifiedCanonicalKernelIrErrorV12> {
    let canonical_length = u64::try_from(bytes.len()).map_err(|_| {
        MeteredVerifiedCanonicalKernelIrErrorV12::WorkLimit(CanonicalKernelIrWorkLimitV1::new(
            usize::MAX,
            budget.limit(),
        ))
    })?;
    let domain_length = u32::try_from(VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_DOMAIN_V1.len())
        .map_err(|_| {
            MeteredVerifiedCanonicalKernelIrErrorV12::WorkLimit(CanonicalKernelIrWorkLimitV1::new(
                usize::MAX,
                budget.limit(),
            ))
        })?;
    let hash_bytes = std::mem::size_of::<u32>()
        .checked_add(VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_DOMAIN_V1.len())
        .and_then(|work| work.checked_add(std::mem::size_of::<u16>()))
        .and_then(|work| work.checked_add(std::mem::size_of::<u64>()))
        .and_then(|work| work.checked_add(bytes.len()))
        .ok_or_else(|| {
            MeteredVerifiedCanonicalKernelIrErrorV12::WorkLimit(CanonicalKernelIrWorkLimitV1::new(
                usize::MAX,
                budget.limit(),
            ))
        })?;
    budget
        .charge_work(hash_bytes)
        .map_err(MeteredVerifiedCanonicalKernelIrErrorV12::WorkLimit)?;
    let mut digest = Sha256::new();
    digest.update(domain_length.to_le_bytes());
    digest.update(VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_DOMAIN_V1);
    digest.update(VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_POLICY_V1.to_le_bytes());
    digest.update(canonical_length.to_le_bytes());
    digest.update(bytes);
    Ok(VerifiedCanonicalKernelIrIdentityV12 {
        digest: digest.finalize().into(),
        canonical_length,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerifiedCanonicalKernelIrErrorV12 {
    Encode(KernelIrEncodeError),
    Decode(KernelIrDecodeError),
    Verification(VerificationErrors),
    NotExactV12 { version: u16 },
    RoundTripMismatch,
    IdentityMismatch,
}

impl fmt::Display for VerifiedCanonicalKernelIrErrorV12 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(error) => {
                write!(formatter, "cannot encode canonical Kernel IR V12: {error}")
            }
            Self::Decode(error) => {
                write!(formatter, "cannot decode canonical Kernel IR V12: {error}")
            }
            Self::Verification(error) => error.fmt(formatter),
            Self::NotExactV12 { version } => {
                write!(
                    formatter,
                    "expected exact Kernel IR V12 bytes, found V{version}"
                )
            }
            Self::RoundTripMismatch => {
                formatter.write_str("Kernel IR V12 round trip changed bytes or semantics")
            }
            Self::IdentityMismatch => {
                formatter.write_str("canonical Kernel IR V12 identity mismatch")
            }
        }
    }
}

impl Error for VerifiedCanonicalKernelIrErrorV12 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Encode(error) => Some(error),
            Self::Decode(error) => Some(error),
            Self::Verification(error) => Some(error),
            Self::NotExactV12 { .. } | Self::RoundTripMismatch | Self::IdentityMismatch => None,
        }
    }
}

include!("canonical_kir_v12_resource_errors.rs");

/// Conservative retained payload transferred out of a completed admission.
/// The admitting call restores its incoming storage floor on every exit. Before
/// another allocation, callers reserve this amount while the returned owner lives.
/// Wire bytes and requested decoded heap payload are counted explicitly; existing
/// verifier-local logical units retain their convention. This is not an RSS claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKernelIrReplayStorageV12 {
    retained: usize,
}
impl CanonicalKernelIrReplayStorageV12 {
    /// Returns the conservative retained payload reservation to transfer.
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

/// A closed rejection from complete allocation-budgeted canonical admission.
#[derive(Debug)]
pub enum CanonicalKernelIrReplayAdmissionErrorV12 {
    /// Wire parsing or its opt-in allocation admission failed.
    Decode(KernelIrDecodeError),
    /// The unchanged full encoder rejected the actual module.
    Encode(KernelIrEncodeError),
    /// Fresh verification rejected the inverse-decoded module.
    Verification(VerificationErrors),
    /// Cumulative work, storage, arithmetic or host allocation admission failed.
    Resource(CanonicalKernelIrVerificationResourceErrorV1),
    /// Canonical identity hashing or its work charge failed.
    Canonical(MeteredVerifiedCanonicalKernelIrErrorV12),
    /// Full inverse equality or supplied canonical bytes did not match.
    CanonicalMismatch,
}
impl fmt::Display for CanonicalKernelIrReplayAdmissionErrorV12 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(e) => write!(f, "canonical admission decode failed: {e}"),
            Self::Encode(e) => write!(f, "canonical admission encode failed: {e}"),
            Self::Verification(e) => write!(f, "canonical admission verification failed: {e}"),
            Self::Resource(e) => e.fmt(f),
            Self::Canonical(e) => e.fmt(f),
            Self::CanonicalMismatch => {
                f.write_str("canonical admission inverse differs from its exact subject")
            }
        }
    }
}
impl Error for CanonicalKernelIrReplayAdmissionErrorV12 {}
impl From<CanonicalKernelIrVerificationResourceErrorV1>
    for CanonicalKernelIrReplayAdmissionErrorV12
{
    fn from(error: CanonicalKernelIrVerificationResourceErrorV1) -> Self {
        Self::Resource(error)
    }
}
type AdmissionError = CanonicalKernelIrReplayAdmissionErrorV12;

impl VerifiedCanonicalKernelIrV12 {
    /// Runs the complete fresh encode/decode/verify/equality/hash flow from a
    /// Module borrow, with decoder allocations and inverse coexistence charged
    /// to one caller ledger. The input Module is caller-owned; it is never cloned.
    /// All exits restore the incoming storage floor while preserving work/peak.
    pub fn from_module_ref_with_verification_budget_v12(
        module: &Module,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<(Self, CanonicalKernelIrReplayStorageV12), AdmissionError> {
        let floor = budget.storage_checkpoint();
        let result = canonical_inner(module, budget).map(|owner| {
            let receipt = CanonicalKernelIrReplayStorageV12 {
                retained: std::mem::size_of::<Self>() + owner.canonical_bytes().len(),
            };
            (owner, receipt)
        });
        budget.rollback_storage(floor)?;
        result
    }
}

fn canonical_inner(
    module: &Module,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<VerifiedCanonicalKernelIrV12, AdmissionError> {
    let inverse = canonical_verified_inverse(module, budget, std::mem::size_of::<Module>())?;
    drop(inverse.module);
    budget.rollback_storage(inverse.storage_floor)?;
    VerifiedCanonicalKernelIrV12::from_validated_bytes_with_work_v1(
        inverse.canonical_bytes,
        budget.work_budget_v1(),
    )
    .map_err(AdmissionError::Canonical)
}

fn canonical_module_inner(
    module: &Module,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<
    (
        VerifiedCanonicalKernelIrModuleV12,
        CanonicalKernelIrReplayStorageV12,
    ),
    AdmissionError,
> {
    let floor = budget.storage_checkpoint();
    // Include any aggregate padding in the retained header without changing
    // the established bytes-only owner's reservation order or units.
    let inverse_inline_payload = std::mem::size_of::<VerifiedCanonicalKernelIrModuleV12>()
        .checked_sub(std::mem::size_of::<VerifiedCanonicalKernelIrV12>())
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    let inverse = canonical_verified_inverse(module, budget, inverse_inline_payload)?;
    let canonical = VerifiedCanonicalKernelIrV12::from_validated_bytes_with_work_v1(
        inverse.canonical_bytes,
        budget.work_budget_v1(),
    )
    .map_err(AdmissionError::Canonical)?;
    // Decoder temporaries and verifier scratch have already been released.
    // Transfer the admitted inverse payload without rewalking or cloning it.
    let retained = budget
        .storage()
        .checked_sub(floor)
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
    Ok((
        VerifiedCanonicalKernelIrModuleV12 {
            canonical,
            module: inverse.module,
        },
        CanonicalKernelIrReplayStorageV12 { retained },
    ))
}

struct CanonicalVerifiedInverseV12 {
    canonical_bytes: Vec<u8>,
    module: Module,
    storage_floor: usize,
}

fn canonical_verified_inverse(
    module: &Module,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    inverse_inline_payload: usize,
) -> Result<CanonicalVerifiedInverseV12, AdmissionError> {
    let extent = count_module_v12_wire_extent_with_work_v1(module, budget.work_budget_v1())
        .map_err(AdmissionError::Encode)?;
    let retained = extent
        .wire_bytes()
        .checked_add(std::mem::size_of::<VerifiedCanonicalKernelIrV12>())
        .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    budget.reserve_storage(retained)?;
    let encoder_scratch =
        crate::wire::decoded_tree_payload_bound_v12::<&crate::FunctionId>(module.kernels.len())
            .map_err(AdmissionError::Decode)?
            .checked_add(extent.peak_auxiliary_bytes())
            .ok_or(CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
    budget.reserve_storage(encoder_scratch)?;
    let encoded = crate::wire::encode_module_v12_with_work_v1(module, budget.work_budget_v1())
        .map_err(AdmissionError::Encode)?;
    budget.release_storage(encoder_scratch)?;
    if encoded.len() != extent.wire_bytes() || encoded.capacity() != encoded.len() {
        return Err(AdmissionError::CanonicalMismatch);
    }
    let inverse_floor = budget.storage_checkpoint();
    budget.reserve_storage(inverse_inline_payload)?;
    let decoded = crate::wire::decode_module_v12_with_allocation_budget_v1(&encoded, budget)
        .map_err(AdmissionError::Decode)?;
    verify_exact_decoded_module_with_budget_v1(&decoded, None, budget).map_err(
        |error| match error {
            MeteredKernelIrVerificationErrorV1::Verification(error) => {
                AdmissionError::Verification(error)
            }
            MeteredKernelIrVerificationErrorV1::Resource(error) => AdmissionError::Resource(error),
        },
    )?;
    budget.charge_work(encoded.len())?;
    if &decoded != module {
        return Err(AdmissionError::CanonicalMismatch);
    }
    Ok(CanonicalVerifiedInverseV12 {
        canonical_bytes: encoded,
        module: decoded,
        storage_floor: inverse_floor,
    })
}

#[cfg(test)]
#[path = "canonical_kir_v12_module_tests.rs"]
mod module_tests;

#[cfg(test)]
#[path = "canonical_kir_v12_allocation_tests.rs"]
mod allocation_tests;

#[cfg(test)]
#[path = "canonical_kir_v12_payload_boundary_tests.rs"]
mod payload_boundary_tests;

#[cfg(test)]
#[path = "canonical_kir_v12_resource_tests.rs"]
mod resource_tests;

#[cfg(test)]
#[path = "canonical_kir_v12_borrowed_resource_tests.rs"]
mod borrowed_resource_tests;
