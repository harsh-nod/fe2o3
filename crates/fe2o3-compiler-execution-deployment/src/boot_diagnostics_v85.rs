use std::os::fd::OwnedFd;

use rustix::fs::{OFlags, fcntl_getfl, fcntl_setfl};
use rustix::io::Errno;
use rustix::pipe::{PipeFlags, fcntl_getpipe_size, fcntl_setpipe_size, pipe_with};

use super::super::{
    DeploymentVerificationErrorKindV1, DeploymentVerificationErrorV1, invalid, io_error,
};

const MAX_BYTES: usize = 64 * 1024;
const PIPE_REQUEST_BYTES: usize = 4096;

pub(super) struct MachineStderrV85 {
    reader: OwnedFd,
    bytes: Vec<u8>,
    eof: bool,
    refused: bool,
}

impl MachineStderrV85 {
    pub(super) fn new() -> Result<(Self, OwnedFd), DeploymentVerificationErrorV1> {
        let (reader, writer) = pipe_with(PipeFlags::CLOEXEC)
            .map_err(|source| io_error("create bounded systemd machine stderr pipe", source))?;
        let size = fcntl_setpipe_size(&reader, PIPE_REQUEST_BYTES)
            .map_err(|source| io_error("bound systemd machine stderr pipe capacity", source))?;
        if size > MAX_BYTES
            || fcntl_getpipe_size(&reader).map_err(|source| {
                io_error("recheck systemd machine stderr pipe capacity", source)
            })? != size
        {
            return Err(invalid(
                DeploymentVerificationErrorKindV1::InvalidQualificationBoot,
                "systemd machine stderr pipe exceeds fixed capacity",
            ));
        }
        let flags = fcntl_getfl(&reader)
            .map_err(|source| io_error("inspect systemd machine stderr reader flags", source))?;
        fcntl_setfl(&reader, flags | OFlags::NONBLOCK)
            .map_err(|source| io_error("make systemd machine stderr reader nonblocking", source))?;
        Ok((
            Self {
                reader,
                bytes: Vec::with_capacity(MAX_BYTES),
                eof: false,
                refused: false,
            },
            writer,
        ))
    }

    pub(super) fn drain(&mut self) -> Result<(), DeploymentVerificationErrorV1> {
        if self.refused {
            return Err(self.failure("systemd machine stderr exceeds the fixed 64KiB bound"));
        }
        if self.eof {
            return Ok(());
        }
        let mut buffer = [0_u8; 4096];
        loop {
            let length = buffer.len().min(MAX_BYTES - self.bytes.len() + 1);
            match rustix::io::read(&self.reader, &mut buffer[..length]) {
                Ok(0) => {
                    self.eof = true;
                    return Ok(());
                }
                Ok(count) => {
                    if count > MAX_BYTES - self.bytes.len() {
                        self.refused = true;
                        return Err(
                            self.failure("systemd machine stderr exceeds the fixed 64KiB bound")
                        );
                    }
                    self.bytes.extend_from_slice(&buffer[..count]);
                }
                Err(Errno::AGAIN | Errno::INTR) => return Ok(()),
                Err(source) => return Err(io_error("read bounded systemd machine stderr", source)),
            }
        }
    }

    pub(super) fn prefix(&self) -> String {
        super::super::preflight::bounded_output_prefix_v85(&self.bytes)
    }

    pub(super) fn failure(&self, message: &'static str) -> DeploymentVerificationErrorV1 {
        invalid(
            DeploymentVerificationErrorKindV1::InvalidQualificationBoot,
            format!("{message}; stderr_prefix={}", self.prefix()),
        )
    }
}

#[cfg(test)]
#[path = "boot_diagnostics_v85_tests.rs"]
mod tests;
