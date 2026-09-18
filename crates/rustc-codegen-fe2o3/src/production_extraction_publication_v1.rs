//! Create-new diagnostic extraction output; this does not confer build authority.

use std::fs::OpenOptions;
use std::io::Write as _;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::path::Path;

pub(super) fn publish_new_extraction_bytes_v1(
    output: &Path,
    bytes: &[u8],
    maximum: usize,
    label: &'static str,
) -> Result<(), String> {
    publish_new_extraction_bytes_with_writer_v1(output, bytes, maximum, label, |file, bytes| {
        file.write_all(bytes).and_then(|()| file.sync_all())
    })
}

pub(super) fn publish_new_extraction_bytes_with_writer_v1(
    output: &Path,
    bytes: &[u8],
    maximum: usize,
    label: &'static str,
    write_and_sync: impl FnOnce(&mut std::fs::File, &[u8]) -> std::io::Result<()>,
) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(format!("refusing to publish an empty or oversized {label}"));
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(output).map_err(|error| {
        format!(
            "failed to create new {label} output `{}`: {error}",
            output.display()
        )
    })?;
    if let Err(error) = write_and_sync(&mut file, bytes) {
        return Err(format!(
            "failed to publish {label} `{}`; the create-new partial output was retained for fail-closed cleanup: {error}",
            output.display()
        ));
    }
    #[cfg(unix)]
    {
        let descriptor = file.metadata().map_err(|error| {
            format!(
                "failed to inspect published {label} descriptor `{}`: {error}",
                output.display()
            )
        })?;
        let path = std::fs::symlink_metadata(output).map_err(|error| {
            format!(
                "failed to re-inspect published {label} path `{}`: {error}",
                output.display()
            )
        })?;
        if !descriptor.is_file()
            || descriptor.len() != bytes.len() as u64
            || descriptor.dev() != path.dev()
            || descriptor.ino() != path.ino()
            || path.file_type().is_symlink()
        {
            return Err(format!(
                "{label} output `{}` changed identity during publication",
                output.display()
            ));
        }
    }
    Ok(())
}
