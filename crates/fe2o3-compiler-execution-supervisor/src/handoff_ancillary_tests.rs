use super::*;
use rustix::net::{AncillaryDrain, RecvAncillaryMessage};
use std::{
    mem::align_of,
    os::{
        fd::{AsRawFd, IntoRawFd},
        unix::fs::MetadataExt,
    },
    panic::{AssertUnwindSafe, catch_unwind},
};

struct Fixture {
    source: OwnedFd,
    _witness: OwnedFd,
    object: (u64, u64),
}

impl Fixture {
    fn new() -> Self {
        let (source, witness) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
        let stat = rustix::fs::fstat(&source).unwrap();
        Self {
            source,
            _witness: witness,
            object: (stat.st_dev, stat.st_ino),
        }
    }

    fn duplicate(&self) -> OwnedFd {
        rustix::io::fcntl_dupfd_cloexec(&self.source, 0).unwrap()
    }

    fn references(&self) -> usize {
        // Both original pipe ends stay open, so this unique inode cannot be
        // reused by parallel tests. No raw-FD or shared pidfd-inode counting.
        std::fs::read_dir("/proc/self/fd")
            .unwrap()
            .filter_map(|entry| std::fs::metadata(entry.ok()?.path()).ok())
            .filter(|metadata| (metadata.dev(), metadata.ino()) == self.object)
            .count()
    }
}

fn bytes(guard: &mut Guard) -> &mut [u8] {
    // SAFETY: the guard initialized its full backing and these private fixtures
    // only write initialized bytes and descriptors they exclusively own.
    unsafe { std::slice::from_raw_parts_mut(guard.backing.bytes.as_mut_ptr().cast::<u8>(), BYTES) }
}

fn record(guard: &mut Guard, offset: usize, level: i32, kind: i32, payload: &[u8]) -> usize {
    let length = HEADER + payload.len();
    let end = offset + length;
    assert!(end <= BYTES);
    let header = libc::cmsghdr {
        cmsg_len: length,
        cmsg_level: level,
        cmsg_type: kind,
    };
    let buffer = bytes(guard);
    // SAFETY: the complete header and payload fit in the initialized backing.
    unsafe {
        buffer
            .as_mut_ptr()
            .add(offset)
            .cast::<libc::cmsghdr>()
            .write_unaligned(header);
    }
    buffer[offset + HEADER..end].copy_from_slice(payload);
    align(end).unwrap()
}

fn descriptor_record(guard: &mut Guard, offset: usize, kind: i32, descriptor: OwnedFd) -> usize {
    let next = record(
        guard,
        offset,
        libc::SOL_SOCKET,
        kind,
        &descriptor.as_raw_fd().to_ne_bytes(),
    );
    // After successful fixture construction, transfer the descriptor to the
    // same owner that would receive it from the kernel, never a borrowed FD.
    let _ = descriptor.into_raw_fd();
    guard.armed = true;
    next
}

#[test]
fn backing_is_zeroed_aligned_and_includes_rustix_padding() {
    let mut guard = Guard::new();
    let (buffer, armed) = guard.parts();
    assert_eq!(
        buffer.len(),
        rustix::cmsg_space!(ScmRights(3), ScmRights(1))
    );
    assert_eq!((buffer.as_ptr() as usize) % align_of::<libc::cmsghdr>(), 0);
    assert!(!*armed);
    // SAFETY: Guard::new initializes every byte.
    assert!(buffer.iter().all(|byte| unsafe { byte.assume_init() } == 0));
    *armed = true;
    assert!(!guard.finish());
}

#[test]
fn auxiliary_only_and_multiple_records_close_exactly_once() {
    let fixture = Fixture::new();
    let baseline = fixture.references();
    let mut guard = Guard::new();
    let next = descriptor_record(&mut guard, 0, SCM_PIDFD, fixture.duplicate());
    descriptor_record(&mut guard, next, SCM_PIDFD, fixture.duplicate());
    assert_eq!(fixture.references(), baseline + 2);
    assert!(guard.cleanup());
    assert_eq!(fixture.references(), baseline);
    let held = fixture.duplicate();
    assert!(!guard.finish());
    assert_eq!(fixture.references(), baseline + 1);
    drop(held);
    assert_eq!(fixture.references(), baseline);
}

#[test]
fn mixed_auxiliary_and_rights_preserve_rustix_custody_in_both_orders() {
    for auxiliary_first in [false, true] {
        let fixture = Fixture::new();
        let baseline = fixture.references();
        let mut guard = Guard::new();
        let kinds = if auxiliary_first {
            [SCM_PIDFD, libc::SCM_RIGHTS]
        } else {
            [libc::SCM_RIGHTS, SCM_PIDFD]
        };
        let next = descriptor_record(&mut guard, 0, kinds[0], fixture.duplicate());
        let end = descriptor_record(&mut guard, next, kinds[1], fixture.duplicate());
        let mut right = None;
        // SAFETY: both canonical fixture records own private transferred FDs;
        // exactly this drain adopts SCM_RIGHTS, while the guard adopts SCM_PIDFD.
        for message in unsafe { AncillaryDrain::parse(&mut bytes(&mut guard)[..end]) } {
            match message {
                RecvAncillaryMessage::ScmRights(mut descriptors) => {
                    right = descriptors.next();
                    assert!(descriptors.next().is_none());
                }
                _ => panic!("unexpected fixture message"),
            }
        }
        assert!(right.is_some());
        assert!(guard.finish());
        assert_eq!(fixture.references(), baseline + 1);
        drop(right);
        assert_eq!(fixture.references(), baseline);
    }
}

#[test]
fn rights_only_are_neither_adopted_nor_rejected_by_guard() {
    let fixture = Fixture::new();
    let baseline = fixture.references();
    let mut guard = Guard::new();
    let end = descriptor_record(&mut guard, 0, libc::SCM_RIGHTS, fixture.duplicate());
    // SAFETY: the canonical fixture owns the transferred descriptor. Dropping
    // rustix's message iterator closes it before the auxiliary-only guard runs.
    for message in unsafe { AncillaryDrain::parse(&mut bytes(&mut guard)[..end]) } {
        drop(message);
    }
    assert_eq!(fixture.references(), baseline);
    assert!(!guard.finish());
    assert_eq!(fixture.references(), baseline);
}

#[test]
fn unknown_records_and_wrong_levels_never_adopt_integer_payloads() {
    let fixture = Fixture::new();
    let baseline = fixture.references();
    for (level, kind) in [(libc::SOL_SOCKET, 0x7fff), (-1, SCM_PIDFD)] {
        let mut guard = Guard::new();
        record(
            &mut guard,
            0,
            level,
            kind,
            &fixture.source.as_raw_fd().to_ne_bytes(),
        );
        guard.armed = true;
        assert!(guard.finish());
        assert_eq!(fixture.references(), baseline);
        rustix::fs::fstat(&fixture.source).unwrap();
    }
}

#[test]
fn malformed_auxiliary_lengths_and_negative_values_do_not_adopt() {
    for length in [0, FD_BYTES - 1, FD_BYTES + 1] {
        let mut guard = Guard::new();
        record(
            &mut guard,
            0,
            libc::SOL_SOCKET,
            SCM_PIDFD,
            &vec![0xff; length],
        );
        guard.armed = true;
        assert!(guard.finish());
    }
    let mut guard = Guard::new();
    record(
        &mut guard,
        0,
        libc::SOL_SOCKET,
        SCM_PIDFD,
        &(-1_i32).to_ne_bytes(),
    );
    guard.armed = true;
    assert!(guard.finish());
}

#[test]
fn complete_auxiliary_prefix_closes_before_malformed_tail_refusal() {
    let fixture = Fixture::new();
    let baseline = fixture.references();
    for length in [1, BYTES + 1, usize::MAX] {
        let mut guard = Guard::new();
        let next = descriptor_record(&mut guard, 0, SCM_PIDFD, fixture.duplicate());
        let header = libc::cmsghdr {
            cmsg_len: length,
            cmsg_level: libc::SOL_SOCKET,
            cmsg_type: SCM_PIDFD,
        };
        // SAFETY: only a complete header is written. Its invalid length must
        // be refused without examining any out-of-bounds or fabricated FD.
        unsafe {
            bytes(&mut guard)
                .as_mut_ptr()
                .add(next)
                .cast::<libc::cmsghdr>()
                .write_unaligned(header);
        }
        assert!(guard.finish());
        assert_eq!(fixture.references(), baseline);
    }
}

#[test]
fn zero_tail_with_nonzero_bytes_is_malformed_without_fd_guessing() {
    let mut guard = Guard::new();
    bytes(&mut guard)[BYTES - 1] = 1;
    guard.armed = true;
    assert!(guard.finish());
}

#[test]
fn unarmed_guard_does_not_adopt_private_borrowed_descriptor() {
    let fixture = Fixture::new();
    let baseline = fixture.references();
    let mut guard = Guard::new();
    record(
        &mut guard,
        0,
        libc::SOL_SOCKET,
        SCM_PIDFD,
        &fixture.source.as_raw_fd().to_ne_bytes(),
    );
    assert!(!guard.finish());
    assert_eq!(fixture.references(), baseline);
    rustix::fs::fstat(&fixture.source).unwrap();
}

#[test]
fn auxiliary_custody_closes_on_unwind_with_rustix_rights() {
    let fixture = Fixture::new();
    let baseline = fixture.references();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let mut guard = Guard::new();
            let next = descriptor_record(&mut guard, 0, SCM_PIDFD, fixture.duplicate());
            let end = descriptor_record(&mut guard, next, libc::SCM_RIGHTS, fixture.duplicate());
            // SAFETY: canonical private records own their descriptors as above.
            let mut messages = unsafe { AncillaryDrain::parse(&mut bytes(&mut guard)[..end]) };
            let _rights = messages.next().expect("one SCM_RIGHTS message");
            panic!("fixture unwind after successful receive");
        }))
        .is_err()
    );
    assert_eq!(fixture.references(), baseline);
}
