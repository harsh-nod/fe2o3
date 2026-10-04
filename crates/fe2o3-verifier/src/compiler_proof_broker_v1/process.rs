use std::{
    fs::File,
    io,
    os::{
        fd::{AsRawFd, OwnedFd},
        unix::fs::MetadataExt,
    },
    path::Path,
};

use fe2o3_process_identity::{
    EXACT_IMMUTABLE_MEMFD_SEALS_V1, measure_executable_sha256_v3, pidfd::ReceivedProcessPidfdV1,
};

use super::{
    Result, require,
    transport::{Credentials, FileSnapshot},
};

pub(super) struct Peer {
    process: ReceivedProcessPidfdV1,
    pub(super) credentials: Credentials,
    image: File,
    snapshot: FileSnapshot,
}

impl Peer {
    pub(super) fn admit(
        original: OwnedFd,
        credentials: Credentials,
        image_sha256: [u8; 32],
    ) -> Result<Self> {
        require(
            credentials.0 > 0 && credentials.1 > 0 && credentials.2 > 0,
            "compiler-proof peer credentials",
        )?;
        let process = ReceivedProcessPidfdV1::admit_received(original, credentials.0)
            .map_err(io::Error::other)?;
        let image = File::open(format!("/proc/{}/exe", credentials.0))?;
        let snapshot = FileSnapshot::capture(&image)?;
        Self::check_image(&image, credentials)?;
        require(
            measure_executable_sha256_v3(Path::new(&format!(
                "/proc/self/fd/{}",
                image.as_raw_fd()
            )))
            .map_err(io::Error::other)?
                == image_sha256,
            "compiler-proof peer executable differs from admitted compiler closure",
        )?;
        let value = Self {
            process,
            credentials,
            image,
            snapshot,
        };
        value.revalidate()?;
        Ok(value)
    }

    fn check_image(image: &File, credentials: Credentials) -> Result<()> {
        let m = image.metadata()?;
        require(
            m.is_file()
                && m.nlink() == 0
                && m.len() > 0
                && m.len() <= 512 * 1024 * 1024
                && m.mode() & 0o111 != 0
                && m.mode() & 0o7000 == 0
                && (m.uid(), m.gid()) == (credentials.1, credentials.2)
                && rustix::fs::fcntl_get_seals(image)? == EXACT_IMMUTABLE_MEMFD_SEALS_V1
                && rustix::fs::fcntl_getfl(image)? & rustix::fs::OFlags::ACCMODE
                    == rustix::fs::OFlags::RDONLY
                && rustix::io::fcntl_getfd(image)? == rustix::io::FdFlags::CLOEXEC,
            "compiler-proof peer image is not the protected sealed executable",
        )
    }

    pub(super) fn revalidate(&self) -> Result<()> {
        self.process.revalidate().map_err(io::Error::other)?;
        Self::check_image(&self.image, self.credentials)?;
        let current = File::open(format!("/proc/{}/exe", self.credentials.0))?;
        Self::check_image(&current, self.credentials)?;
        require(
            FileSnapshot::capture(&self.image)? == self.snapshot
                && FileSnapshot::capture(&current)? == self.snapshot,
            "compiler-proof peer executable changed",
        )?;
        self.process.revalidate().map_err(io::Error::other)
    }

    pub(super) fn require_start_time(&self, expected: u64) -> Result<()> {
        self.revalidate()?;
        require(
            self.process.start_time_ticks() == expected,
            "compiler-proof original peer start time changed",
        )
    }

    pub(super) fn require_parent(&self, expected: u32) -> Result<()> {
        use io::Read;
        self.process.revalidate().map_err(io::Error::other)?;
        let file = File::open(format!("/proc/{}/status", self.credentials.0))?;
        fe2o3_process_identity::pidfd::require_procfs(&file, "compiler-proof child status")
            .map_err(io::Error::other)?;
        let mut bytes = String::new();
        file.take(16 * 1024 + 1).read_to_string(&mut bytes)?;
        require(
            bytes.len() <= 16 * 1024,
            "compiler-proof child status bound",
        )?;
        let parents = bytes
            .lines()
            .filter_map(|line| line.strip_prefix("PPid:\t"))
            .collect::<Vec<_>>();
        require(
            parents.len() == 1 && parents[0] == expected.to_string(),
            "compiler-proof child is not the original wrapper's child",
        )?;
        self.process.revalidate().map_err(io::Error::other)
    }
}

#[cfg(test)]
mod tests;
