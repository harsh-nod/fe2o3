use super::*;
use crate::native_capability::tests::{sealed, transfer_boundaries};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn header(native: bool) -> [u8; BYTES] {
    let mut bytes = [0; BYTES];
    bytes[..8].copy_from_slice(if native { b"F2O3CEP3" } else { b"F2O3CEP1" });
    bytes[8..10].copy_from_slice(&if native { 3u16 } else { 1u16 }.to_le_bytes());
    bytes[12..20].copy_from_slice(&(BYTES as u64).to_le_bytes());
    bytes
}

#[test]
fn sealed_family_inspection_has_exact_same_account_boundaries() {
    for native in [false, true] {
        let file = sealed(&header(native));
        rustix::fs::seek(&file, rustix::fs::SeekFrom::Start(17)).unwrap();
        transfer_boundaries(
            Policy::FILE_STORAGE,
            Policy::IO_WORK,
            Policy::IO_STORAGE,
            |b| {
                let observed = inspect_compiler_execution_policy_family_v1(&file, b)?;
                assert_eq!(
                    observed,
                    if native {
                        Family::NativeV3
                    } else {
                        Family::LegacyV1
                    }
                );
                assert_eq!(
                    rustix::fs::seek(&file, rustix::fs::SeekFrom::Current(0)).unwrap(),
                    17
                );
                Ok(())
            },
        );
        // Header selection does not validate the deliberately absent policy body.
        let mut work = Work::new(usize::MAX);
        let mut b = Budget::new(&mut work, 1024 * 1024);
        b.reserve_storage(Policy::FILE_STORAGE).unwrap();
        if native {
            assert!(Policy::from_file(file, &mut b).is_err());
        } else {
            assert!(crate::CompilerExecutionPolicyCapabilityV1::from_file(file).is_err());
        }
    }
}

#[test]
fn sealed_family_inspection_refuses_descriptor_and_header_mutations() {
    for case in 0..7 {
        let mut bytes = header(true);
        if case == 0 {
            bytes[7] = b'2';
            bytes[8] = 2;
        }
        if case == 1 {
            bytes[8] = 1;
        }
        let file = if case == 2 {
            File::from(
                rustix::fs::memfd_create("unsealed-family", rustix::fs::MemfdFlags::CLOEXEC)
                    .unwrap(),
            )
        } else if case == 3 {
            sealed(&bytes[..BYTES - 1])
        } else if case == 4 {
            File::open("/dev/null").unwrap()
        } else {
            sealed(&bytes)
        };
        if case == 5 {
            rustix::fs::fchmod(&file, rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR).unwrap();
        }
        if case == 6 {
            rustix::io::fcntl_setfd(&file, rustix::io::FdFlags::empty()).unwrap();
        }
        let mut work = Work::new(Policy::IO_WORK);
        let mut b = Budget::new(&mut work, Policy::FILE_STORAGE + Policy::IO_STORAGE);
        b.reserve_storage(Policy::FILE_STORAGE).unwrap();
        assert!(inspect_compiler_execution_policy_family_v1(&file, &mut b).is_err());
        assert_eq!(b.storage(), Policy::FILE_STORAGE);
        assert_eq!(b.work(), Policy::IO_WORK);
        assert!(file.metadata().is_ok());
    }
}
