use crate::{
    native_capability::{NativeCapability, Record, Result, Storage},
    sealed_image::CapabilityRole,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V2 as BYTES,
    CompilerExecutionServiceLaunchManifestV2 as Manifest,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::{fs::File, os::fd::RawFd};

type Capability = NativeCapability<Manifest, BYTES>;
impl Record<BYTES> for Manifest {
    const ROLE: CapabilityRole = CapabilityRole {
        name: "native compiler-execution service launch capability",
        memfd_name: "fe2o3-compiler-execution-service-launch-v2",
    };
    fn bytes(&self) -> &[u8; BYTES] {
        self.canonical_bytes()
    }
    fn retained_storage(&self) -> usize {
        self.retained_storage()
    }
    fn decode_retained(bytes: &[u8; BYTES], budget: &mut Budget<'_>) -> Result<Self> {
        let (manifest, storage) = Self::decode(bytes, budget)?;
        budget.reserve_storage(storage.additional_storage())?;
        Ok(manifest)
    }
}

/// Move-only sealed identity frame, using the unchanged launch wire. Structural
/// admission is not a native policy match or any process/readiness authority.
/// Consumers must validate the exact pinned native policy before use.
///
/// Inputs stay prepaid on the same ledger and all operations restore entry
/// storage. Reserve each returned delta before retention; retire full owner
/// charges after drop/transfer. On consuming failure retire the dropped input
/// reservation after return. No external Command inheritance callback is installed.
///
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionServiceLaunchCapabilityV2;
/// fn duplicate(value: CompilerExecutionServiceLaunchCapabilityV2) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_closure_capability::CompilerExecutionServiceLaunchCapabilityV2;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<CompilerExecutionServiceLaunchCapabilityV2>();
/// ```
pub struct CompilerExecutionServiceLaunchCapabilityV2(Capability);
impl CompilerExecutionServiceLaunchCapabilityV2 {
    /// Fixed outer work; admission additionally charges the shared frame decoder.
    pub const IO_WORK: usize = Capability::IO_WORK;
    /// Additional outer logical scratch, not generated stack, allocator or RSS bounds.
    pub const IO_STORAGE: usize = Capability::IO_STORAGE;
    pub const FILE_STORAGE: usize = Capability::FILE_STORAGE;

    pub fn create(manifest: Manifest, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        Capability::create(manifest, budget).map(|(value, storage)| (Self(value), storage))
    }
    pub fn from_file(image: File, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        Capability::from_file(image, budget).map(|(value, storage)| (Self(value), storage))
    }
    /// Borrows a non-CLOEXEC fd >= 3, prepaid at FILE_STORAGE. Does not own or
    /// close that source slot; its caller must retain it unchanged during admission.
    pub fn from_inherited_at(fd: RawFd, budget: &mut Budget<'_>) -> Result<(Self, Storage)> {
        Capability::from_inherited_at(fd, budget).map(|(value, storage)| (Self(value), storage))
    }
    pub const fn manifest(&self) -> &Manifest {
        &self.0.record
    }
    pub fn revalidate(&self, budget: &mut Budget<'_>) -> Result<()> {
        self.0.revalidate(budget)
    }
    pub fn try_clone_for_transfer(&self, budget: &mut Budget<'_>) -> Result<(File, Storage)> {
        self.0.try_clone_for_transfer(budget)
    }
    /// Revalidates this owner and a borrowed CLOEXEC transfer against the original
    /// sealed object and bytes. Prepay `retained_storage() + FILE_STORAGE` on
    /// the same ledger. Charges IO_WORK and IO_STORAGE scratch, restoring entry
    /// storage without creating, retaining, closing or moving either descriptor.
    /// This does not establish a native policy match or launch authority.
    pub fn validate_transfer(&self, transfer: &File, budget: &mut Budget<'_>) -> Result<()> {
        self.0.validate_transfer(transfer, budget)
    }
    pub const fn retained_storage(&self) -> usize {
        Capability::RETAINED
    }
}

const _: () = {
    Capability::assert_layout::<CompilerExecutionServiceLaunchCapabilityV2>();
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native_capability::{
        CompilerExecutionCapabilityErrorV2 as Error,
        tests::{failure, policy, run, sealed, transfer_boundaries},
    };
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_STORAGE_V2 as SCRATCH,
        COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V2 as WORK,
        CompilerExecutionClientProcessIdentityV1 as Client,
        CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
        CompilerExecutionServiceLaunchManifestErrorV2 as ManifestError,
    };
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceErrorV1 as Resource,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    use std::os::{fd::AsRawFd, unix::fs::MetadataExt};

    fn manifest() -> Manifest {
        let p = policy(7);
        let mut w = Work::new(WORK);
        let mut b = Budget::new(&mut w, 100_000);
        b.reserve_storage(p.retained_storage()).unwrap();
        Manifest::new(
            Client::new(1234, 5678, 9012).unwrap(),
            Service::new(6001, 7001).unwrap(),
            &p,
            &mut b,
        )
        .unwrap()
        .0
    }

    #[test]
    fn native_manifest_survives_sealed_transfer_and_borrowed_inheritance() {
        let m = manifest();
        let expected = *m.canonical_bytes();
        let mut w = Work::new(1_000_000);
        let mut b = Budget::new(&mut w, 1_000_000);
        b.reserve_storage(m.retained_storage()).unwrap();
        let (cap, charge) = CompilerExecutionServiceLaunchCapabilityV2::create(m, &mut b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(b.storage(), cap.retained_storage());
        let (file, charge) = cap.try_clone_for_transfer(&mut b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        let inode = file.metadata().unwrap().ino();
        let floor = b.storage();
        cap.validate_transfer(&file, &mut b).unwrap();
        assert_eq!(b.storage(), floor);
        transfer_boundaries(floor, Capability::IO_WORK, Capability::IO_STORAGE, |b| {
            cap.validate_transfer(&file, b)
        });
        let denied =
            CompilerExecutionServiceLaunchCapabilityV2::from_inherited_at(file.as_raw_fd(), &mut b);
        assert!(denied.is_err());
        assert_eq!(b.storage(), floor);
        rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
        let (inherited, charge) =
            CompilerExecutionServiceLaunchCapabilityV2::from_inherited_at(file.as_raw_fd(), &mut b)
                .unwrap();
        assert_eq!(charge.additional_storage(), inherited.retained_storage());
        b.reserve_storage(charge.additional_storage()).unwrap();
        inherited.revalidate(&mut b).unwrap();
        assert_eq!(inherited.manifest().canonical_bytes(), &expected);
        assert_eq!(file.metadata().unwrap().ino(), inode);
        assert!(rustix::io::fcntl_getfd(&file).unwrap().is_empty());
        drop(inherited);
        b.release_storage(Capability::RETAINED).unwrap();
        rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::CLOEXEC).unwrap();
        drop(cap);
        b.release_storage(Capability::RETAINED).unwrap();
        let (recovered, charge) =
            CompilerExecutionServiceLaunchCapabilityV2::from_file(file, &mut b).unwrap();
        b.reserve_storage(charge.additional_storage()).unwrap();
        assert_eq!(b.storage(), recovered.retained_storage());
        assert_eq!(recovered.manifest().canonical_bytes(), &expected);
        drop(recovered);
        b.release_storage(Capability::RETAINED).unwrap();
        assert_eq!(b.storage(), 0);
    }

    #[test]
    fn launch_admission_nested_decoder_uses_exact_shared_budget() {
        let bytes = *manifest().canonical_bytes();
        let floor = Capability::FILE_STORAGE;
        let peak = floor + Capability::IO_STORAGE + SCRATCH;
        for mode in 0..3 {
            let (result, work, live, observed_peak) = run(
                floor,
                Capability::IO_WORK + WORK - usize::from(mode == 0),
                peak - usize::from(mode == 1),
                |b| CompilerExecutionServiceLaunchCapabilityV2::from_file(sealed(&bytes), b),
            );
            assert_eq!(live, floor);
            match mode {
                0 => {
                    assert!(matches!(
                        failure(result),
                        Error::Launch(ManifestError::Resource(Resource::Work(_)))
                    ));
                    assert_eq!(work, Capability::IO_WORK + 8);
                }
                1 => {
                    assert!(matches!(
                        failure(result),
                        Error::Launch(ManifestError::Resource(Resource::Storage(_)))
                    ));
                    assert_eq!(work, Capability::IO_WORK + WORK);
                }
                _ => {
                    let (cap, delta) = result.unwrap();
                    assert_eq!(delta.additional_storage() + floor, cap.retained_storage());
                    assert_eq!(observed_peak, peak);
                }
            }
        }
    }

    #[test]
    fn structural_admission_does_not_claim_a_native_policy_match() {
        let p = crate::native_capability::tests::legacy_policy(7);
        let old = fe2o3_compiler_execution_protocol::CompilerExecutionServiceLaunchManifestV1::new(
            Client::new(1234, 5678, 9012).unwrap(),
            Service::new(6001, 7001).unwrap(),
            &p,
        );
        let p = policy(7);
        let mut w = Work::new(1_000_000);
        let mut b = Budget::new(&mut w, 1_000_000);
        b.reserve_storage(Capability::FILE_STORAGE + p.retained_storage())
            .unwrap();
        let (cap, delta) = CompilerExecutionServiceLaunchCapabilityV2::from_file(
            sealed(old.canonical_bytes()),
            &mut b,
        )
        .unwrap();
        b.reserve_storage(delta.additional_storage()).unwrap();
        assert!(!cap.manifest().matches_policy(&p, &mut b).unwrap());
        let mut bad = *old.canonical_bytes();
        bad[80] ^= 1;
        b.reserve_storage(Capability::FILE_STORAGE).unwrap();
        assert!(matches!(
            failure(CompilerExecutionServiceLaunchCapabilityV2::from_file(
                sealed(&bad),
                &mut b
            )),
            Error::Launch(_)
        ));
    }

    #[test]
    fn identical_launch_bytes_do_not_allow_sealed_object_replacement() {
        let m = manifest();
        let bytes = *m.canonical_bytes();
        let (result, _, _, _) = run(m.retained_storage(), 1_000_000, 1_000_000, |b| {
            CompilerExecutionServiceLaunchCapabilityV2::create(m, b)
        });
        let (mut cap, _) = result.unwrap();
        let (transfer, _) = run(cap.retained_storage(), 1_000_000, 1_000_000, |b| {
            cap.try_clone_for_transfer(b)
        })
        .0
        .unwrap();
        let replacement = sealed(&bytes);
        let floor = cap.retained_storage() + 2 * Capability::FILE_STORAGE;
        let (result, _, live, _) = run(floor, Capability::IO_WORK, 1_000_000, |b| {
            cap.validate_transfer(&replacement, b)
        });
        assert!(matches!(
            failure(result),
            Error::Rejected("sealed image identity or length changed")
        ));
        assert_eq!(live, floor);
        drop(replacement);
        cap.0.image.replace_file_for_test(sealed(&bytes));
        let (result, _, live, _) = run(cap.retained_storage(), 1_000_000, 1_000_000, |b| {
            cap.revalidate(b)
        });
        assert!(result.is_err());
        assert_eq!(live, cap.retained_storage());
        let floor = cap.retained_storage() + Capability::FILE_STORAGE;
        let (result, _, live, _) = run(floor, Capability::IO_WORK, 1_000_000, |b| {
            cap.validate_transfer(&transfer, b)
        });
        assert!(matches!(
            failure(result),
            Error::Rejected("sealed image identity or length changed")
        ));
        assert_eq!(live, floor);
        assert_eq!(transfer.metadata().unwrap().len(), BYTES as u64);
    }
}
