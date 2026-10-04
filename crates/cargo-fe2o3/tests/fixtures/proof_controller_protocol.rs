//! Inert, test-only fixed-descriptor protocol. Never application proof-lease authority.

use fe2o3_kernel_analysis::{
    PhysicalMachineAnalyzerIdentityV1, PhysicalMachineEffectWorkerPolicyV1,
    PhysicalMachineRuntimeClosureIdentityV1, PhysicalMachineToolchainIdentityV1,
    PhysicalMachineWorkerExecutableIdentityV1,
};
use rustix::{
    fs::{OFlags, SealFlags},
    net::{
        self, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
        SendAncillaryBuffer, SendFlags,
    },
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{IoSlice, IoSliceMut, Write},
    mem::MaybeUninit,
    os::{
        fd::{AsRawFd, BorrowedFd},
        unix::fs::{FileExt, MetadataExt, PermissionsExt},
    },
    time::Instant,
};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub const UID: u32 = 61000;
pub const GID: u32 = 61000;
pub const MAX_CONFIG: usize = 8192;
pub const MAX_PACKET: usize = 32 * 1024;
pub const CHUNK_BYTES: usize = 4096;
pub const ARTIFACT_NAMES: [&str; 7] = [
    "subject.bin",
    "proof.rs",
    "obligation.bin",
    "proof.receipt",
    "analysis.receipt",
    "analysis.request",
    "analysis.bundle",
];
pub const WORKER_PATH: &str = "/work/fe2o3-llvm-link-worker";
pub const RUNTIME_PATH: &str =
    "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5";

pub fn require(condition: bool, reason: &'static str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(reason.into())
    }
}

pub fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub version: u32,
    pub nonce: [u8; 32],
    pub parent_pid: i32,
    pub kernel: [u8; 32],
    pub envelope_hash: [u8; 32],
    pub envelope_len: u64,
    pub payload_hash: [u8; 32],
    pub payload_len: u64,
    pub executable_hash: [u8; 32],
    pub executable_len: u64,
    pub closure_hash: [u8; 32],
    pub closure_len: u64,
    pub analyzer: [u8; 32],
    pub toolchain: [u8; 32],
    pub runtime: [u8; 32],
}

impl Config {
    pub fn policy(&self) -> Result<PhysicalMachineEffectWorkerPolicyV1> {
        require(
            self.version == 1 && self.nonce != [0; 32] && self.parent_pid > 0,
            "invalid fixture config",
        )?;
        Ok(PhysicalMachineEffectWorkerPolicyV1::new(
            PhysicalMachineWorkerExecutableIdentityV1::from_parts(
                self.executable_hash,
                self.executable_len,
            ),
            PhysicalMachineRuntimeClosureIdentityV1::from_parts(
                self.closure_hash,
                self.closure_len,
            ),
            PhysicalMachineAnalyzerIdentityV1::from_sha256_bytes(self.analyzer),
            PhysicalMachineToolchainIdentityV1::from_sha256_bytes(self.toolchain),
        )?)
    }
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Content {
    pub sha256: [u8; 32],
    pub len: u64,
}

#[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub artifacts: [Content; 7],
    pub proof_key: [u8; 32],
    pub boundary: u8,
    pub signed_and_imported: bool,
    pub analyzer_authenticated: bool,
    pub authority: [bool; 4],
    pub pointers: [u64; 8],
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum Message {
    Ready {
        runtime: [u8; 32],
    },
    Start,
    Executing,
    Proved {
        evidence: Box<Evidence>,
    },
    ArtifactRequest {
        index: u8,
        offset: u64,
    },
    Artifact {
        index: u8,
        offset: u64,
        bytes: Vec<u8>,
    },
    Probe,
    Retained {
        digest: [u8; 32],
        pointers: [u64; 8],
    },
    Release,
    Released,
    Rejected {
        stage: String,
        detail: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Packet {
    pub nonce: [u8; 32],
    pub message: Message,
}

pub fn wait(fd: BorrowedFd<'_>, event: rustix::event::PollFlags, deadline: Instant) -> Result<()> {
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or("fixture deadline")?;
        let timeout = rustix::event::Timespec {
            tv_sec: remaining.as_secs().try_into()?,
            tv_nsec: remaining.subsec_nanos().into(),
        };
        let mut fds = [rustix::event::PollFd::new(&fd, event)];
        match rustix::event::poll(&mut fds, Some(&timeout)) {
            Ok(0) => return Err("fixture deadline".into()),
            Ok(_) => return Ok(()),
            Err(rustix::io::Errno::INTR) => (),
            Err(error) => return Err(error.into()),
        }
    }
}

pub fn send(
    fd: BorrowedFd<'_>,
    nonce: [u8; 32],
    message: Message,
    deadline: Instant,
) -> Result<()> {
    let bytes = serde_json::to_vec(&Packet { nonce, message })?;
    require(bytes.len() <= MAX_PACKET, "oversized fixture packet")?;
    loop {
        wait(fd, rustix::event::PollFlags::OUT, deadline)?;
        match net::sendmsg(
            fd,
            &[IoSlice::new(&bytes)],
            &mut SendAncillaryBuffer::new(&mut []),
            SendFlags::DONTWAIT | SendFlags::NOSIGNAL,
        ) {
            Ok(count) => return require(count == bytes.len(), "partial fixture send"),
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => (),
            Err(error) => return Err(error.into()),
        }
    }
}

pub fn receive(
    fd: BorrowedFd<'_>,
    sender: (i32, u32, u32),
    nonce: [u8; 32],
    deadline: Instant,
) -> Result<Message> {
    loop {
        wait(fd, rustix::event::PollFlags::IN, deadline)?;
        let mut bytes = vec![0; MAX_PACKET];
        let mut space =
            [MaybeUninit::uninit(); rustix::cmsg_space!(ScmCredentials(1), ScmRights(1))];
        let mut ancillary = RecvAncillaryBuffer::new(&mut space);
        let message = match net::recvmsg(
            fd,
            &mut [IoSliceMut::new(&mut bytes)],
            &mut ancillary,
            RecvFlags::DONTWAIT | RecvFlags::CMSG_CLOEXEC,
        ) {
            Ok(message) => message,
            Err(rustix::io::Errno::AGAIN | rustix::io::Errno::INTR) => continue,
            Err(error) => return Err(error.into()),
        };
        require(
            message.bytes > 0
                && message.bytes <= bytes.len()
                && (message.flags - ReturnFlags::CMSG_CLOEXEC).is_empty(),
            "truncated or empty fixture packet",
        )?;
        let mut credentials = None;
        for value in ancillary.drain() {
            match value {
                RecvAncillaryMessage::ScmCredentials(value) if credentials.is_none() => {
                    credentials = Some(value)
                }
                _ => return Err("unexpected fixture ancillary".into()),
            }
        }
        let credentials = credentials.ok_or("missing fixture sender credentials")?;
        require(
            (
                credentials.pid.as_raw_pid(),
                credentials.uid.as_raw(),
                credentials.gid.as_raw(),
            ) == sender,
            "wrong fixture sender",
        )?;
        let packet: Packet = serde_json::from_slice(&bytes[..message.bytes])?;
        require(packet.nonce == nonce, "wrong fixture nonce")?;
        return Ok(packet.message);
    }
}

pub fn read_sealed(file: &mut File, maximum: usize) -> Result<Box<[u8]>> {
    let metadata = file.metadata()?;
    require(
        metadata.is_file()
            && metadata.nlink() == 0
            && metadata.uid() == 0
            && metadata.gid() == 0
            && metadata.mode() & 0o7777 == 0o400
            && metadata.len() > 0
            && metadata.len() <= maximum as u64,
        "fixture input metadata",
    )?;
    let status = rustix::fs::fcntl_getfl(&*file)?;
    require(
        status & OFlags::ACCMODE == OFlags::RDONLY
            && !status.intersects(OFlags::PATH | OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT),
        "fixture input must be read-only",
    )?;
    let seals = SealFlags::SEAL | SealFlags::SHRINK | SealFlags::GROW | SealFlags::WRITE;
    require(
        rustix::fs::fcntl_get_seals(&*file)? == seals,
        "fixture input seals",
    )?;
    let link = std::fs::read_link(format!("/proc/self/fd/{}", file.as_raw_fd()))?;
    let link = link.as_os_str().as_encoded_bytes();
    require(
        (link.starts_with(b"/memfd:") || link.starts_with(b"memfd:"))
            && link.ends_with(b" (deleted)"),
        "fixture input must be memfd",
    )?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(metadata.len() as usize)?;
    bytes.resize(metadata.len() as usize, 0);
    file.read_exact_at(&mut bytes, 0)?;
    require(
        file.read_at(&mut [0], metadata.len())? == 0,
        "fixture input length",
    )?;
    let snapshot = |m: &std::fs::Metadata| {
        (
            m.dev(),
            m.ino(),
            m.mode(),
            m.uid(),
            m.gid(),
            m.nlink(),
            m.len(),
            m.mtime(),
            m.mtime_nsec(),
            m.ctime(),
            m.ctime_nsec(),
        )
    };
    require(
        snapshot(&metadata) == snapshot(&file.metadata()?)
            && rustix::fs::fcntl_getfl(&*file)? == status
            && rustix::fs::fcntl_get_seals(&*file)? == seals,
        "fixture input changed during capture",
    )?;
    Ok(bytes.into_boxed_slice())
}

pub fn admit_control(fd: BorrowedFd<'_>) -> Result<()> {
    require(
        net::sockopt::socket_type(fd)? == net::SocketType::SEQPACKET
            && net::sockopt::socket_domain(fd)? == net::AddressFamily::UNIX
            && !net::sockopt::socket_acceptconn(fd)?,
        "fixture control socket type",
    )?;
    require(
        rustix::fs::fcntl_getfl(fd)? == OFlags::RDWR | OFlags::NONBLOCK,
        "fixture control flags",
    )?;
    let unnamed = net::SocketAddrAny::from(net::SocketAddrUnix::new_unnamed());
    require(
        net::getsockname(fd)? == unnamed && net::getpeername(fd)? == Some(unnamed),
        "fixture control must be unnamed",
    )?;
    net::sockopt::set_socket_passcred(fd, true)?;
    require(
        net::sockopt::socket_passcred(fd)?,
        "fixture control credentials disabled",
    )
}

pub fn seal(bytes: &[u8], executable: bool) -> Result<File> {
    let mut file = File::from(rustix::fs::memfd_create(
        c"proof-controller-fixture",
        rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
    )?);
    file.write_all(bytes)?;
    file.set_permissions(std::fs::Permissions::from_mode(if executable {
        0o555
    } else {
        0o400
    }))?;
    rustix::fs::fcntl_add_seals(
        &file,
        SealFlags::SEAL | SealFlags::SHRINK | SealFlags::GROW | SealFlags::WRITE,
    )?;
    Ok(File::open(format!("/proc/self/fd/{}", file.as_raw_fd()))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{os::fd::AsFd, time::Duration};

    fn pair() -> (std::os::fd::OwnedFd, std::os::fd::OwnedFd) {
        net::socketpair(
            net::AddressFamily::UNIX,
            net::SocketType::SEQPACKET,
            net::SocketFlags::CLOEXEC | net::SocketFlags::NONBLOCK,
            None,
        )
        .unwrap()
    }

    #[test]
    fn credentials_nonce_and_unexpected_rights_are_checked() {
        let (first, second) = pair();
        admit_control(first.as_fd()).unwrap();
        admit_control(second.as_fd()).unwrap();
        let identity = (
            rustix::process::getpid().as_raw_pid(),
            rustix::process::getuid().as_raw(),
            rustix::process::getgid().as_raw(),
        );
        let deadline = Instant::now() + Duration::from_secs(2);
        send(first.as_fd(), [7; 32], Message::Start, deadline).unwrap();
        assert!(matches!(
            receive(second.as_fd(), identity, [7; 32], deadline).unwrap(),
            Message::Start
        ));
        send(first.as_fd(), [7; 32], Message::Start, deadline).unwrap();
        assert!(
            receive(second.as_fd(), identity, [8; 32], deadline)
                .unwrap_err()
                .to_string()
                .contains("nonce")
        );
        send(first.as_fd(), [7; 32], Message::Start, deadline).unwrap();
        assert!(
            receive(
                second.as_fd(),
                (identity.0, identity.1 + 1, identity.2),
                [7; 32],
                deadline
            )
            .unwrap_err()
            .to_string()
            .contains("sender")
        );
        let bytes = serde_json::to_vec(&Packet {
            nonce: [7; 32],
            message: Message::Start,
        })
        .unwrap();
        let file = File::open("/dev/null").unwrap();
        let rights = [file.as_fd()];
        let mut space = [MaybeUninit::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut ancillary = SendAncillaryBuffer::new(&mut space);
        assert!(ancillary.push(net::SendAncillaryMessage::ScmRights(&rights)));
        net::sendmsg(
            &first,
            &[IoSlice::new(&bytes)],
            &mut ancillary,
            SendFlags::NOSIGNAL,
        )
        .unwrap();
        assert!(
            receive(second.as_fd(), identity, [7; 32], deadline)
                .unwrap_err()
                .to_string()
                .contains("ancillary")
        );
    }

    #[test]
    fn only_nonblocking_seqpacket_endpoints_are_admitted() {
        for (kind, flags) in [
            (net::SocketType::STREAM, net::SocketFlags::NONBLOCK),
            (net::SocketType::SEQPACKET, net::SocketFlags::empty()),
        ] {
            let (fd, _peer) = net::socketpair(
                net::AddressFamily::UNIX,
                kind,
                flags | net::SocketFlags::CLOEXEC,
                None,
            )
            .unwrap();
            assert!(admit_control(fd.as_fd()).is_err());
        }
    }

    #[test]
    fn largest_artifact_chunk_fits_bounded_packet() {
        let packet = Packet {
            nonce: [255; 32],
            message: Message::Artifact {
                index: u8::MAX,
                offset: u64::MAX,
                bytes: vec![255; CHUNK_BYTES],
            },
        };
        assert!(serde_json::to_vec(&packet).unwrap().len() <= MAX_PACKET);
    }

    #[test]
    fn unknown_control_fields_are_rejected() {
        let mut value = serde_json::to_value(Packet {
            nonce: [1; 32],
            message: Message::Start,
        })
        .unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("authority".into(), serde_json::Value::Bool(true));
        assert!(serde_json::from_value::<Packet>(value).is_err());
    }
}
