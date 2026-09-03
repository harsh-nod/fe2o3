//! One-shot, read-only projection of a caller-bound qualification manifest.

use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::fs::{FileExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use fe2o3_debug_protocol::{
    MAX_QUALIFICATION_ASSESSMENT_BYTES_V1, MAX_QUALIFICATION_MANIFEST_BYTES_V1,
    decode_qualification_manifest_v1,
};
use rustix::fs::{FileType, Mode, OFlags, ResolveFlags, openat2};

const USAGE: &str = "fe2o3-debug qualification --manifest /absolute/path/to/qualification.json";

pub(crate) fn run(arguments: Vec<OsString>) -> ExitCode {
    let path = match parse_options(arguments) {
        Ok(path) => path,
        Err(message) => {
            super::write_bootstrap_error(
                "arguments",
                "invalid_qualification_command_line",
                &message,
            );
            return ExitCode::FAILURE;
        }
    };
    let bytes = match read_manifest(&path) {
        Ok(bytes) => bytes,
        Err(message) => {
            super::write_bootstrap_error("input", "qualification_manifest_unavailable", &message);
            return ExitCode::FAILURE;
        }
    };
    let manifest = match decode_qualification_manifest_v1(&bytes) {
        Ok(manifest) => manifest,
        Err(error) => {
            super::write_bootstrap_error(
                "input",
                "qualification_manifest_rejected",
                &error.to_string(),
            );
            return ExitCode::FAILURE;
        }
    };
    let assessment = match manifest.assessment() {
        Ok(assessment) => assessment,
        Err(error) => {
            super::write_bootstrap_error(
                "assessment",
                "qualification_assessment_failed",
                &error.to_string(),
            );
            return ExitCode::FAILURE;
        }
    };
    let mut output = match serde_json::to_vec(&assessment) {
        Ok(output) => output,
        Err(_) => {
            super::write_bootstrap_error(
                "output",
                "qualification_assessment_encoding_failed",
                "could not encode the validated qualification assessment",
            );
            return ExitCode::FAILURE;
        }
    };
    if output
        .len()
        .checked_add(1)
        .is_none_or(|length| length > MAX_QUALIFICATION_ASSESSMENT_BYTES_V1)
    {
        super::write_bootstrap_error(
            "output",
            "qualification_assessment_too_large",
            "the qualification assessment exceeds its encoded byte limit",
        );
        return ExitCode::FAILURE;
    }
    output.push(b'\n');
    if std::io::stdout().lock().write_all(&output).is_err() {
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn parse_options(arguments: Vec<OsString>) -> Result<PathBuf, String> {
    let [command, option, value] = arguments.as_slice() else {
        return Err(USAGE.to_owned());
    };
    if command != OsStr::new("qualification") || option != OsStr::new("--manifest") {
        return Err(USAGE.to_owned());
    }
    let path = PathBuf::from(value);
    if !path.is_absolute() {
        return Err("qualification manifest path must be absolute".to_owned());
    }
    Ok(path)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StableMetadataV1 {
    device: u64,
    inode: u64,
    mode: u32,
    links: u64,
    bytes: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
}

impl StableMetadataV1 {
    fn from_metadata(metadata: &std::fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            mode: metadata.mode(),
            links: metadata.nlink(),
            bytes: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
        }
    }
}

fn securely_open(path: &Path) -> Result<File, String> {
    openat2(
        rustix::fs::CWD,
        path,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )
    .map(File::from)
    .map_err(|error| format!("could not securely open qualification manifest: {error}"))
}

fn read_manifest(path: &Path) -> Result<Vec<u8>, String> {
    read_manifest_with_hook(path, || {})
}

fn read_manifest_with_hook(path: &Path, after_read: impl FnOnce()) -> Result<Vec<u8>, String> {
    let mut file = securely_open(path)?;
    let before = file
        .metadata()
        .map_err(|error| format!("could not inspect qualification manifest: {error}"))?;
    if FileType::from_raw_mode(before.mode()) != FileType::RegularFile
        || before.nlink() != 1
        || before.len() == 0
        || before.len() > MAX_QUALIFICATION_MANIFEST_BYTES_V1 as u64
    {
        return Err("qualification manifest is not a bounded single-link regular file".to_owned());
    }
    let expected = usize::try_from(before.len())
        .map_err(|_| "qualification manifest exceeds the platform size".to_owned())?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected)
        .map_err(|_| "could not reserve bounded qualification manifest storage".to_owned())?;
    Read::by_ref(&mut file)
        .take(MAX_QUALIFICATION_MANIFEST_BYTES_V1 as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("could not read qualification manifest: {error}"))?;
    if bytes.len() != expected || bytes.len() > MAX_QUALIFICATION_MANIFEST_BYTES_V1 {
        return Err("qualification manifest exceeds the byte limit".to_owned());
    }
    after_read();
    let after = file
        .metadata()
        .map_err(|error| format!("could not revalidate qualification manifest: {error}"))?;
    let stable = StableMetadataV1::from_metadata(&before);
    let reopened = securely_open(path)
        .and_then(|reopened| {
            reopened.metadata().map_err(|error| {
                format!("could not inspect reopened qualification manifest: {error}")
            })
        })
        .map(|metadata| StableMetadataV1::from_metadata(&metadata));
    if stable != StableMetadataV1::from_metadata(&after) || reopened.as_ref() != Ok(&stable) {
        return Err("qualification manifest changed during admission".to_owned());
    }
    let mut offset = 0;
    let mut buffer = [0_u8; 64 * 1024];
    while offset < bytes.len() {
        let count = buffer.len().min(bytes.len() - offset);
        let read = file
            .read_at(&mut buffer[..count], offset as u64)
            .map_err(|error| format!("could not reread qualification manifest: {error}"))?;
        if read == 0 || buffer[..read] != bytes[offset..offset + read] {
            return Err("qualification manifest changed during admission".to_owned());
        }
        offset += read;
    }
    let mut trailing = [0_u8; 1];
    if file
        .read_at(&mut trailing, bytes.len() as u64)
        .map_err(|error| format!("could not finish qualification manifest reread: {error}"))?
        != 0
    {
        return Err("qualification manifest changed during admission".to_owned());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    fn directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "fe2o3-qualification-admission-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("unnamed")
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn command_line_is_exact_and_absolute() {
        assert_eq!(
            parse_options(vec![
                "qualification".into(),
                "--manifest".into(),
                "/tmp/qualification.json".into(),
            ]),
            Ok(PathBuf::from("/tmp/qualification.json"))
        );
        for arguments in [
            vec!["qualification".into()],
            vec![
                "qualification".into(),
                "--manifest".into(),
                "relative.json".into(),
            ],
            vec![
                "qualification".into(),
                "--manifest".into(),
                "/tmp/qualification.json".into(),
                "extra".into(),
            ],
        ] {
            assert!(parse_options(arguments).is_err());
        }
    }

    #[test]
    fn manifest_admission_rejects_symlink_hardlink_oversize_and_substitution() {
        let root = directory();
        let input = root.join("input.json");
        fs::write(&input, b"{}\n").unwrap();

        let linked = root.join("linked.json");
        symlink(&input, &linked).unwrap();
        assert!(read_manifest(&linked).is_err());

        let hardlinked = root.join("hardlinked.json");
        fs::hard_link(&input, &hardlinked).unwrap();
        assert!(read_manifest(&input).is_err());
        fs::remove_file(hardlinked).unwrap();

        let oversized = root.join("oversized.json");
        let file = File::create(&oversized).unwrap();
        file.set_len(MAX_QUALIFICATION_MANIFEST_BYTES_V1 as u64 + 1)
            .unwrap();
        assert!(read_manifest(&oversized).is_err());

        let replacement = root.join("replacement.json");
        fs::write(&replacement, b"attacker\n").unwrap();
        let original = input.clone();
        let attacker = replacement.clone();
        assert!(
            read_manifest_with_hook(&input, move || {
                fs::rename(attacker, original).unwrap();
            })
            .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }
}
