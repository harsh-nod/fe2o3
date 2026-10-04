use std::error::Error;
use std::fmt;
use std::fs::File;
use std::io;
use std::time::Duration;

use rustix::fs::SealFlags;
use rustix::io::Errno;

pub const EXACT_IMMUTABLE_MEMFD_SEALS_V1: SealFlags = SealFlags::WRITE
    .union(SealFlags::GROW)
    .union(SealFlags::SHRINK)
    .union(SealFlags::SEAL);

const DATA_SEAL_BUSY_RETRIES: usize = 8;
const DATA_SEAL_BUSY_INITIAL_DELAY: Duration = Duration::from_millis(1);

/// Policy for an `F_SEAL_WRITE` collision with an externally observed writable mapping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImmutableMemfdBusyPolicyV1 {
    /// Treat any writable mapping as an immediate invariant violation.
    Reject,
    /// Allow a bounded quiescence interval before rejecting the image.
    ///
    /// The caller must authenticate the sealed bytes independently before granting authority.
    BoundedExternalObserverQuiescence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImmutableMemfdSealStageV1 {
    InitialInspection,
    BusyInspection,
    DataSeal,
    FinalSeal,
    FinalInspection,
}

#[derive(Debug)]
pub enum ImmutableMemfdSealErrorV1 {
    Syscall {
        stage: ImmutableMemfdSealStageV1,
        source: Errno,
    },
    InitialSealsPresent {
        actual: SealFlags,
    },
    PartialSealsAfterBusy {
        actual: SealFlags,
    },
    BusyRejected,
    BusyExhausted {
        attempts: usize,
    },
    FinalSealMismatch {
        actual: SealFlags,
    },
}

impl ImmutableMemfdSealErrorV1 {
    pub const fn stage(&self) -> ImmutableMemfdSealStageV1 {
        match self {
            Self::Syscall { stage, .. } => *stage,
            Self::InitialSealsPresent { .. } => ImmutableMemfdSealStageV1::InitialInspection,
            Self::PartialSealsAfterBusy { .. }
            | Self::BusyRejected
            | Self::BusyExhausted { .. } => ImmutableMemfdSealStageV1::DataSeal,
            Self::FinalSealMismatch { .. } => ImmutableMemfdSealStageV1::FinalInspection,
        }
    }
}

impl fmt::Display for ImmutableMemfdSealErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syscall { stage, source } => {
                write!(formatter, "immutable memfd {stage:?} failed: {source}")
            }
            Self::InitialSealsPresent { actual } => write!(
                formatter,
                "immutable memfd started with unexpected seals {actual:?}"
            ),
            Self::PartialSealsAfterBusy { actual } => write!(
                formatter,
                "immutable memfd data seal left unexpected partial seals {actual:?}"
            ),
            Self::BusyRejected => {
                formatter.write_str("immutable memfd data seal encountered a writable mapping")
            }
            Self::BusyExhausted { attempts } => write!(
                formatter,
                "immutable memfd data seal remained busy after {attempts} attempts"
            ),
            Self::FinalSealMismatch { actual } => write!(
                formatter,
                "immutable memfd has unexpected final seals {actual:?}"
            ),
        }
    }
}

impl Error for ImmutableMemfdSealErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Syscall { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<ImmutableMemfdSealErrorV1> for io::Error {
    fn from(error: ImmutableMemfdSealErrorV1) -> Self {
        let kind = match &error {
            ImmutableMemfdSealErrorV1::Syscall { source, .. } => {
                io::Error::from_raw_os_error(source.raw_os_error()).kind()
            }
            ImmutableMemfdSealErrorV1::BusyRejected
            | ImmutableMemfdSealErrorV1::BusyExhausted { .. } => io::ErrorKind::ResourceBusy,
            ImmutableMemfdSealErrorV1::InitialSealsPresent { .. }
            | ImmutableMemfdSealErrorV1::PartialSealsAfterBusy { .. }
            | ImmutableMemfdSealErrorV1::FinalSealMismatch { .. } => io::ErrorKind::InvalidData,
        };
        Self::new(kind, error)
    }
}

/// Applies the exact immutable `WRITE|GROW|SHRINK|SEAL` protocol to a fresh memfd.
///
/// Only `EBUSY` from the data-seal syscall is eligible for retry. Every failed attempt must leave
/// the initially empty seal set unchanged. `F_SEAL_SEAL` is attempted once, only after write
/// sealing succeeds, and the exact final set is verified before returning.
///
/// This is a low-level seal transition, not descriptor admission: callers must independently
/// establish the expected memfd origin, object type, access mode, descriptor flags, and content
/// identity before granting authority.
pub fn seal_immutable_memfd_v1(
    file: &File,
    busy_policy: ImmutableMemfdBusyPolicyV1,
) -> Result<(), ImmutableMemfdSealErrorV1> {
    seal_immutable_memfd_with_policy(
        || {
            rustix::fs::fcntl_add_seals(
                file,
                SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK,
            )
        },
        || rustix::fs::fcntl_get_seals(file),
        || rustix::fs::fcntl_add_seals(file, SealFlags::SEAL),
        busy_policy,
        DATA_SEAL_BUSY_RETRIES,
        DATA_SEAL_BUSY_INITIAL_DELAY,
        std::thread::sleep,
    )
}

fn seal_immutable_memfd_with_policy(
    mut add_data_seals: impl FnMut() -> Result<(), Errno>,
    mut get_seals: impl FnMut() -> Result<SealFlags, Errno>,
    mut add_final_seal: impl FnMut() -> Result<(), Errno>,
    busy_policy: ImmutableMemfdBusyPolicyV1,
    busy_retries: usize,
    mut retry_delay: Duration,
    mut wait: impl FnMut(Duration),
) -> Result<(), ImmutableMemfdSealErrorV1> {
    let initial = get_seals().map_err(|source| ImmutableMemfdSealErrorV1::Syscall {
        stage: ImmutableMemfdSealStageV1::InitialInspection,
        source,
    })?;
    if !initial.is_empty() {
        return Err(ImmutableMemfdSealErrorV1::InitialSealsPresent { actual: initial });
    }

    for attempt in 0..=busy_retries {
        match add_data_seals() {
            Ok(()) => break,
            Err(Errno::BUSY) => {
                let actual = get_seals().map_err(|source| ImmutableMemfdSealErrorV1::Syscall {
                    stage: ImmutableMemfdSealStageV1::BusyInspection,
                    source,
                })?;
                if !actual.is_empty() {
                    return Err(ImmutableMemfdSealErrorV1::PartialSealsAfterBusy { actual });
                }
                if busy_policy == ImmutableMemfdBusyPolicyV1::Reject {
                    return Err(ImmutableMemfdSealErrorV1::BusyRejected);
                }
                if attempt == busy_retries {
                    return Err(ImmutableMemfdSealErrorV1::BusyExhausted {
                        attempts: busy_retries + 1,
                    });
                }
                wait(retry_delay);
                retry_delay = retry_delay.saturating_mul(2);
            }
            Err(source) => {
                return Err(ImmutableMemfdSealErrorV1::Syscall {
                    stage: ImmutableMemfdSealStageV1::DataSeal,
                    source,
                });
            }
        }
    }

    add_final_seal().map_err(|source| ImmutableMemfdSealErrorV1::Syscall {
        stage: ImmutableMemfdSealStageV1::FinalSeal,
        source,
    })?;
    let actual = get_seals().map_err(|source| ImmutableMemfdSealErrorV1::Syscall {
        stage: ImmutableMemfdSealStageV1::FinalInspection,
        source,
    })?;
    if actual != EXACT_IMMUTABLE_MEMFD_SEALS_V1 {
        return Err(ImmutableMemfdSealErrorV1::FinalSealMismatch { actual });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::io::Write;
    use std::os::fd::AsRawFd;

    #[test]
    fn busy_retry_is_bounded_ordered_and_exact() {
        let data_seals = SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK;
        let attempts = Cell::new(0);
        let inspections = Cell::new(0);
        let final_calls = Cell::new(0);
        let waits = RefCell::new(Vec::new());
        let seals = Cell::new(SealFlags::empty());
        seal_immutable_memfd_with_policy(
            || {
                attempts.set(attempts.get() + 1);
                if attempts.get() < 3 {
                    Err(Errno::BUSY)
                } else {
                    seals.set(data_seals);
                    Ok(())
                }
            },
            || {
                inspections.set(inspections.get() + 1);
                Ok(seals.get())
            },
            || {
                final_calls.set(final_calls.get() + 1);
                seals.set(EXACT_IMMUTABLE_MEMFD_SEALS_V1);
                Ok(())
            },
            ImmutableMemfdBusyPolicyV1::BoundedExternalObserverQuiescence,
            4,
            Duration::from_millis(3),
            |delay| waits.borrow_mut().push(delay),
        )
        .unwrap();
        assert_eq!(attempts.get(), 3);
        assert_eq!(inspections.get(), 4);
        assert_eq!(final_calls.get(), 1);
        assert_eq!(
            waits.into_inner(),
            [Duration::from_millis(3), Duration::from_millis(6)]
        );
    }

    #[test]
    fn fail_closed_policy_branches_are_distinct() {
        let no_op = || Ok(());
        let error = seal_immutable_memfd_with_policy(
            no_op,
            || Err(Errno::IO),
            no_op,
            ImmutableMemfdBusyPolicyV1::Reject,
            0,
            Duration::ZERO,
            |_| {},
        )
        .unwrap_err();
        assert_eq!(error.stage(), ImmutableMemfdSealStageV1::InitialInspection);

        let error = seal_immutable_memfd_with_policy(
            no_op,
            || Ok(SealFlags::GROW),
            no_op,
            ImmutableMemfdBusyPolicyV1::Reject,
            0,
            Duration::ZERO,
            |_| {},
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ImmutableMemfdSealErrorV1::InitialSealsPresent { .. }
        ));

        let inspections = Cell::new(0);
        let error = seal_immutable_memfd_with_policy(
            || Err(Errno::BUSY),
            || {
                inspections.set(inspections.get() + 1);
                Ok(if inspections.get() == 1 {
                    SealFlags::empty()
                } else {
                    SealFlags::GROW
                })
            },
            no_op,
            ImmutableMemfdBusyPolicyV1::BoundedExternalObserverQuiescence,
            1,
            Duration::ZERO,
            |_| {},
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ImmutableMemfdSealErrorV1::PartialSealsAfterBusy { .. }
        ));

        let inspections = Cell::new(0);
        let error = seal_immutable_memfd_with_policy(
            || Err(Errno::BUSY),
            || {
                inspections.set(inspections.get() + 1);
                if inspections.get() == 1 {
                    Ok(SealFlags::empty())
                } else {
                    Err(Errno::IO)
                }
            },
            no_op,
            ImmutableMemfdBusyPolicyV1::BoundedExternalObserverQuiescence,
            1,
            Duration::ZERO,
            |_| {},
        )
        .unwrap_err();
        assert_eq!(error.stage(), ImmutableMemfdSealStageV1::BusyInspection);

        for (source, expected_stage) in [
            (Errno::PERM, ImmutableMemfdSealStageV1::DataSeal),
            (Errno::INVAL, ImmutableMemfdSealStageV1::DataSeal),
        ] {
            let error = seal_immutable_memfd_with_policy(
                || Err(source),
                || Ok(SealFlags::empty()),
                no_op,
                ImmutableMemfdBusyPolicyV1::BoundedExternalObserverQuiescence,
                8,
                Duration::ZERO,
                |_| {},
            )
            .unwrap_err();
            assert_eq!(error.stage(), expected_stage);
        }

        let error = seal_immutable_memfd_with_policy(
            || Err(Errno::BUSY),
            || Ok(SealFlags::empty()),
            no_op,
            ImmutableMemfdBusyPolicyV1::BoundedExternalObserverQuiescence,
            2,
            Duration::ZERO,
            |_| {},
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ImmutableMemfdSealErrorV1::BusyExhausted { attempts: 3 }
        ));

        let error = seal_immutable_memfd_with_policy(
            || Ok(()),
            || Ok(SealFlags::empty()),
            || Err(Errno::PERM),
            ImmutableMemfdBusyPolicyV1::Reject,
            0,
            Duration::ZERO,
            |_| {},
        )
        .unwrap_err();
        assert_eq!(error.stage(), ImmutableMemfdSealStageV1::FinalSeal);

        let inspections = Cell::new(0);
        let error = seal_immutable_memfd_with_policy(
            || Ok(()),
            || {
                inspections.set(inspections.get() + 1);
                if inspections.get() == 1 {
                    Ok(SealFlags::empty())
                } else {
                    Err(Errno::IO)
                }
            },
            || Ok(()),
            ImmutableMemfdBusyPolicyV1::Reject,
            0,
            Duration::ZERO,
            |_| {},
        )
        .unwrap_err();
        assert_eq!(error.stage(), ImmutableMemfdSealStageV1::FinalInspection);

        let inspections = Cell::new(0);
        let error = seal_immutable_memfd_with_policy(
            || Ok(()),
            || {
                inspections.set(inspections.get() + 1);
                Ok(SealFlags::empty())
            },
            || Ok(()),
            ImmutableMemfdBusyPolicyV1::Reject,
            0,
            Duration::ZERO,
            |_| {},
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ImmutableMemfdSealErrorV1::FinalSealMismatch { .. }
        ));
    }

    #[test]
    fn real_writable_mapping_quiesces_or_exhausts_without_partial_seals() {
        let fd = rustix::fs::memfd_create(
            "fe2o3-seal-busy-test",
            rustix::fs::MemfdFlags::CLOEXEC | rustix::fs::MemfdFlags::ALLOW_SEALING,
        )
        .unwrap();
        let mut image = File::from(fd);
        image.write_all(&[0_u8; 4096]).unwrap();
        // SAFETY: the live memfd supplies one initialized page and the mapping is released exactly
        // once below before `image` is dropped.
        let mapping = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                4096,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                image.as_raw_fd(),
                0,
            )
        };
        assert_ne!(mapping, libc::MAP_FAILED);

        let error =
            seal_immutable_memfd_v1(&image, ImmutableMemfdBusyPolicyV1::Reject).unwrap_err();
        assert!(matches!(error, ImmutableMemfdSealErrorV1::BusyRejected));
        assert!(rustix::fs::fcntl_get_seals(&image).unwrap().is_empty());

        let error = seal_immutable_memfd_with_policy(
            || {
                rustix::fs::fcntl_add_seals(
                    &image,
                    SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK,
                )
            },
            || rustix::fs::fcntl_get_seals(&image),
            || rustix::fs::fcntl_add_seals(&image, SealFlags::SEAL),
            ImmutableMemfdBusyPolicyV1::BoundedExternalObserverQuiescence,
            1,
            Duration::ZERO,
            |_| {},
        )
        .unwrap_err();
        assert!(matches!(
            error,
            ImmutableMemfdSealErrorV1::BusyExhausted { attempts: 2 }
        ));
        assert!(rustix::fs::fcntl_get_seals(&image).unwrap().is_empty());

        let waits = Cell::new(0);
        seal_immutable_memfd_with_policy(
            || {
                rustix::fs::fcntl_add_seals(
                    &image,
                    SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK,
                )
            },
            || rustix::fs::fcntl_get_seals(&image),
            || rustix::fs::fcntl_add_seals(&image, SealFlags::SEAL),
            ImmutableMemfdBusyPolicyV1::BoundedExternalObserverQuiescence,
            1,
            Duration::ZERO,
            |_| {
                waits.set(waits.get() + 1);
                // SAFETY: the first data-seal attempt proved this mapping is still live; one retry
                // means this callback cannot be invoked again after the successful `munmap`.
                assert_eq!(unsafe { libc::munmap(mapping, 4096) }, 0);
            },
        )
        .unwrap();
        assert_eq!(waits.get(), 1);
        assert_eq!(
            rustix::fs::fcntl_get_seals(&image).unwrap(),
            EXACT_IMMUTABLE_MEMFD_SEALS_V1
        );
    }
}
