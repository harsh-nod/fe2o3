//! Whole-directory native publication. A legacy directory is never widened.
use super::super::{no_attributes, open_directory, same_object};
use super::*;
use rustix::fs::{AtFlags, Mode, OFlags, RawDir, RenameFlags};
use std::{fs::File, mem::MaybeUninit, os::unix::fs::MetadataExt};

const PREFIX: &str = ".proof-custodian-native-";
const DIRECTORY: &str = "proof-custodian";

fn inventory(
    directory: &File,
    maximum: usize,
    mut inspect: impl FnMut(&[u8]) -> io::Result<()>,
) -> io::Result<()> {
    // A separate open description preserves the retained directory's cursor.
    // RawDir performs one getdents attempt, unlike Dir's hidden EINTR retries.
    let scan = open_directory(directory, ".")?;
    require(same_object(directory, &scan)?, "native directory changed")?;
    let mut buffer = [MaybeUninit::uninit(); 4096];
    let mut entries = RawDir::new(&scan, &mut buffer);
    for index in 0..=maximum {
        let Some(entry) = entries.next() else {
            return Ok(());
        };
        require(index < maximum, "native directory inventory bound")?;
        let entry = entry?;
        let name = entry.file_name().to_bytes();
        if name != b"." && name != b".." {
            inspect(name)?;
        }
    }
    unreachable!("bounded inventory either ends or rejects")
}

fn validate(
    directory: &File,
    candidate: &Candidate,
    uid: u32,
    gid: u32,
    budget: &mut Budget<'_>,
) -> io::Result<()> {
    budget.charge_work(64 * 1024).map_err(other)?;
    let m = directory.metadata()?;
    require(
        m.uid() == uid && m.gid() == gid && m.mode() == libc::S_IFDIR | 0o755,
        "native configuration directory metadata differs",
    )?;
    no_attributes(directory)?;
    let mut seen = [false; 3];
    inventory(directory, 5, |name| {
        let index = RECORD_NAMES
            .iter()
            .position(|expected| expected.as_bytes() == name);
        let index =
            index.ok_or_else(|| io::Error::other("unexpected native configuration record"))?;
        require(!seen[index], "duplicate native configuration record")?;
        seen[index] = true;
        Ok(())
    })?;
    require(
        seen.into_iter().all(|present| present),
        "native deployment requires exactly three records; legacy coexistence/replacement refused",
    )?;
    for (name, expected) in RECORD_NAMES.into_iter().zip(candidate.records()) {
        budget.charge_work(expected.len() + 4096).map_err(other)?;
        let file = File::from(rustix::fs::openat(
            directory,
            name,
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            Mode::empty(),
        )?);
        let before = file.metadata()?;
        require(
            before.uid() == uid
                && before.gid() == gid
                && before.mode() == libc::S_IFREG | 0o444
                && before.nlink() == 1
                && before.len() == expected.len() as u64,
            "native configuration record metadata differs",
        )?;
        no_attributes(&file)?;
        budget.reserve_storage(expected.len()).map_err(other)?;
        let mut actual = vec![0; expected.len()];
        for (index, chunk) in actual.chunks_mut(64 * 1024).enumerate() {
            require(
                rustix::io::pread(&file, &mut *chunk, (index * 64 * 1024) as u64)? == chunk.len(),
                "native installed record single read was short",
            )?;
        }
        require(
            rustix::io::pread(&file, &mut [0; 1], expected.len() as u64)? == 0,
            "native installed record grew",
        )?;
        let after = file.metadata()?;
        let identity = |m: &std::fs::Metadata| {
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
            actual == expected && identity(&before) == identity(&after),
            "native installed record differs; refusing replacement",
        )?;
        drop(actual);
        budget.release_storage(expected.len()).map_err(other)?;
    }
    Ok(())
}

struct Staging<'a> {
    parent: &'a File,
    directory: File,
    name: String,
    published: bool,
}
impl Drop for Staging<'_> {
    fn drop(&mut self) {
        if !self.published {
            // Only our three names and original directory occurrence are eligible.
            for name in RECORD_NAMES {
                let _ = rustix::fs::unlinkat(&self.directory, name, AtFlags::empty());
            }
            if open_directory(self.parent, &self.name)
                .and_then(|other| same_object(&other, &self.directory))
                .unwrap_or(false)
            {
                let _ = rustix::fs::unlinkat(self.parent, &self.name, AtFlags::REMOVEDIR);
            }
        }
    }
}

pub(super) fn publish<'work>(
    parent: &File,
    candidate: &Candidate,
    uid: u32,
    gid: u32,
    budget: &mut Budget<'work>,
    mut revalidate: impl FnMut(&mut Budget<'work>) -> io::Result<()>,
) -> io::Result<()> {
    // Includes the finite directory inventory and our three-name cleanup prefix.
    budget.charge_work(2 * 1024 * 1024).map_err(other)?;
    budget.reserve_storage(64 * 1024).map_err(other)?;
    revalidate(budget)?;
    inventory(parent, 4096, |name| {
        require(
            !name.starts_with(super::super::STAGING_PREFIX.as_bytes()),
            "interrupted proof staging requires independent administrator inspection",
        )
    })?;
    match open_directory(parent, DIRECTORY) {
        Ok(existing) => {
            validate(&existing, candidate, uid, gid, budget)?;
            rustix::fs::fsync(&existing)?;
            rustix::fs::fsync(parent)?;
            revalidate(budget)?;
            require(
                same_object(&existing, &open_directory(parent, DIRECTORY)?)?,
                "native configuration path changed",
            )?;
            return validate(&existing, candidate, uid, gid, budget);
        }
        Err(error) if error.raw_os_error() == Some(libc::ENOENT) => (),
        Err(error) => return Err(error),
    }
    let mut nonce = [0; 16];
    require(
        rustix::rand::getrandom(&mut nonce, rustix::rand::GetRandomFlags::NONBLOCK)? == nonce.len()
            && nonce != [0; 16],
        "native provisioning randomness unavailable",
    )?;
    let name = format!("{PREFIX}{:032x}", u128::from_le_bytes(nonce));
    rustix::fs::mkdirat(parent, &name, Mode::RWXU)?;
    let directory = match open_directory(parent, &name) {
        Ok(directory) => directory,
        Err(error) => {
            let cleanup = rustix::fs::unlinkat(parent, &name, AtFlags::REMOVEDIR);
            return Err(io::Error::other(format!(
                "native staging open failed: {error}; cleanup: {cleanup:?}"
            )));
        }
    };
    let mut staging = Staging {
        parent,
        directory,
        name,
        published: false,
    };
    for (name, bytes) in RECORD_NAMES.into_iter().zip(candidate.records()) {
        budget.charge_work(bytes.len() + 4096).map_err(other)?;
        let file = File::from(rustix::fs::openat(
            &staging.directory,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::RUSR | Mode::WUSR,
        )?);
        require(
            rustix::io::write(&file, bytes)? == bytes.len(),
            "native record write was short",
        )?;
        rustix::fs::fchmod(&file, Mode::from_raw_mode(0o444))?;
        rustix::fs::fsync(&file)?;
        revalidate(budget)?;
    }
    rustix::fs::fchmod(&staging.directory, Mode::from_raw_mode(0o755))?;
    validate(&staging.directory, candidate, uid, gid, budget)?;
    rustix::fs::fsync(&staging.directory)?;
    revalidate(budget)?;
    require(
        same_object(&staging.directory, &open_directory(parent, &staging.name)?)?,
        "native staging path changed",
    )?;
    rustix::fs::renameat_with(
        parent,
        &staging.name,
        parent,
        DIRECTORY,
        RenameFlags::NOREPLACE,
    )?;
    staging.published = true;
    let finish = (|| {
        rustix::fs::fsync(parent)?;
        revalidate(budget)?;
        let published = open_directory(parent, DIRECTORY)?;
        require(
            same_object(&staging.directory, &published)?,
            "native published directory changed",
        )?;
        validate(&published, candidate, uid, gid, budget)
    })();
    finish.map_err(|error: io::Error| io::Error::other(format!(
        "native records published but final validation failed; retain for administrator inspection: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_inventory_is_bounded_and_preserves_the_original_cursor() {
        let directory = tempfile::tempdir().unwrap();
        for name in ["first", "second", "third"] {
            std::fs::write(directory.path().join(name), []).unwrap();
        }
        let retained = File::open(directory.path()).unwrap();
        let mut original_buffer = [MaybeUninit::uninit(); 4096];
        let mut original = RawDir::new(&retained, &mut original_buffer);
        while let Some(entry) = original.next() {
            entry.unwrap();
        }
        for _ in 0..2 {
            let mut names = Vec::new();
            inventory(&retained, 5, |name| {
                names.push(name.to_vec());
                Ok(())
            })
            .unwrap();
            names.sort_unstable();
            assert_eq!(
                names,
                [b"first".to_vec(), b"second".to_vec(), b"third".to_vec()]
            );
            assert!(original.next().is_none(), "retained directory cursor moved");
        }
        assert!(inventory(&retained, 4, |_| Ok(())).is_err());
        let mut visits = 0;
        assert!(
            inventory(&retained, 0, |_| {
                visits += 1;
                Ok(())
            })
            .is_err()
        );
        assert_eq!(visits, 0);
    }
}
