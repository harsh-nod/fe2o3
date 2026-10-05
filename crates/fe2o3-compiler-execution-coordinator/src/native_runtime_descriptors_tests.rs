//! Actual borrowed descriptor mechanics only; no trace or compiler admission.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::{
    fs::{File, OpenOptions},
    os::fd::AsFd,
};

fn inspect_fd(file: impl AsFd) -> Result<()> {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, crate::native_launch::FILE_STORAGE + FRAME);
    budget
        .reserve_storage(crate::native_launch::FILE_STORAGE)
        .unwrap();
    inspect(file.as_fd(), &mut budget)
}

#[test]
fn native_descriptor_policy_accepts_data_files_pipes_and_sockets() {
    inspect_fd(tempfile::tempfile().unwrap()).unwrap();
    let (reader, writer) = rustix::pipe::pipe().unwrap();
    inspect_fd(reader).unwrap();
    inspect_fd(writer).unwrap();
    let (left, right) = rustix::net::socketpair(
        rustix::net::AddressFamily::UNIX,
        rustix::net::SocketType::STREAM,
        rustix::net::SocketFlags::CLOEXEC,
        None,
    )
    .unwrap();
    inspect_fd(left).unwrap();
    inspect_fd(right).unwrap();
}

#[test]
fn native_descriptor_policy_refuses_procfs_memory_and_descriptor_aliases() {
    let memory = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/proc/self/mem")
        .unwrap();
    assert!(matches!(inspect_fd(&memory), Err(Error::UnsupportedWriter)));
    let alias = io::fcntl_dupfd_cloexec(&memory, 0).unwrap();
    assert!(matches!(inspect_fd(alias), Err(Error::UnsupportedWriter)));
    inspect_fd(File::open("/proc/self/status").unwrap()).unwrap();
}

#[test]
fn native_descriptor_policy_accepts_null_but_refuses_other_writable_devices() {
    inspect_fd(OpenOptions::new().write(true).open("/dev/null").unwrap()).unwrap();
    let zero = OpenOptions::new().write(true).open("/dev/zero").unwrap();
    assert!(matches!(inspect_fd(zero), Err(Error::UnsupportedWriter)));
    inspect_fd(File::open("/dev/zero").unwrap()).unwrap();
}

#[test]
fn native_descriptor_policy_accepts_actual_terminal_data_interface() {
    let terminal = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/ptmx")
        .unwrap();
    assert!(rustix::termios::isatty(&terminal));
    inspect_fd(terminal).unwrap();
}

#[test]
fn native_descriptor_policy_refuses_unknown_and_control_filesystems() {
    for magic in [
        0,
        0x9fa0,
        0x62656572,
        0x64626720,
        0x74726163,
        0x73636673,
        0x09041934,
        0x63677270,
        0x27e0eb,
        u64::MAX,
    ] {
        assert!(
            !data_filesystem(magic),
            "unexpected writable filesystem {magic:x}"
        );
    }
    for magic in [
        0xef53, 0x9123683e, 0x58465342, 0x01021994, 0x858458f6, 0x794c7630, 0x6969, 0x65735546,
        0x2fc12fc1, 0xf2f52010, 0x00c36400, 0xff534d42, 0xfe534d42,
    ] {
        assert!(data_filesystem(magic));
    }
}

#[test]
fn native_descriptor_policy_has_exact_original_floor_work_and_scratch_boundaries() {
    let file = tempfile::tempfile().unwrap();
    let original = crate::native_launch::FILE_STORAGE;
    for (short_work, short_floor, short_scratch) in [(0, 0, 0), (1, 0, 0), (0, 1, 0), (0, 0, 1)] {
        let mut work = Work::new(WORK - short_work);
        let mut b = Budget::new(&mut work, original + FRAME - short_scratch);
        b.reserve_storage(original - short_floor).unwrap();
        let result = inspect(file.as_fd(), &mut b);
        match (short_work, short_floor, short_scratch) {
            (0, 0, 0) => {
                result.unwrap();
                assert_eq!(b.work(), WORK);
                assert_eq!(b.peak_storage(), original + FRAME);
            }
            (1, 0, 0) => assert!(matches!(result, Err(Error::Resource(Resource::Work(_))))),
            (0, 1, 0) => assert!(matches!(result, Err(Error::Resource(Resource::Accounting)))),
            (0, 0, 1) => assert!(matches!(result, Err(Error::Resource(Resource::Storage(_))))),
            _ => unreachable!(),
        }
        assert_eq!(b.storage(), original - short_floor);
        assert!(fs::fstat(&file).is_ok());
    }
}
