//! Fresh native key custody, separate from public-record transport and V1 owners.
use crate::{
    native_capability::{
        CompilerExecutionCapabilityErrorV2 as Error, ENTRY_WORK, Result, Storage, envelope_overhead,
    },
    sealed_image::{CapabilityRole, SealedCapabilityImage},
};
use ed25519_dalek::{Signer, SigningKey};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionAttestationReceiptV2 as Receipt,
    CompilerExecutionAttestationRequestV2 as Request,
    CompilerExecutionAttestationStorageV2 as ProtocolStorage,
    CompilerExecutionCurrentRecordAttestationV3 as CurrentAttestation,
    CompilerExecutionCurrentRecordVerificationV3 as CurrentVerification,
    CompilerExecutionIssuerPolicyIdentityV2 as PolicyIdentity,
    CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionNativeJournalErrorV2 as JournalError,
    CompilerExecutionReceiptCarriageV2 as Carriage,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{fmt, fs::File, mem::size_of, os::fd::RawFd};
use subtle::ConstantTimeEq;
use zeroize::{Zeroize, ZeroizeOnDrop};

const KEY_BYTES: usize = 32;
const CRYPTO_SCRATCH: usize = 4096;
const ROLE: CapabilityRole = CapabilityRole {
    name: "native compiler-execution signing-key capability",
    memfd_name: "fe2o3-compiler-execution-signing-key-v2",
};

/// Move-only secret custody pinned to a complete native policy identity.
///
/// Fresh admission derives the public key once. Revalidation compares the seed
/// without deriving another key. Signing operations retain and revalidate this
/// capability; no direct seed/key getter, V1 owner conversion, process launch,
/// or execution authority is exposed. The consuming protected issuer must
/// independently establish occurrence, currentness and durable ordering.
/// A transferred File contains the readable seed: its recipient must be trusted.
///
/// Inputs stay prepaid on the same ledger. Calls restore entry storage; reserve
/// each returned delta before retaining the result. Retire a consumed input's
/// charge only after its drop or explicit ownership transfer, including errors.
///
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV2;
/// fn clone<T: Clone>() {}
/// clone::<CompilerExecutionSigningKeyCapabilityV2>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV2;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<CompilerExecutionSigningKeyCapabilityV2>();
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::{
///     CompilerExecutionSigningKeyCapabilityV1, CompilerExecutionSigningKeyCapabilityV2,
/// };
/// fn upgrade(key: CompilerExecutionSigningKeyCapabilityV1) -> CompilerExecutionSigningKeyCapabilityV2 {
///     key.into()
/// }
/// ```
pub struct CompilerExecutionSigningKeyCapabilityV2 {
    key: SigningKey,
    image: SealedCapabilityImage,
    policy: PolicyIdentity,
}

impl CompilerExecutionSigningKeyCapabilityV2 {
    const RETAINED: usize = size_of::<(Self, Storage)>() + KEY_BYTES;

    /// Logical charge for one File and its 32-byte sealed image, including when
    /// multiple descriptors share backing. This is not physical kernel memory.
    pub const FILE_STORAGE: usize = size_of::<(File, Storage)>() + KEY_BYTES;
    /// Named fixed crypto allowance for one pinned Dalek Ed25519 key derivation.
    /// It is an admission unit, not a measured instruction or wall-time bound.
    pub const DERIVATION_WORK: usize = 65_536;
    /// Fixed Dalek signing allowance for one journal digest.
    pub const SIGN_WORK: usize = 65_536;
    /// Entry, at most 64 descriptor/credential/cleanup calls at weight 1024,
    /// and fixed byte staging/comparison. No native I/O operation retries.
    /// Borrowed transfer validation uses 38 calls: two 19-call secret checks,
    /// each with pre/post metadata, credentials, access and one positional read.
    pub const IO_WORK: usize = ENTRY_WORK + 64 * 1024 + 32 * KEY_BYTES;
    /// Create, consuming-file and inherited admission each derive one key.
    pub const ADMISSION_WORK: usize = Self::IO_WORK + Self::DERIVATION_WORK;
    /// Additional logical scratch for result/staging owners, guarded seeds,
    /// metadata, fixed path/control frames and named crypto scratch. This is
    /// not a generated stack, allocator, kernel-page, RSS, or time bound.
    pub const IO_STORAGE: usize = 4 * Self::RETAINED
        + 4 * KEY_BYTES
        + 4 * size_of::<std::fs::Metadata>()
        + 4096
        + CRYPTO_SCRATCH;

    /// Borrows a prepaid 32-byte seed and policy, returning a FULL owner charge.
    /// The caller's seed is guarded before even entry-work admission, and wiped
    /// on success, error and unwind. Copies made before this call remain the
    /// caller's responsibility. Closing a memfd does not prove kernel-page erasure.
    pub fn create_and_zeroize(
        seed: &mut [u8; KEY_BYTES],
        policy: &Policy,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        let seed = SeedGuard(seed);
        Self::scope(
            budget,
            KEY_BYTES + policy.retained_storage(),
            Self::ADMISSION_WORK,
            |_| {
                let key = SigningKey::from_bytes(seed.0);
                require_policy_key(&key, policy)?;
                let image = SealedCapabilityImage::create_fixed(seed.0, ROLE)?
                    .into_read_only_fixed::<KEY_BYTES>()?;
                let admitted = Self {
                    key,
                    image,
                    policy: policy.identity(),
                };
                admitted.check_image()?;
                Ok((admitted, Storage(Self::RETAINED)))
            },
        )
    }

    /// Consumes a prepaid File, borrowing the prepaid native policy. Returns
    /// only growth over FILE_STORAGE, not a second full image charge.
    pub fn from_file(
        image: File,
        policy: &Policy,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        Self::scope(
            budget,
            Self::FILE_STORAGE + policy.retained_storage(),
            Self::ADMISSION_WORK,
            |_| {
                let image = SealedCapabilityImage::from_file_fixed::<KEY_BYTES>(image, ROLE)?;
                Ok((
                    Self::decode_image(image, policy)?,
                    Storage(Self::RETAINED - Self::FILE_STORAGE),
                ))
            },
        )
    }

    /// Borrows a live, non-CLOEXEC inherited fd >= 3 and the native policy.
    /// Keep FILE_STORAGE and the policy prepaid for the call. The source stays
    /// open; the private CLOEXEC owner returns its FULL retained charge.
    pub fn from_inherited_at(
        fd: RawFd,
        policy: &Policy,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        Self::scope(
            budget,
            Self::FILE_STORAGE + policy.retained_storage(),
            Self::ADMISSION_WORK,
            |_| {
                let image = SealedCapabilityImage::from_inherited_fixed::<KEY_BYTES>(fd, ROLE)?;
                Ok((Self::decode_image(image, policy)?, Storage(Self::RETAINED)))
            },
        )
    }

    fn decode_image(image: SealedCapabilityImage, policy: &Policy) -> Result<Self> {
        let key = read_secret(&image, |seed| Ok(SigningKey::from_bytes(seed)))?;
        require_policy_key(&key, policy)?;
        Ok(Self {
            key,
            image,
            policy: policy.identity(),
        })
    }

    /// Checks exact native policy identity as well as the seed, retained inode,
    /// seals, length, permissions, descriptor access and current service owner.
    pub fn revalidate(&self, policy: &Policy, budget: &mut Budget<'_>) -> Result<()> {
        Self::scope(
            budget,
            Self::RETAINED + policy.retained_storage(),
            Self::IO_WORK,
            |_| self.check_policy_image(policy),
        )
    }

    /// Signs one native request without exporting a key or seed. This authenticates
    /// bytes only; live occurrence custody must be established by the issuer.
    /// All inputs stay prepaid and the returned protocol storage is unreserved.
    pub fn issue_receipt(
        &self,
        policy: &Policy,
        request: &Request,
        budget: &mut Budget<'_>,
    ) -> Result<(Receipt, ProtocolStorage)> {
        self.signing_scope(policy, request.retained_storage(), budget, |budget| {
            Ok(Receipt::issue(policy, request, &self.key, budget)?)
        })
    }

    /// Consumes a prepaid currentness verification and returns its protocol growth.
    /// The native authenticator checks both anchor receipts and the fresh challenge;
    /// protected journal/currentness custody is still the consuming issuer's duty.
    pub fn attest_current(
        &self,
        policy: &Policy,
        carriage: &Carriage,
        verification: CurrentVerification,
        challenge: [u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<(CurrentAttestation, ProtocolStorage)> {
        let inputs = carriage
            .retained_storage()
            .checked_add(size_of::<(CurrentVerification, ProtocolStorage)>())
            .ok_or(Resource::Arithmetic)?;
        self.signing_scope(policy, inputs, budget, |budget| {
            CurrentAttestation::issue_native(
                policy,
                carriage,
                verification,
                challenge,
                &self.key,
                budget,
            )
            .map_err(|error| match error {
                JournalError::Resource(resource) => Error::Resource(resource),
                _ => Error::Rejected("native currentness authentication failed"),
            })
        })
    }

    /// Signs a fixed, caller-domain-separated journal digest. This is a key-use
    /// primitive, not proof that the journal is durable or its claims are true.
    pub fn sign_journal_digest(
        &self,
        policy: &Policy,
        digest: &[u8; 32],
        budget: &mut Budget<'_>,
    ) -> Result<([u8; 64], Storage)> {
        self.signing_scope(policy, 32, budget, |budget| {
            budget.charge_work(Self::SIGN_WORK)?;
            Ok((
                self.key.sign(digest).to_bytes(),
                Storage(size_of::<([u8; 64], Storage)>()),
            ))
        })
    }

    fn signing_scope<T>(
        &self,
        policy: &Policy,
        inputs: usize,
        budget: &mut Budget<'_>,
        operation: impl FnOnce(&mut Budget<'_>) -> Result<T>,
    ) -> Result<T> {
        let floor = Self::RETAINED
            .checked_add(policy.retained_storage())
            .and_then(|n| n.checked_add(inputs))
            .ok_or(Resource::Arithmetic)?;
        Self::scope(budget, floor, 2 * Self::IO_WORK, |budget| {
            self.check_policy_image(policy)?;
            let output = operation(budget)?;
            self.check_policy_image(policy)?;
            Ok(output)
        })
    }

    fn check_policy_image(&self, policy: &Policy) -> Result<()> {
        if policy.identity() != self.policy {
            return Err(Error::Rejected(
                "signing key is pinned to another native policy",
            ));
        }
        self.check_image()?;
        require_policy_key(&self.key, policy)
    }

    fn check_image(&self) -> Result<()> {
        read_secret(&self.image, |seed| self.check_seed(seed))
    }

    fn check_seed(&self, seed: &[u8; KEY_BYTES]) -> Result<()> {
        if !bool::from(seed.ct_eq(self.key.as_bytes())) {
            return Err(Error::Rejected("signing-key bytes changed"));
        }
        Ok(())
    }

    /// Revalidates and returns a separately charged read-only CLOEXEC File.
    /// This meters the transfer, not arbitrary future operations on that File.
    /// The trusted recipient can read the seed; read-only means immutable, not secret.
    pub fn try_clone_for_transfer(&self, budget: &mut Budget<'_>) -> Result<(File, Storage)> {
        Self::scope(budget, Self::RETAINED, Self::IO_WORK, |_| {
            self.check_image()?;
            Ok((self.image.clone_fixed()?, Storage(Self::FILE_STORAGE)))
        })
    }

    /// Revalidates the exact native policy, owner and borrowed read-only CLOEXEC
    /// transfer, including original object identity and guarded seed comparison.
    /// Prepay `retained_storage() + FILE_STORAGE + policy.retained_storage()` on
    /// the same ledger. Charges IO_WORK and IO_STORAGE scratch, restoring entry
    /// storage. No key derivation, descriptor duplication or ownership transfer;
    /// both descriptors stay live and secret staging is wiped on every exit.
    pub fn validate_transfer(
        &self,
        transfer: &File,
        policy: &Policy,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        Self::scope(
            budget,
            Self::RETAINED + Self::FILE_STORAGE + policy.retained_storage(),
            Self::IO_WORK,
            |_| {
                self.check_policy_image(policy)?;
                self.image.validate_secret_transfer_fixed(transfer)?;
                let mut seed = [0; KEY_BYTES];
                with_secret(
                    &mut seed,
                    |seed| {
                        self.image.read_transfer_fixed_into(transfer, seed)?;
                        self.image.validate_secret_transfer_fixed(transfer)
                    },
                    |seed| self.check_seed(seed),
                )
            },
        )
    }

    pub fn verifying_key(&self) -> [u8; KEY_BYTES] {
        self.key.verifying_key().to_bytes()
    }
    pub const fn policy_identity(&self) -> PolicyIdentity {
        self.policy
    }
    pub const fn retained_storage(&self) -> usize {
        Self::RETAINED
    }

    fn scope<T>(
        budget: &mut Budget<'_>,
        floor: usize,
        work: usize,
        operation: impl FnOnce(&mut Budget<'_>) -> Result<T>,
    ) -> Result<T> {
        budget.with_prepaid_scope(floor, ENTRY_WORK, work, Self::IO_STORAGE, operation)
    }
}

impl fmt::Debug for CompilerExecutionSigningKeyCapabilityV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompilerExecutionSigningKeyCapabilityV2")
            .field("authority", &"signing-key-custody-only")
            .field("policy", &self.policy)
            .finish_non_exhaustive()
    }
}

// This guard borrows the original caller buffer; it never creates a second seed.
struct SeedGuard<'a>(&'a mut [u8; KEY_BYTES]);
impl Drop for SeedGuard<'_> {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

fn read_secret<T>(
    image: &SealedCapabilityImage,
    use_seed: impl FnOnce(&[u8; KEY_BYTES]) -> Result<T>,
) -> Result<T> {
    image.validate_secret_fixed()?;
    let mut seed = [0; KEY_BYTES];
    with_secret(
        &mut seed,
        |seed| {
            image.read_fixed_into(seed)?;
            image.validate_secret_fixed()
        },
        use_seed,
    )
}

// Guard before I/O: a short read, I/O error or post-read refusal can leave bytes.
fn with_secret<T>(
    seed: &mut [u8; KEY_BYTES],
    read: impl FnOnce(&mut [u8; KEY_BYTES]) -> Result<()>,
    use_seed: impl FnOnce(&[u8; KEY_BYTES]) -> Result<T>,
) -> Result<T> {
    let seed = SeedGuard(seed);
    read(seed.0)?;
    use_seed(seed.0)
}

fn require_policy_key(key: &SigningKey, policy: &Policy) -> Result<()> {
    if key.verifying_key().as_bytes() != policy.verifying_key() {
        return Err(Error::Rejected(
            "signing key does not match the pinned native policy",
        ));
    }
    Ok(())
}

const _: () = {
    use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as Ledger;
    type Cap = CompilerExecutionSigningKeyCapabilityV2;
    assert!(Cap::RETAINED >= Cap::FILE_STORAGE);
    assert!(ROLE.memfd_name.len() < 128);
    assert!(
        8 * size_of::<Error>()
            + 64 * size_of::<usize>()
            + size_of::<Ledger>()
            + size_of::<std::result::Result<(), Resource>>()
            + 2 * size_of::<bool>()
            + size_of::<SeedGuard<'static>>()
            + 128
            + envelope_overhead::<(Cap, Storage), Error>()
            + envelope_overhead::<(File, Storage), Error>()
            + envelope_overhead::<(), Error>()
            <= 4096
    );
    fn zeroizing_key<T: ZeroizeOnDrop>() {}
    let _ = zeroizing_key::<SigningKey>;
};

#[cfg(test)]
#[path = "compiler_execution_signing_key_v2_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "compiler_execution_signing_key_v2_binding_tests.rs"]
mod binding_tests;
