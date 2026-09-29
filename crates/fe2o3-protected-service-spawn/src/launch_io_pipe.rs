//! Inert exact framing shared by native issuer waits and the launch scheduler.
use super::*;

/// Mechanical framing failure, with no child, descriptor or payload authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PipeFrameError {
    /// EOF arrived before the complete frame.
    Truncated,
    /// A byte followed the complete frame.
    Trailing,
    /// A non-retryable read error, retaining which protocol read failed.
    Io {
        /// Kernel errno from the single read attempt.
        source: Errno,
        /// Whether the read was the one-byte EOF probe.
        eof: bool,
    },
    /// A terminal frame cannot be read or returned twice.
    Finished,
}

/// Fixed inert bytes plus exact-EOF state. This performs no I/O of its own and
/// supplies no deadline, metering, pipe-shape, liveness or admission policy.
pub struct ExactPipeFrame<const N: usize> {
    bytes: [u8; N],
    used: usize,
    finished: bool,
}

impl<const N: usize> Default for ExactPipeFrame<N> {
    fn default() -> Self {
        Self {
            bytes: [0; N],
            used: 0,
            finished: false,
        }
    }
}

impl<const N: usize> ExactPipeFrame<N> {
    /// Calls read at most once, with the unfilled suffix or a one-byte EOF probe.
    /// Partial reads, EINTR and EAGAIN remain pending. Only exact EOF yields bytes;
    /// any other terminal outcome permanently prevents a subsequent success.
    pub fn read_with(
        &mut self,
        read: impl FnOnce(&mut [u8]) -> Result<usize, Errno>,
    ) -> Result<Option<[u8; N]>, PipeFrameError> {
        if self.finished {
            return Err(PipeFrameError::Finished);
        }
        let eof = self.used == N;
        let mut trailing = [0];
        let buffer = if eof {
            &mut trailing[..]
        } else {
            &mut self.bytes[self.used..]
        };
        let capacity = buffer.len();
        let result = match read(buffer) {
            Ok(0) if eof => Ok(Some(self.bytes)),
            Ok(0) => Err(PipeFrameError::Truncated),
            Ok(count) if !eof && count <= capacity => {
                self.used += count;
                Ok(None)
            }
            Ok(_) => Err(PipeFrameError::Trailing),
            Err(Errno::AGAIN | Errno::INTR) => Ok(None),
            Err(source) => Err(PipeFrameError::Io { source, eof }),
        };
        self.finished = !matches!(result, Ok(None));
        result
    }
}

pub(super) fn read_nonblocking(fd: BorrowedFd<'_>, bytes: &mut [u8]) -> Result<usize, Errno> {
    use rustix::fs::{FileType, OFlags, fcntl_getfl, fstat};
    if FileType::from_raw_mode(fstat(fd)?.st_mode) != FileType::Fifo {
        return Err(Errno::INVAL);
    }
    let flags = fcntl_getfl(fd)?;
    if flags & OFlags::ACCMODE != OFlags::RDONLY
        || !flags.contains(OFlags::NONBLOCK)
        || flags.intersects(OFlags::APPEND | OFlags::ASYNC | OFlags::DIRECT | OFlags::PATH)
    {
        return Err(Errno::INVAL);
    }
    // A writer can enable packet mode on its own end. A suffix-sized read could
    // silently discard trailing bytes, so always include an oversize sentinel.
    let mut scratch = [0; MAX_PIPE_READY_BYTES + 1];
    let count = rustix::io::read(fd, &mut scratch)?;
    if count <= bytes.len() {
        bytes[..count].copy_from_slice(&scratch[..count]);
    }
    Ok(count)
}

impl<O: Observer, I: Io> Scheduler<'_, O, I> {
    pub(super) fn pipe<const N: usize>(
        &mut self,
        reader: BorrowedFd<'_>,
    ) -> Result<[u8; N], Error<O::Error>> {
        const PHASE: &str = "service-ready pipe";
        if !(1..=MAX_PIPE_READY_BYTES).contains(&N) {
            return Err(Failure::MalformedReadyTransfer.into());
        }
        let mut frame = ExactPipeFrame::<N>::default();
        for attempt in 0..MAX_PHASE_ATTEMPTS {
            self.begin(Boundary::ReadyPipe, PHASE)?;
            let result = frame.read_with(|bytes| self.io.pipe(reader, bytes));
            self.deadline(PHASE)?;
            match result {
                Ok(Some(bytes)) => return Ok(bytes),
                Ok(None) => {
                    if attempt + 1 == MAX_PHASE_ATTEMPTS {
                        break;
                    }
                    self.progress(PHASE)?;
                }
                Err(PipeFrameError::Io { source, eof }) => {
                    return Err(Failure::Io {
                        operation: if eof {
                            "read service-ready pipe EOF"
                        } else {
                            "read service-ready pipe"
                        },
                        source,
                    }
                    .into());
                }
                Err(_) => return Err(Failure::MalformedReadyTransfer.into()),
            }
        }
        Err(Failure::Timeout(PHASE).into())
    }
}

#[cfg(test)]
#[path = "launch_io_pipe_tests.rs"]
mod tests;
