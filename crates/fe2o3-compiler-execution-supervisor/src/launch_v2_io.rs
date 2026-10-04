//! Finite I/O adapter for the shared 704-byte static manifest and predicates.
use super::{BYTES, Error, Object, Result, StaticManifest, checks};
use rustix::fs::{MemfdFlags, Mode, OFlags, SealFlags};
use std::fs::File;

pub(super) fn create(manifest: &StaticManifest) -> Result<(File, Object)> {
    let image = rustix::fs::memfd_create(
        c"fe2o3-static-preexec-manifest-v1",
        MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING,
    )
    .map(File::from)
    .map_err(|errno| Error::Io {
        operation: "create static pre-exec manifest memfd",
        errno,
    })?;
    rustix::fs::fchmod(&image, Mode::RUSR).map_err(|errno| Error::Io {
        operation: "protect static pre-exec manifest mode",
        errno,
    })?;
    let written = rustix::io::pwrite(&image, &manifest.encode(), 0).map_err(|errno| Error::Io {
        operation: "populate static pre-exec manifest",
        errno,
    })?;
    if written != BYTES {
        return Err(Error::DescriptorChanged(
            "short static pre-exec manifest write",
        ));
    }
    rustix::fs::fsync(&image).map_err(|errno| Error::Io {
        operation: "sync static pre-exec manifest",
        errno,
    })?;
    rustix::fs::fcntl_add_seals(
        &image,
        SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK,
    )
    .and_then(|()| rustix::fs::fcntl_add_seals(&image, SealFlags::SEAL))
    .map_err(|errno| Error::Io {
        operation: "seal static pre-exec manifest",
        errno,
    })?;
    let expected = checks::object_identity(&image, "static pre-exec manifest")?;
    let directory = rustix::fs::open(
        c"/proc/self/fd",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|errno| Error::Io {
        operation: "open descriptor directory",
        errno,
    })?;
    let read_only = rustix::fs::openat(
        &directory,
        rustix::path::DecInt::from_fd(&image),
        OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
    .map_err(|errno| Error::Io {
        operation: "bind read-only static pre-exec manifest",
        errno,
    })?;
    // Keep the original object pinned until the reopened object and bytes agree.
    validate(&read_only, manifest, expected)?;
    Ok((read_only, expected))
}

pub(super) fn validate(file: &File, manifest: &StaticManifest, expected: Object) -> Result<()> {
    checks::validate_manifest_metadata(file, expected)?;
    let mut bytes = [0; BYTES];
    if rustix::io::pread(file, &mut bytes[..], 0).map_err(|errno| Error::Io {
        operation: "read static pre-exec manifest",
        errno,
    })? != BYTES
    {
        return Err(Error::DescriptorChanged(
            "short static pre-exec manifest read",
        ));
    }
    let mut trailing = [0; 1];
    if rustix::io::pread(file, &mut trailing[..], BYTES as u64).map_err(|errno| Error::Io {
        operation: "check static pre-exec manifest boundary",
        errno,
    })? != 0
    {
        return Err(Error::DescriptorChanged("static pre-exec manifest length"));
    }
    let decoded = StaticManifest::decode(&bytes)?;
    if &decoded != manifest || bytes != manifest.encode() {
        return Err(Error::DescriptorChanged("static pre-exec manifest bytes"));
    }
    manifest.validate_manifest_object(&expected)?;
    checks::validate_manifest_metadata(file, expected)?;
    Ok(())
}
