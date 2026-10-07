//! Opt-in build-only inert files from an actual original-root publication.
//! Neither selection metadata nor the output manifest grants provenance or
//! policy authority. Independent review and separately pinned approval follow.
use fe2o3_artifact_transaction::ProducerIdentity;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use rustix::fs::{AtFlags, Mode, OFlags, RenameFlags};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{ffi::OsString, fs::File, io, os::unix::fs::MetadataExt, path::Path, process::Command};

pub(crate) const ENV: &str = "FE2O3_NATIVE_POLICY_INPUT_EXPORT_V1";
const FLAG: &str = "--export-native-policy-inputs";
const REQUEST_MAX: usize = 64 * 1024;
const FRAME: usize = 256 * 1024;
const SOURCE_MAX: usize = 4 * 1024 * 1024;
const ROSTER_MAX: usize = fe2o3_verifier::MAX_NATIVE_CONDITIONAL_ROOT_POLICY_FILE_BYTES_V1;
const NAMES: [&str; 3] = ["source-packet-v2", "policy-roster-v1", "manifest.json"];

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    crate_name: String,
    source: String,
    output: String,
}

fn error(value: impl std::fmt::Display) -> io::Error {
    io::Error::other(value.to_string())
}
fn require(condition: bool, message: &'static str) -> io::Result<()> {
    if condition {
        Ok(())
    } else {
        Err(error(message))
    }
}

impl Request {
    fn validate(&self) -> io::Result<()> {
        require(
            self.crate_name.len() <= 256
                && self.source.len() <= 4091
                && self.output.len() <= 4096
                && !self.output.as_bytes().contains(&0)
                && Path::new(&self.output).is_absolute()
                && Path::new(&self.output).file_name().is_some(),
            "native policy export requires bounded exact producer names and an absolute output directory",
        )?;
        self.producer().map(|_| ())
    }
    fn producer(&self) -> io::Result<ProducerIdentity> {
        ProducerIdentity::from_codegen(&self.crate_name, Some(Path::new(&self.source)))
            .map_err(error)
    }
    pub(crate) fn matches(&self, producer: &ProducerIdentity) -> io::Result<bool> {
        Ok(self.producer()? == *producer)
    }
    pub(crate) fn preflight(&self) -> Result<OutputParent, String> {
        self.open_parent(true).map_err(|e| e.to_string())
    }
    fn open_parent(&self, absent: bool) -> io::Result<OutputParent> {
        self.validate()?;
        let output = Path::new(&self.output);
        let parent = File::from(rustix::fs::open(
            output
                .parent()
                .ok_or_else(|| error("native export parent"))?,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )?);
        let m = parent.metadata()?;
        require(
            m.is_dir() && m.uid() == rustix::process::geteuid().as_raw() && m.mode() & 0o022 == 0,
            "native export requires a caller-owned parent without group/other write access",
        )?;
        let name = output.file_name().unwrap().to_owned();
        if absent {
            match rustix::fs::statat(&parent, &name, AtFlags::SYMLINK_NOFOLLOW) {
                Err(rustix::io::Errno::NOENT) => {}
                Err(e) => return Err(e.into()),
                Ok(_) => return Err(error("native export refuses an existing output")),
            }
        }
        Ok(OutputParent {
            directory: parent,
            name,
        })
    }

    pub(crate) fn publish(
        &self,
        producer: &ProducerIdentity,
        source: &[u8],
        roster: &[u8],
        observations: OriginalObservations,
        budget: &mut Budget<'_>,
    ) -> io::Result<()> {
        budget.charge_work(1024 * 1024).map_err(error)?;
        budget.reserve_storage(FRAME).map_err(error)?;
        require(
            self.matches(producer)?,
            "native export producer differs from actual owner",
        )?;
        require(
            (1..=SOURCE_MAX).contains(&source.len()) && (1..=ROSTER_MAX).contains(&roster.len()),
            "native export input bounds",
        )?;
        require(
            budget.storage() >= FRAME + source.len() + roster.len(),
            "native export source custody is not prepaid",
        )?;
        budget
            .charge_work(2 * (source.len() + roster.len()))
            .map_err(error)?;
        let manifest = Manifest {
            schema: "fe2o3.native-policy-inputs.v1",
            crate_name: &self.crate_name,
            source_spelling: &self.source,
            target: "gfx942:xnack-",
            source_packet: Blob::new(source),
            policy_roster: Blob::new(roster),
            original_handoff_identity: observations.handoff.0,
            original_handoff_bytes: observations.handoff.1,
            original_carriage_identity: observations.carriage,
            original_finalized_artifact_identity: observations.artifact,
            grants_authority: false,
            independently_approved: false,
        };
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(REQUEST_MAX).map_err(error)?;
        require(
            bytes.capacity() == REQUEST_MAX,
            "native export manifest capacity",
        )?;
        serde_json::to_writer(&mut bytes, &manifest).map_err(error)?;
        require(
            bytes.capacity() == REQUEST_MAX && bytes.len() <= REQUEST_MAX,
            "native export manifest bound",
        )?;
        let parent = self.open_parent(true)?;
        publish_directory(&parent, [source, roster, &bytes], || Ok(()))
    }
}

/// Retained only for a final presence diagnostic; it cannot create an admitted
/// producer, policy, publication, currentness record or runtime capability.
pub(crate) struct OutputParent {
    directory: File,
    name: OsString,
}
impl OutputParent {
    pub(crate) fn require_bundle(self) -> Result<(), String> {
        let result = (|| {
            let directory = open_directory(&self.directory, &self.name)?;
            for name in NAMES {
                let m = rustix::fs::statat(&directory, name, AtFlags::SYMLINK_NOFOLLOW)?;
                require(
                    m.st_mode & libc::S_IFMT == libc::S_IFREG,
                    "native export bundle is incomplete",
                )?;
            }
            Ok::<_, io::Error>(())
        })();
        result.map_err(|e| format!("requested native policy input bundle was not produced (this check is diagnostic only): {e}"))
    }
}

pub(crate) struct OriginalObservations {
    pub handoff: ([u8; 32], u64),
    pub carriage: [u8; 32],
    pub artifact: [u8; 32],
}
#[derive(Serialize)]
struct Blob {
    sha256: String,
    bytes: u64,
}
impl Blob {
    fn new(bytes: &[u8]) -> Self {
        Self {
            sha256: crate::hex_encode(&Sha256::digest(bytes)),
            bytes: bytes.len() as u64,
        }
    }
}
#[derive(Serialize)]
struct Manifest<'a> {
    schema: &'static str,
    crate_name: &'a str,
    source_spelling: &'a str,
    target: &'static str,
    source_packet: Blob,
    policy_roster: Blob,
    original_handoff_identity: [u8; 32],
    original_handoff_bytes: u64,
    original_carriage_identity: [u8; 32],
    original_finalized_artifact_identity: [u8; 32],
    grants_authority: bool,
    independently_approved: bool,
}

pub(crate) fn parse(
    command: &str,
    args: &[OsString],
) -> Result<(Vec<OsString>, Option<Request>), String> {
    let mut forwarded = Vec::with_capacity(args.len());
    let mut selected = None;
    let mut at = 0;
    let mut trailing = false;
    while at < args.len() {
        if !trailing && args[at] == FLAG {
            if command != "build" || selected.is_some() || at + 3 >= args.len() {
                return Err(
                    "one --export-native-policy-inputs CRATE SOURCE NEW_OUTPUT_DIR requires build"
                        .into(),
                );
            }
            let text = |index: usize| {
                args[index]
                    .to_str()
                    .map(str::to_owned)
                    .ok_or_else(|| "native policy export requires exact UTF-8 arguments".to_owned())
            };
            let request = Request {
                crate_name: text(at + 1)?,
                source: text(at + 2)?,
                output: text(at + 3)?,
            };
            request.validate().map_err(|e| e.to_string())?;
            selected = Some(request);
            at += 4;
        } else {
            trailing |= args[at] == "--";
            forwarded.push(args[at].clone());
            at += 1;
        }
    }
    Ok((forwarded, selected))
}

pub(crate) fn configure(command: &mut Command, request: Option<&Request>) -> Result<(), String> {
    command.env_remove(ENV);
    if let Some(request) = request {
        request.validate().map_err(|e| e.to_string())?;
        let encoded = serde_json::to_string(request).map_err(|e| e.to_string())?;
        if encoded.len() > REQUEST_MAX {
            return Err("native export request exceeds bound".into());
        }
        command.env(ENV, encoded);
    }
    Ok(())
}
pub(crate) fn from_environment() -> Result<Option<Request>, String> {
    std::env::var_os(ENV)
        .map(|value| decode(&value))
        .transpose()
}
pub(crate) fn reject_ambient(value: Option<&std::ffi::OsStr>) -> Result<(), String> {
    if value.is_some() {
        Err("native policy export cannot be activated through ambient environment".into())
    } else {
        Ok(())
    }
}
fn decode(value: &std::ffi::OsStr) -> Result<Request, String> {
    let value = value
        .to_str()
        .ok_or("native export metadata is not UTF-8")?;
    if value.len() > REQUEST_MAX {
        return Err("native export request exceeds bound".into());
    }
    let request: Request = serde_json::from_str(value).map_err(|e| e.to_string())?;
    request.validate().map_err(|e| e.to_string())?;
    Ok(request)
}

fn open_directory(parent: &File, name: impl rustix::path::Arg) -> io::Result<File> {
    Ok(File::from(rustix::fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )?))
}
fn same(directory: &File, parent: &File, name: &str) -> bool {
    let (Ok(a), Ok(b)) = (
        directory.metadata(),
        rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW),
    ) else {
        return false;
    };
    a.dev() == b.st_dev && a.ino() == b.st_ino
}
struct Stage<'a> {
    parent: &'a File,
    directory: File,
    name: String,
    files: [Option<File>; 3],
    published: bool,
}
impl Drop for Stage<'_> {
    fn drop(&mut self) {
        if self.published {
            return;
        }
        for (name, original) in NAMES.into_iter().zip(&self.files) {
            if let Some(file) = original
                && let Ok(original) = file.metadata()
                && let Ok(current) =
                    rustix::fs::statat(&self.directory, name, AtFlags::SYMLINK_NOFOLLOW)
                && original.dev() == current.st_dev
                && original.ino() == current.st_ino
            {
                let _ = rustix::fs::unlinkat(&self.directory, name, AtFlags::empty());
            }
        }
        if same(&self.directory, self.parent, &self.name) {
            let _ = rustix::fs::unlinkat(self.parent, &self.name, AtFlags::REMOVEDIR);
        }
    }
}
fn publish_directory(
    parent: &OutputParent,
    records: [&[u8]; 3],
    before_publish: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    let mut nonce = [0; 16];
    require(
        rustix::rand::getrandom(&mut nonce, rustix::rand::GetRandomFlags::NONBLOCK)? == nonce.len()
            && nonce != [0; 16],
        "native export randomness unavailable",
    )?;
    let name = format!(".fe2o3-native-inputs-{:032x}", u128::from_le_bytes(nonce));
    rustix::fs::mkdirat(&parent.directory, &name, Mode::RWXU)?;
    let directory = match open_directory(&parent.directory, &name) {
        Ok(directory) => directory,
        Err(error) => {
            // No original directory descriptor was captured: leave the staging
            // name for inspection rather than delete an unverified occurrence.
            return Err(error);
        }
    };
    let mut stage = Stage {
        parent: &parent.directory,
        directory,
        name,
        files: std::array::from_fn(|_| None),
        published: false,
    };
    for (index, (name, bytes)) in NAMES.into_iter().zip(records).enumerate() {
        let file = File::from(rustix::fs::openat(
            &stage.directory,
            name,
            OFlags::WRONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::CREATE | OFlags::EXCL,
            Mode::RUSR,
        )?);
        stage.files[index] = Some(file);
        let file = stage.files[index].as_ref().unwrap();
        require(
            rustix::io::write(file, bytes)? == bytes.len(),
            "native export single-attempt write was short",
        )?;
        rustix::fs::fchmod(file, Mode::RUSR)?;
        rustix::fs::fsync(file)?;
    }
    rustix::fs::fsync(&stage.directory)?;
    before_publish()?;
    require(
        same(&stage.directory, &parent.directory, &stage.name),
        "native export staging was substituted",
    )?;
    rustix::fs::renameat_with(
        &parent.directory,
        &stage.name,
        &parent.directory,
        &parent.name,
        RenameFlags::NOREPLACE,
    )?;
    stage.published = true;
    rustix::fs::fsync(&parent.directory).map_err(|e| {
        error(format!(
            "native inputs published but parent sync failed: {e}"
        ))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests;
