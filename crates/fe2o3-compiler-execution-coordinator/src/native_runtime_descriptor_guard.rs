//! Descriptor checkpoint results, not a filesystem policy or runtime guard.
//!
//! The caller must bind entry and exit to the same original selected task and
//! hold every memory/file-table sharer throughout. Before stepping ANY open it
//! must separately protect source/output paths and pre-open device/creation/
//! truncation effects. A returned-descriptor check cannot undo those effects.
use super::{Budget, Entry, Error, Result, View, descriptors, with_descriptor};
use rustix::{fs, net};
use std::{fs::File, mem::size_of, os::fd::AsFd};

#[path = "native_runtime_descriptor_wire.rs"]
mod wire;
use wire::{CONTROL_BYTES, HEADER_BYTES, Header, MAX_IOVECS};

const LOCAL_WORK: usize = 32 + MAX_IOVECS * 32 + 12 * 1088;
const LOCAL_SCRATCH: usize = 4096 + MAX_IOVECS * 16 + 4 * size_of::<fs::Stat>();
pub(crate) const ENTRY_WORK: usize =
    LOCAL_WORK + View::DESCRIPTOR_WORK + descriptors::WORK + 2 * View::MEMORY_WORK;
pub(crate) const EXIT_WORK: usize =
    LOCAL_WORK + View::DESCRIPTOR_WORK + descriptors::WORK + 2 * View::MEMORY_WORK;
pub(crate) const SCRATCH: usize =
    LOCAL_SCRATCH + View::FRAME + descriptors::FRAME + size_of::<(File, usize)>();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OpenRequirement {
    pub(crate) directory: i32,
    pub(crate) path_address: u64,
    pub(crate) flags: u32,
    pub(crate) write_side_effects: bool,
}

/// Inert original syscall scalars for the caller's separate pre-open policy.
pub(crate) fn open_requirement(entry: Entry) -> Result<OpenRequirement> {
    let (directory, path_address, flags) = match entry.number {
        2 => (-100, entry.arguments[0], entry.arguments[1]),
        257 => (
            descriptor_scalar(entry.arguments[0])?,
            entry.arguments[1],
            entry.arguments[2],
        ),
        _ => return Err(Error::Invalid("not a selected native open syscall")),
    };
    let flags =
        u32::try_from(flags).map_err(|_| Error::Invalid("noncanonical native open flags"))?;
    if path_address == 0 || flags & 3 == 3 {
        return Err(Error::Invalid("invalid native open path or access mode"));
    }
    let write_side_effects = flags & 3 != 0 || flags & (0x40 | 0x200 | 0x40_0000) != 0;
    Ok(OpenRequirement {
        directory,
        path_address,
        flags,
        write_side_effects,
    })
}

/// This retains only observations, never a capability to execute or resume.
pub(crate) struct PendingDescriptorCheck {
    effect: Effect,
}
impl PendingDescriptorCheck {
    pub(crate) const STORAGE: usize = size_of::<Self>();
}

enum Effect {
    Open,
    Query,
    Receive {
        descriptor: i32,
        identity: (u64, u64),
        address: u64,
        header: Header,
    },
}

/// The returned plain observation must remain charged until consumed/dropped.
/// The original owner, not this value, associates the selected entry and exit.
pub(crate) fn prepare_descriptor_entry(
    entry: Entry,
    view: &View<'_, '_>,
    b: &mut Budget<'_>,
) -> Result<PendingDescriptorCheck> {
    b.with_prepaid_scope(0, 8, LOCAL_WORK, LOCAL_SCRATCH, |b| {
        let effect = match entry.number {
            2 | 257 => {
                open_requirement(entry)?;
                Effect::Open
            }
            16 => {
                let descriptor = descriptor_scalar(entry.arguments[0])?;
                with_descriptor(view, descriptor, b, |file, b| {
                    descriptors::inspect(file.as_fd(), b)?;
                    query_ioctl(file, entry.arguments[1])
                })?;
                Effect::Query
            }
            47 => {
                let descriptor = descriptor_scalar(entry.arguments[0])?;
                let flags = entry.arguments[2];
                if flags & !(wire::MSG_DONTWAIT | wire::MSG_CMSG_CLOEXEC) != 0 {
                    return Err(Error::Invalid("unsupported native recvmsg flags"));
                }
                let address = entry.arguments[1];
                let header = read_header(view, address, b)?;
                if !matches!(header.control_length, 0 | 32) {
                    return Err(Error::Invalid(
                        "unsupported native ancillary receive capacity",
                    ));
                }
                let identity = with_descriptor(view, descriptor, b, |file, b| {
                    descriptors::inspect(file.as_fd(), b)?;
                    receive_socket(file, header.control_length != 0, flags)
                })?;
                if header.control_length != 0 {
                    if header.vector_count > MAX_IOVECS as u64 {
                        return Err(Error::Invalid("native receive vector limit exceeded"));
                    }
                    let mut vectors = [0; MAX_IOVECS * 16];
                    let length = header.vector_count as usize * 16;
                    if length != 0 {
                        view.read_memory(header.vectors, &mut vectors[..length], b)?;
                    }
                    if !header.credential_layout(address, &vectors[..length]) {
                        return Err(Error::Invalid(
                            "native receive header/control aliases payload",
                        ));
                    }
                }
                Effect::Receive {
                    descriptor,
                    identity,
                    address,
                    header,
                }
            }
            299 => return Err(Error::Invalid("native recvmmsg imports are not admitted")),
            _ => {
                return Err(Error::Invalid(
                    "not a selected native descriptor checkpoint",
                ));
            }
        };
        Ok(PendingDescriptorCheck { effect })
    })
}

/// Inspect the actual kernel result before ANY application instruction or
/// file-table/memory sharer resumes. Refusal requires owned-tree retirement.
pub(crate) fn validate_descriptor_exit(
    pending: PendingDescriptorCheck,
    result: i64,
    view: &View<'_, '_>,
    b: &mut Budget<'_>,
) -> Result<()> {
    b.with_prepaid_scope(
        PendingDescriptorCheck::STORAGE,
        8,
        LOCAL_WORK,
        LOCAL_SCRATCH,
        |b| {
            if (-4095..0).contains(&result) {
                // An ancillary receive may install FDs and then fail a user-memory
                // copy. Only no-progress EINTR/EAGAIN is safe without inspection;
                // EFAULT and other errors retire the tree rather than hide imports.
                return match &pending.effect {
                    Effect::Receive { header, .. }
                        if header.control_length != 0 && !matches!(result, -4 | -11) =>
                    {
                        Err(Error::Invalid(
                            "failed ancillary receive may have installed descriptors",
                        ))
                    }
                    _ => Ok(()),
                };
            }
            if result < 0 {
                return Err(Error::Invalid("noncanonical native syscall result"));
            }
            match pending.effect {
                Effect::Open => {
                    let descriptor = i32::try_from(result).map_err(|_| {
                        Error::Invalid("native open result exceeds descriptor range")
                    })?;
                    with_descriptor(view, descriptor, b, |file, b| {
                        descriptors::inspect(file.as_fd(), b)?;
                        Ok(())
                    })
                }
                Effect::Query => Ok(()),
                Effect::Receive {
                    descriptor,
                    identity,
                    address,
                    header,
                } => {
                    let after = read_header(view, address, b)?;
                    if !header.stable_fields(after) {
                        return Err(Error::Invalid(
                            "native receive changed protected header fields",
                        ));
                    }
                    with_descriptor(view, descriptor, b, |file, b| {
                        descriptors::inspect(file.as_fd(), b)?;
                        if receive_socket(file, header.control_length != 0, wire::MSG_DONTWAIT)?
                            != identity
                        {
                            return Err(Error::Invalid("native receive socket identity changed"));
                        }
                        Ok(())
                    })?;
                    if header.control_length == 0 {
                        if after.control_length != 0 {
                            return Err(Error::Invalid(
                                "zero-capacity receive acquired ancillary data",
                            ));
                        }
                        return Ok(());
                    }
                    if result == 0
                        && after.name_length == 0
                        && after.control_length == 0
                        && after.flags & (0x08 | 0x20) == 0
                    {
                        return Ok(());
                    }
                    if after.name_length != 0 || !matches!(after.control_length, 28 | 32) {
                        return Err(Error::Invalid(
                            "native credential receive changed its bounded shape",
                        ));
                    }
                    let mut control = [0; CONTROL_BYTES];
                    view.read_memory(
                        header.control,
                        &mut control[..after.control_length as usize],
                        b,
                    )?;
                    if !wire::credential_control(after, &control) {
                        return Err(Error::Invalid(
                            "native receive contains rights, unknown or truncated control",
                        ));
                    }
                    Ok(())
                }
            }
        },
    )
}

fn read_header(view: &View<'_, '_>, address: u64, b: &mut Budget<'_>) -> Result<Header> {
    let mut bytes = [0; HEADER_BYTES];
    view.read_memory(address, &mut bytes, b)?;
    Ok(Header::decode(&bytes))
}

fn descriptor_scalar(value: u64) -> Result<i32> {
    let descriptor = value as i32;
    if value != descriptor as u32 as u64 && value != descriptor as i64 as u64 {
        return Err(Error::Invalid("noncanonical native descriptor scalar"));
    }
    Ok(descriptor)
}

fn receive_socket(file: &File, credentials: bool, flags: u64) -> Result<(u64, u64)> {
    let stat = fs::fstat(file).map_err(|source| Error::Io {
        operation: "inspect native receive socket",
        source,
    })?;
    let access = fs::fcntl_getfl(file).map_err(|source| Error::Io {
        operation: "inspect native receive access",
        source,
    })?;
    if fs::FileType::from_raw_mode(stat.st_mode) != fs::FileType::Socket
        || access.contains(fs::OFlags::PATH)
        || (!access.contains(fs::OFlags::NONBLOCK) && flags & wire::MSG_DONTWAIT == 0)
    {
        return Err(Error::Invalid(
            "native receive requires an actual nonblocking socket",
        ));
    }
    if credentials {
        let admitted_shape = (|| -> rustix::io::Result<bool> {
            Ok(
                net::sockopt::socket_domain(file)? == net::AddressFamily::UNIX
                    && net::sockopt::socket_type(file)? == net::SocketType::SEQPACKET
                    && net::sockopt::socket_passcred(file)?
                    && !net::sockopt::socket_acceptconn(file)?,
            )
        })()
        .map_err(|source| Error::Io {
            operation: "inspect native credential socket properties",
            source,
        })?;
        if !admitted_shape {
            return Err(Error::Invalid(
                "native credential receive socket shape is unsupported",
            ));
        }
    }
    Ok((stat.st_dev, stat.st_ino))
}

fn query_ioctl(file: &File, request: u64) -> Result<()> {
    let stat = fs::fstat(file).map_err(|source| Error::Io {
        operation: "inspect native ioctl descriptor",
        source,
    })?;
    let kind = fs::FileType::from_raw_mode(stat.st_mode);
    if !matches!(request, 0x541b | 0x5401 | 0x5413) {
        return Err(Error::Invalid("unsupported native ioctl request"));
    }
    // FIFO and null have no mutating implementation of these three queries;
    // the terminal probes may return ENOTTY, as ordinary isatty callers expect.
    if kind == fs::FileType::Fifo
        || (kind == fs::FileType::CharacterDevice
            && (fs::major(stat.st_rdev), fs::minor(stat.st_rdev)) == (1, 3))
        || descriptors::is_terminal(file.as_fd(), &stat)
    {
        return Ok(());
    }
    if request == 0x541b && kind == fs::FileType::Socket {
        let domain = net::sockopt::socket_domain(file).map_err(|source| Error::Io {
            operation: "inspect native ioctl socket domain",
            source,
        })?;
        if domain == net::AddressFamily::UNIX {
            return Ok(());
        }
    }
    Err(Error::Invalid(
        "native ioctl is not a supported data-only query",
    ))
}

#[cfg(test)]
#[path = "native_runtime_descriptor_guard_tests.rs"]
mod tests;
