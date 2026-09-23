//! Shared move-only transport for the two native public trust records.
use crate::sealed_image::{CapabilityRole, SealedCapabilityImage};
use fe2o3_compiler_execution_protocol::{
    CompilerExecutionAttestationErrorV2, CompilerExecutionClientProfileErrorV2,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, fs::File, mem::size_of, os::fd::RawFd};

pub(crate) type Result<T> = std::result::Result<T, CompilerExecutionCapabilityErrorV2>;
pub(crate) const ENTRY_WORK: usize = 8;

/// Additional unreserved storage returned by a capability operation. Preserve
/// consumed inputs' reservations, then reserve this delta before keeping the
/// result. Retire an owner's FULL retained charge after drop or transfer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompilerExecutionCapabilityStorageV2(pub(crate) usize);
impl CompilerExecutionCapabilityStorageV2 {
    pub const fn additional_storage(self) -> usize {
        self.0
    }
}
pub(crate) use CompilerExecutionCapabilityStorageV2 as Storage;

/// Bounded diagnostics: no attacker-supplied paths, retry strings, or heap
/// formatting. I/O errors carry only a static operation and an OS errno.
#[derive(Debug)]
pub enum CompilerExecutionCapabilityErrorV2 {
    Resource(Resource),
    Policy(CompilerExecutionAttestationErrorV2),
    Profile(CompilerExecutionClientProfileErrorV2),
    Io { operation: &'static str, errno: i32 },
    Rejected(&'static str),
}
impl CompilerExecutionCapabilityErrorV2 {
    pub(crate) fn io(operation: &'static str, error: rustix::io::Errno) -> Self {
        Self::Io {
            operation,
            errno: error.raw_os_error(),
        }
    }
    pub(crate) fn last_os(operation: &'static str) -> Self {
        Self::Io {
            operation,
            errno: std::io::Error::last_os_error()
                .raw_os_error()
                .unwrap_or(libc::EIO),
        }
    }
}
impl From<Resource> for CompilerExecutionCapabilityErrorV2 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<CompilerExecutionAttestationErrorV2> for CompilerExecutionCapabilityErrorV2 {
    fn from(value: CompilerExecutionAttestationErrorV2) -> Self {
        Self::Policy(value)
    }
}
impl From<CompilerExecutionClientProfileErrorV2> for CompilerExecutionCapabilityErrorV2 {
    fn from(value: CompilerExecutionClientProfileErrorV2) -> Self {
        Self::Profile(value)
    }
}
impl fmt::Display for CompilerExecutionCapabilityErrorV2 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(e) => e.fmt(f),
            Self::Policy(e) => e.fmt(f),
            Self::Profile(e) => e.fmt(f),
            Self::Io { operation, errno } => {
                write!(f, "native capability {operation}: errno {errno}")
            }
            Self::Rejected(reason) => write!(f, "native capability rejected: {reason}"),
        }
    }
}
impl Error for CompilerExecutionCapabilityErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Policy(e) => Some(e),
            Self::Profile(e) => Some(e),
            Self::Io { .. } | Self::Rejected(_) => None,
        }
    }
}

pub(crate) trait Record<const N: usize>: Sized {
    const ROLE: CapabilityRole;
    fn bytes(&self) -> &[u8; N];
    fn retained_storage(&self) -> usize;
    // Reserve the decoded owner's returned charge immediately on this ledger.
    fn decode_retained(bytes: &[u8; N], budget: &mut Budget<'_>) -> Result<Self>;
}

pub(crate) struct NativeCapability<T, const N: usize> {
    pub(crate) record: T,
    pub(crate) image: SealedCapabilityImage,
}
impl<T: Record<N>, const N: usize> NativeCapability<T, N> {
    // Charge each descriptor its complete logical image even when a dup shares
    // the backing inode. This deliberately overcounts; it is not kernel RSS.
    pub(crate) const FILE_STORAGE: usize = size_of::<(File, Storage)>() + N;
    pub(crate) const RETAINED: usize = size_of::<(Self, Storage)>() + N;
    // At most 32 descriptor syscalls, weighted at 1024 logical work each, plus
    // fixed byte staging/comparison. Native decoding charges separately.
    pub(crate) const IO_WORK: usize = ENTRY_WORK + 32 * 1024 + 32 * N;
    pub(crate) const IO_STORAGE: usize =
        4 * Self::RETAINED + 4 * N + 4 * size_of::<std::fs::Metadata>() + 4096;

    pub(crate) const fn assert_layout<Public>() {
        use fe2o3_compiler_execution_protocol::CompilerExecutionAttestationStorageV2;
        use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as LedgerIdentity;
        assert!(size_of::<Public>() == size_of::<Self>());
        assert!(size_of::<(Public, Storage)>() <= Self::RETAINED - N);
        assert!(Self::RETAINED >= Self::FILE_STORAGE);
        assert!(
            Self::RETAINED >= size_of::<T>() + size_of::<CompilerExecutionAttestationStorageV2>()
        );
        assert!(Self::IO_STORAGE >= Self::RETAINED);
        assert!(T::ROLE.memfd_name.len() < 128);
        assert!(
            8 * size_of::<CompilerExecutionCapabilityErrorV2>()
                + 64 * size_of::<usize>()
                + size_of::<LedgerIdentity>()
                + size_of::<std::result::Result<(), Resource>>()
                + 2 * size_of::<bool>()
                + 512
                + envelope_overhead::<(Public, Storage), CompilerExecutionCapabilityErrorV2>()
                + envelope_overhead::<(File, Storage), CompilerExecutionCapabilityErrorV2>()
                + envelope_overhead::<(), CompilerExecutionCapabilityErrorV2>()
                + envelope_overhead::<bool, CompilerExecutionCapabilityErrorV2>()
                <= 4096
        );
    }

    pub(crate) fn scope<R>(
        budget: &mut Budget<'_>,
        floor: usize,
        operation: impl FnOnce(&mut Budget<'_>) -> Result<R>,
    ) -> Result<R> {
        budget.with_prepaid_scope(
            floor,
            ENTRY_WORK,
            Self::IO_WORK,
            Self::IO_STORAGE,
            operation,
        )
    }

    pub(crate) fn create(record: T, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        let floor = record.retained_storage();
        Self::scope(budget, floor, |_| {
            let admitted = Self::create_inner(record)?;
            Ok((
                admitted,
                Storage(
                    Self::RETAINED
                        .checked_sub(floor)
                        .ok_or(Resource::Accounting)?,
                ),
            ))
        })
    }

    pub(crate) fn create_inner(record: T) -> Result<Self> {
        let image = SealedCapabilityImage::create_fixed(record.bytes(), T::ROLE)?;
        let admitted = Self { record, image };
        admitted.check()?;
        Ok(admitted)
    }

    /// Consumes the prepaid File reservation, returning only capability growth.
    pub(crate) fn from_file(image: File, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        Self::scope(budget, Self::FILE_STORAGE, |budget| {
            let image = SealedCapabilityImage::from_file_fixed::<N>(image, T::ROLE)?;
            Ok((
                Self::decode_image(image, budget)?,
                Storage(Self::RETAINED - Self::FILE_STORAGE),
            ))
        })
    }

    /// Borrows an inherited source descriptor. Its reservation stays live;
    /// the distinct retained descriptor/capability returns the FULL charge.
    pub(crate) fn from_inherited_at(fd: RawFd, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        Self::scope(budget, Self::FILE_STORAGE, |budget| {
            let image = SealedCapabilityImage::from_inherited_fixed::<N>(fd, T::ROLE)?;
            Ok((Self::decode_image(image, budget)?, Storage(Self::RETAINED)))
        })
    }

    fn decode_image(image: SealedCapabilityImage, budget: &mut Budget<'_>) -> Result<Self> {
        let bytes = image.read_fixed::<N>()?;
        let record = T::decode_retained(&bytes, budget)?;
        Ok(Self { record, image })
    }

    fn check(&self) -> Result<()> {
        // The immutable in-memory record was admitted by its nominal V2
        // constructor/decoder. Byte equality therefore needs no second decode.
        if &self.image.read_fixed::<N>()? != self.record.bytes() {
            return Err(CompilerExecutionCapabilityErrorV2::Rejected(
                "sealed image bytes changed",
            ));
        }
        Ok(())
    }

    pub(crate) fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
        Self::scope(budget, Self::RETAINED, |_| self.check())
    }

    pub(crate) fn try_clone_for_transfer(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(File, Storage)> {
        Self::scope(budget, Self::RETAINED, |_| {
            self.check()?;
            Ok((self.image.clone_fixed()?, Storage(Self::FILE_STORAGE)))
        })
    }
}

pub(crate) const fn envelope_overhead<T, E>() -> usize {
    size_of::<std::result::Result<T, E>>().saturating_sub(size_of::<T>())
        + size_of::<std::thread::Result<std::result::Result<T, E>>>().saturating_sub(size_of::<T>())
}

#[cfg(test)]
#[path = "native_capability_tests.rs"]
pub(crate) mod tests;
