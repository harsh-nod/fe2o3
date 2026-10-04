use std::error::Error;
use std::fs::File;
use std::io::Write as _;
use std::os::fd::FromRawFd as _;

use rustix::fs::{MemfdFlags, Mode, SealFlags};

#[derive(Debug, Eq, PartialEq)]
struct Credentials {
    uid: u32,
    gid: u32,
    capabilities: [[u32; 3]; 2],
}

pub(super) fn run() -> Result<serde_json::Value, String> {
    probe().map_err(|error| format!("application auditor syscall probe failed: {error}"))?;
    Ok(serde_json::json!({
        "credentials": "inspected",
        "filesystem": "inspected",
        "sealed_image": "verified",
        "credential_changes": "EPERM",
        "other_memfd_flags": "EPERM",
        "new_sockets": "EPERM",
    }))
}

fn probe() -> Result<(), Box<dyn Error>> {
    let initial = credentials()?;
    for (number, expected) in [
        (libc::SYS_setfsuid, initial.uid),
        (libc::SYS_setfsgid, initial.gid),
    ] {
        for query in [u32::MAX as u64, u64::MAX] {
            // SAFETY: both encodings truncate to the documented nonmutating u32 query.
            assert_eq!(unsafe { libc::syscall(number, query) }, i64::from(expected));
        }
        for request in [u64::from(expected), (1_u64 << 32) | u64::from(expected)] {
            // Using the current ID distinguishes seccomp denial from a refused credential change.
            super::blocked_noncreation(
                "filesystem credential change",
                number,
                [request as usize, 0, 0, 0, 0, 0],
            )?;
        }
    }
    for (path, magic) in [
        ("/proc", 0x9fa0),
        ("/proc/thread-self/ns/mnt", 0x6e73_6673),
        ("/proc/thread-self/ns/user", 0x6e73_6673),
    ] {
        assert_eq!(rustix::fs::fstatfs(File::open(path)?)?.f_type, magic);
    }

    let mut image = File::from(rustix::fs::memfd_create(
        c"fe2o3-application-auditor-probe",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
    )?);
    rustix::fs::fchmod(&image, Mode::from_raw_mode(0o400))?;
    image.write_all(b"sealed auditor probe")?;
    image.flush()?;
    image.sync_all()?;
    let seals = SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK;
    rustix::fs::fcntl_add_seals(&image, seals)?;
    rustix::fs::fcntl_add_seals(&image, SealFlags::SEAL)?;
    assert_eq!(
        rustix::fs::fcntl_get_seals(&image)?,
        seals | SealFlags::SEAL
    );
    assert_eq!(
        image.write(b"mutation").unwrap_err().raw_os_error(),
        Some(libc::EPERM)
    );
    for name in [
        c"security.capability",
        c"system.posix_acl_access",
        c"system.posix_acl_default",
    ] {
        assert!(matches!(
            rustix::fs::fgetxattr(&image, name, &mut [0_u8; 128]),
            Err(rustix::io::Errno::NODATA | rustix::io::Errno::OPNOTSUPP)
        ));
    }
    for flags in [0, 1, 2, 3 | libc::MFD_HUGETLB, 3 | 0x10] {
        // SAFETY: the name is NUL-terminated; unexpected descriptors are closed before failure.
        let fd = unsafe {
            libc::syscall(
                libc::SYS_memfd_create,
                c"rejected-auditor-image".as_ptr(),
                flags,
            )
        };
        if fd >= 0 {
            // SAFETY: a successful memfd_create returned this unique descriptor.
            drop(unsafe { File::from_raw_fd(fd as i32) });
            return Err("application allowed unapproved memfd flags".into());
        }
        super::expect_eperm("unapproved memfd flags")?;
    }
    // Flags, like filesystem IDs, are kernel u32 arguments, regardless of upper register bits.
    // SAFETY: the static name is NUL-terminated and the low flags are the exact admitted set.
    let fd = unsafe {
        libc::syscall(
            libc::SYS_memfd_create,
            c"upper-word-auditor-image".as_ptr(),
            (1_u64 << 32) | 3,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: a successful memfd_create returned this unique descriptor.
    drop(unsafe { File::from_raw_fd(fd as i32) });
    assert!(matches!(
        rustix::net::socket(
            rustix::net::AddressFamily::UNIX,
            rustix::net::SocketType::SEQPACKET,
            None
        ),
        Err(rustix::io::Errno::PERM)
    ));
    assert!(matches!(
        rustix::net::socketpair(
            rustix::net::AddressFamily::UNIX,
            rustix::net::SocketType::SEQPACKET,
            rustix::net::SocketFlags::CLOEXEC,
            None
        ),
        Err(rustix::io::Errno::PERM)
    ));
    assert_eq!(credentials()?, initial);
    Ok(())
}

fn credentials() -> Result<Credentials, Box<dyn Error>> {
    let (mut uid, mut euid, mut suid) = (0, 0, 0);
    let (mut gid, mut egid, mut sgid) = (0, 0, 0);
    // SAFETY: each call writes three distinct scalar outputs.
    if unsafe { libc::getresuid(&mut uid, &mut euid, &mut suid) } != 0
        || unsafe { libc::getresgid(&mut gid, &mut egid, &mut sgid) } != 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    assert_eq!((uid, uid, gid, gid), (euid, suid, egid, sgid));
    #[repr(C)]
    struct Header {
        version: u32,
        pid: i32,
    }
    let mut header = Header {
        version: 0x2008_0522,
        pid: 0,
    };
    let mut capabilities = [[u32::MAX; 3]; 2];
    // SAFETY: the v3 capability ABI writes two records of three consecutive u32 fields.
    if unsafe { libc::syscall(libc::SYS_capget, &raw mut header, capabilities.as_mut_ptr()) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    assert_eq!(capabilities, [[0; 3]; 2]);
    Ok(Credentials {
        uid,
        gid,
        capabilities,
    })
}
