//! Independent native content policy and protected proof resources.
use crate::deployment::NativeApplicationProofCustodianDeploymentV1 as Config;
use crate::{other, require};
use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV5 as Handoff;
use fe2o3_kernel_ir::{
    CanonicalKernelIrStorageAccountIdentityV1 as Account,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError,
    CanonicalKernelIrWorkBudgetV1 as Work, CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
};
use fe2o3_verifier::{
    RecoveredCompilerConditionalNativeSemanticHandoffStorageV5 as RecoveredStorage,
    RecoveredCompilerConditionalNativeSemanticHandoffV5 as Recovered,
    recover_native_conditional_handoff_under_policy_file_v1 as recover,
    validate_native_conditional_root_policy_file_v1 as validate_policy,
};
use sha2::{Digest, Sha256};
use std::{fs::File, io, marker::PhantomData, os::unix::fs::MetadataExt};

pub(crate) use fe2o3_verifier::MAX_NATIVE_CONDITIONAL_ROOT_POLICY_FILE_BYTES_V1 as MAX_POLICY_BYTES;
mod sealed;
pub(crate) use sealed::{read_sealed_file, seal_native_control_bytes, seal_policy_source};
mod tools;
pub(crate) use tools::Tools;
const IO_WORK: usize = 64 * 1024;
const IO_STORAGE: usize = 64 * 1024;

/// Original root-sealed policy input matching independently pinned configuration.
/// This is not, alone, proof of fixed-path installation or parent authenticity:
/// root deployment/worker activation retains those separate original owners.
pub(crate) struct Policy<'work> {
    file: File,
    bytes: Box<[u8]>,
    expected: ([u8; 32], u64),
    file_identity: (u64, u64),
    ledger: Ledger,
    account: Option<Account>,
    lifetime: PhantomData<&'work Work>,
}
impl<'work> Policy<'work> {
    pub(crate) fn admit(
        file: File,
        config: &Config,
        budget: &mut Budget<'work>,
    ) -> io::Result<(Self, usize)> {
        let floor = budget.storage();
        budget.charge_work(IO_WORK).map_err(other)?;
        let expected = config.semantic_policy();
        let len = usize::try_from(expected.1).map_err(other)?;
        require(
            expected.0 != [0; 32] && (1..=MAX_POLICY_BYTES).contains(&len),
            "native policy configured extent",
        )?;
        require(
            floor >= Config::RETAINED + size_of::<File>(),
            "native policy inputs not prepaid",
        )?;
        // The mechanical sealed reader may transiently overlap its Vec and Box.
        budget
            .reserve_storage(IO_STORAGE + 3 * len)
            .map_err(other)?;
        budget.charge_work(4 * len).map_err(other)?;
        check_descriptor(&file, expected.1)?;
        let (bytes, bytes_charge) = read_sealed_file(&file, (0, 0), len, budget)?;
        budget.reserve_storage(bytes_charge).map_err(other)?;
        require(
            bytes.len() == len && <[u8; 32]>::from(Sha256::digest(&bytes)) == expected.0,
            "native policy pinned bytes differ",
        )?;
        validate_policy(&bytes, budget)?;
        let metadata = file.metadata()?;
        let value = Self {
            file,
            bytes,
            expected,
            file_identity: (metadata.dev(), metadata.ino()),
            ledger: budget.work_ledger_identity_v1(),
            account: budget.storage_account_identity_v1(),
            lifetime: PhantomData,
        };
        value.revalidate_inner(budget)?;
        let additional = value
            .retained_storage()
            .checked_sub(size_of::<File>())
            .ok_or_else(|| io::Error::other("native policy retained accounting"))?;
        budget
            .release_storage(budget.storage() - floor)
            .map_err(other)?;
        Ok((value, additional))
    }
    pub(crate) fn retained_storage(&self) -> usize {
        size_of::<Self>() + self.bytes.len() + size_of::<usize>()
    }
    fn require_account(&self, budget: &Budget<'work>) -> io::Result<()> {
        require(
            self.ledger == budget.work_ledger_identity_v1()
                && self.account == budget.storage_account_identity_v1(),
            "native policy account replaced",
        )
    }
    pub(crate) fn revalidate(&self, budget: &mut Budget<'work>) -> io::Result<()> {
        budget.charge_work(IO_WORK).map_err(other)?;
        self.require_account(budget)?;
        require(
            budget.storage() >= self.retained_storage(),
            "native policy owner not prepaid",
        )?;
        let scratch = IO_STORAGE + 2 * self.bytes.len();
        budget.reserve_storage(scratch).map_err(other)?;
        self.revalidate_inner(budget)?;
        budget.release_storage(scratch).map_err(other)
    }
    fn revalidate_inner(&self, budget: &mut Budget<'work>) -> io::Result<()> {
        self.require_account(budget)?;
        budget.charge_work(4 * self.bytes.len()).map_err(other)?;
        check_descriptor(&self.file, self.expected.1)?;
        let metadata = self.file.metadata()?;
        require(
            (metadata.dev(), metadata.ino()) == self.file_identity,
            "native policy original descriptor changed",
        )?;
        let (bytes, _) = read_sealed_file(&self.file, (0, 0), self.bytes.len(), budget)?;
        require(
            bytes == self.bytes && <[u8; 32]>::from(Sha256::digest(&bytes)) == self.expected.0,
            "native policy retained bytes changed",
        )?;
        Ok(())
    }
    /// CLOEXEC duplicate of the same sealed file, not a new fixed-path admission.
    /// Returns a full unreserved File/receipt charge; original custody is retained.
    pub(crate) fn try_clone_for_handoff(
        &self,
        budget: &mut Budget<'work>,
    ) -> io::Result<(File, usize)> {
        self.revalidate(budget)?;
        budget.charge_work(IO_WORK).map_err(other)?;
        budget
            .reserve_storage(size_of::<File>() + IO_STORAGE)
            .map_err(other)?;
        let file = self.file.try_clone()?;
        check_descriptor(&file, self.expected.1)?;
        let metadata = file.metadata()?;
        require(
            (metadata.dev(), metadata.ino()) == self.file_identity,
            "native policy handoff descriptor differs",
        )?;
        self.revalidate(budget)?;
        budget
            .release_storage(size_of::<File>() + IO_STORAGE)
            .map_err(other)?;
        Ok((file, size_of::<File>() + size_of::<usize>()))
    }
    /// Recovers content only under the independent sealed file's exact source-bound
    /// roster. No roster carried in the request may select acceptance. Handoff
    /// backing/metadata must already be prepaid. Failed recovery is terminal and
    /// retains all partial charges; there is no blanket-refund scope here.
    pub(crate) fn recover(
        &self,
        handoff: Handoff,
        budget: &mut Budget<'work>,
    ) -> io::Result<(Recovered, RecoveredStorage)> {
        self.revalidate(budget)?;
        recover(&self.bytes, handoff, budget)
    }
}
fn check_descriptor(file: &File, length: u64) -> io::Result<()> {
    require(
        rustix::io::fcntl_getfd(file)? == rustix::io::FdFlags::CLOEXEC
            && file.metadata()?.len() == length,
        "native policy descriptor flags or extent",
    )
}

enum ScopeError {
    Resource(ResourceError),
    Io(io::Error),
}
impl From<ResourceError> for ScopeError {
    fn from(value: ResourceError) -> Self {
        Self::Resource(value)
    }
}
/// Only bounded codecs and descriptor mechanics use this refundable scope. It
/// must never enclose terminal source recovery or generated proof execution.
fn refundable_scope<T>(
    budget: &mut Budget<'_>,
    floor: usize,
    work: usize,
    scratch: usize,
    run: impl FnOnce(&mut Budget<'_>) -> io::Result<T>,
) -> io::Result<T> {
    budget
        .with_prepaid_scope(floor, 0, work, scratch, |b| run(b).map_err(ScopeError::Io))
        .map_err(|error| match error {
            ScopeError::Resource(error) => other(error),
            ScopeError::Io(error) => error,
        })
}
