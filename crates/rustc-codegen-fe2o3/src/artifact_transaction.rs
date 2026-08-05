//! Compiler artifact publication for cooperating local writers.
//!
//! The lock and bounded ownership registry coordinate fe2o3 processes that use this protocol.
//! Producer source paths and registry contents are non-authoritative cleanup hints, not
//! authenticated identities or proof that an artifact is valid. The compiler subprocess receives
//! staged paths through an inherited `/proc/self/fd` staging-directory handle; this pins the exact
//! private staging inode across pathname substitution, but is Linux-specific and does not
//! constrain a hostile subprocess.
//!
//! The configured output directory is a generated-artifact namespace. Canonically named files
//! without a registry owner are treated as legacy fe2o3 outputs: a successful transaction adopts
//! them, while a failed transaction invalidates them so stale executable code cannot survive a
//! rejected codegen preflight or rebuild. An entry explicitly owned by another producer is never
//! adopted or removed. Fully absent ownership entries are pruned as crash tombstones before name
//! protection is applied.
//!
//! Staged files and directories are synced before publication, and the output directory is synced
//! after registry commit and staging cleanup. Each final rename is atomic, but the collection is
//! not atomically visible as a unit: a crash during the rename sequence can leave a partial
//! generation, which a later cooperating transaction will reconcile.

use crate::amdgpu_llvm::{DeviceArtifact, EmitError};
use rustix::fd::{AsRawFd, OwnedFd};
use rustix::fs::{
    AtFlags, Dir, FileType, FlockOperation, Mode, OFlags, flock, fstat, fsync, mkdirat, open,
    openat, renameat, statat, unlinkat,
};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};

const LOCK_FILE: &str = ".fe2o3-artifacts.lock";
const OWNERSHIP_FILE: &str = ".fe2o3-owners-v1";
const RECOVERY_OWNERSHIP_FILE: &str = ".fe2o3-owners-v1.recovery";
const STAGED_OWNERSHIP_FILE: &str = "owners-v1.next";
const STAGING_PREFIX: &str = ".fe2o3-stage-";
const OWNERSHIP_MAGIC: &[u8] = b"FE2O3-OWNERS-V1\0";
const MAX_ARTIFACT_NAME_BYTES: usize = 128;
const MAX_PRODUCER_SOURCE_BYTES: usize = 4096;
const MAX_PRODUCERS: usize = 1024;
const MAX_KERNELS_PER_PRODUCER: usize = 4096;
const MAX_TOTAL_OWNED_KERNELS: usize = 4096;
const MAX_OWNERSHIP_BYTES: usize = 1024 * 1024;
const MAX_STAGING_ATTEMPTS: u64 = 64;
// Three files for every owned and ownerless kernel, plus bounded staging/metadata headroom.
const MAX_OUTPUT_ENTRIES: usize = MAX_TOTAL_OWNED_KERNELS * 7;

static NEXT_STAGING_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ProducerIdentity {
    stable_source: String,
    crate_name: String,
}

impl ProducerIdentity {
    pub(crate) fn from_codegen(
        crate_name: &str,
        local_source: Option<&Path>,
    ) -> Result<Self, EmitError> {
        validate_simple_name(crate_name, "crate name")?;
        let stable_source = match local_source {
            Some(path) => {
                let path = path.to_str().ok_or_else(|| EmitError::InvalidProducer {
                    reason: "local crate source path is not UTF-8".to_string(),
                })?;
                format!("path:{path}")
            }
            None => format!("crate:{crate_name}"),
        };
        if stable_source.len() > MAX_PRODUCER_SOURCE_BYTES {
            return Err(EmitError::InvalidProducer {
                reason: format!("stable source identity exceeds {MAX_PRODUCER_SOURCE_BYTES} bytes"),
            });
        }
        if stable_source.ends_with(':') || stable_source.as_bytes().contains(&0) {
            return Err(EmitError::InvalidProducer {
                reason: "stable source identity is empty or contains a NUL byte".to_string(),
            });
        }

        Ok(Self {
            stable_source,
            crate_name: crate_name.to_string(),
        })
    }

    #[cfg(test)]
    fn for_test(crate_name: &str, source: &str) -> Self {
        Self::from_codegen(crate_name, Some(Path::new(source))).unwrap()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ProducerOwnership {
    crate_name: String,
    kernels: BTreeSet<String>,
}

// Cleanup bookkeeping only. It is neither launch authority nor evidence that an artifact is valid.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct OwnershipRegistry {
    producers: BTreeMap<String, ProducerOwnership>,
}

impl OwnershipRegistry {
    fn owned_by(&self, producer: &ProducerIdentity) -> BTreeSet<String> {
        self.producers
            .get(&producer.stable_source)
            .map(|ownership| ownership.kernels.clone())
            .unwrap_or_default()
    }

    fn owner_of<'a>(&'a self, kernel: &str) -> Option<&'a str> {
        self.producers.iter().find_map(|(source, ownership)| {
            ownership
                .kernels
                .contains(kernel)
                .then_some(source.as_str())
        })
    }

    fn set_owned(&mut self, producer: &ProducerIdentity, kernels: BTreeSet<String>) {
        if kernels.is_empty() {
            self.producers.remove(&producer.stable_source);
        } else {
            self.producers.insert(
                producer.stable_source.clone(),
                ProducerOwnership {
                    crate_name: producer.crate_name.clone(),
                    kernels,
                },
            );
        }
    }

    fn encode(&self) -> Result<Vec<u8>, EmitError> {
        self.validate()?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(OWNERSHIP_MAGIC);
        push_u32(&mut bytes, self.producers.len())?;
        for (source, ownership) in &self.producers {
            push_text(&mut bytes, source)?;
            push_text(&mut bytes, &ownership.crate_name)?;
            push_u32(&mut bytes, ownership.kernels.len())?;
            for kernel in &ownership.kernels {
                push_text(&mut bytes, kernel)?;
            }
        }
        if bytes.len() > MAX_OWNERSHIP_BYTES {
            return Err(EmitError::Ownership {
                reason: "canonical ownership registry exceeds its byte bound".to_string(),
            });
        }
        Ok(bytes)
    }

    fn decode(bytes: &[u8]) -> Result<Self, EmitError> {
        if bytes.len() > MAX_OWNERSHIP_BYTES {
            return Err(ownership_error("ownership registry exceeds its byte bound"));
        }
        let mut decoder = Decoder::new(bytes);
        if decoder.take(OWNERSHIP_MAGIC.len())? != OWNERSHIP_MAGIC {
            return Err(ownership_error("bad ownership registry magic"));
        }
        let producer_count = decoder.u32()? as usize;
        if producer_count > MAX_PRODUCERS {
            return Err(ownership_error("too many ownership producers"));
        }

        let mut producers = BTreeMap::new();
        let mut total_kernels = 0usize;
        for _ in 0..producer_count {
            let source = decoder.text(MAX_PRODUCER_SOURCE_BYTES)?;
            validate_stable_source(&source)?;
            let crate_name = decoder.text(MAX_ARTIFACT_NAME_BYTES)?;
            validate_simple_name(&crate_name, "owned crate name")?;
            let kernel_count = decoder.u32()? as usize;
            if kernel_count > MAX_KERNELS_PER_PRODUCER {
                return Err(ownership_error("too many kernels for one producer"));
            }
            total_kernels = total_kernels
                .checked_add(kernel_count)
                .ok_or_else(|| ownership_error("owned kernel count overflow"))?;
            if total_kernels > MAX_TOTAL_OWNED_KERNELS {
                return Err(ownership_error("too many kernels in ownership registry"));
            }

            let mut kernels = BTreeSet::new();
            for _ in 0..kernel_count {
                let kernel = decoder.text(MAX_ARTIFACT_NAME_BYTES)?;
                validate_artifact_name(&kernel)?;
                if !kernels.insert(kernel) {
                    return Err(ownership_error("duplicate owned kernel name"));
                }
            }
            if producers
                .insert(
                    source,
                    ProducerOwnership {
                        crate_name,
                        kernels,
                    },
                )
                .is_some()
            {
                return Err(ownership_error("duplicate producer identity"));
            }
        }
        if !decoder.is_finished() {
            return Err(ownership_error("trailing ownership registry bytes"));
        }

        let registry = Self { producers };
        registry.validate()?;
        if registry.encode()? != bytes {
            return Err(ownership_error("ownership registry is not canonical"));
        }
        Ok(registry)
    }

    fn validate(&self) -> Result<(), EmitError> {
        if self.producers.len() > MAX_PRODUCERS {
            return Err(ownership_error("too many ownership producers"));
        }
        let mut all_kernels = BTreeSet::new();
        for (source, ownership) in &self.producers {
            validate_stable_source(source)?;
            validate_simple_name(&ownership.crate_name, "owned crate name")?;
            if ownership.kernels.is_empty() || ownership.kernels.len() > MAX_KERNELS_PER_PRODUCER {
                return Err(ownership_error("invalid per-producer kernel count"));
            }
            for kernel in &ownership.kernels {
                validate_artifact_name(kernel)?;
                if !all_kernels.insert(kernel) {
                    return Err(ownership_error(
                        "one artifact name is owned by multiple producers",
                    ));
                }
            }
        }
        if all_kernels.len() > MAX_TOTAL_OWNED_KERNELS {
            return Err(ownership_error("too many kernels in ownership registry"));
        }
        Ok(())
    }
}

struct Decoder<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Decoder<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], EmitError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or_else(|| ownership_error("ownership offset overflow"))?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or_else(|| ownership_error("truncated ownership registry"))?;
        self.offset = end;
        Ok(value)
    }

    fn u16(&mut self) -> Result<u16, EmitError> {
        let bytes: [u8; 2] = self.take(2)?.try_into().unwrap();
        Ok(u16::from_le_bytes(bytes))
    }

    fn u32(&mut self) -> Result<u32, EmitError> {
        let bytes: [u8; 4] = self.take(4)?.try_into().unwrap();
        Ok(u32::from_le_bytes(bytes))
    }

    fn text(&mut self, maximum: usize) -> Result<String, EmitError> {
        let length = self.u16()? as usize;
        if length == 0 || length > maximum {
            return Err(ownership_error("invalid ownership text length"));
        }
        String::from_utf8(self.take(length)?.to_vec())
            .map_err(|_| ownership_error("ownership text is not UTF-8"))
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

fn push_u32(bytes: &mut Vec<u8>, value: usize) -> Result<(), EmitError> {
    let value = u32::try_from(value).map_err(|_| ownership_error("ownership count overflow"))?;
    bytes.extend_from_slice(&value.to_le_bytes());
    Ok(())
}

fn push_text(bytes: &mut Vec<u8>, text: &str) -> Result<(), EmitError> {
    let length =
        u16::try_from(text.len()).map_err(|_| ownership_error("ownership text length overflow"))?;
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(text.as_bytes());
    Ok(())
}

fn validate_stable_source(source: &str) -> Result<(), EmitError> {
    if source.len() > MAX_PRODUCER_SOURCE_BYTES
        || !(source.starts_with("path:") || source.starts_with("crate:"))
        || source.ends_with(':')
        || source.as_bytes().contains(&0)
    {
        return Err(ownership_error("invalid stable producer source"));
    }
    Ok(())
}

fn ownership_error(reason: impl Into<String>) -> EmitError {
    EmitError::Ownership {
        reason: reason.into(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PublicationState {
    NotStarted {
        total_final_renames: usize,
    },
    Partial {
        completed_final_renames: usize,
        total_final_renames: usize,
    },
    FinalsPublished {
        final_renames: usize,
    },
    CommittedWithCleanupFailure {
        final_renames: usize,
    },
    Committed {
        final_renames: usize,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Barrier, mpsc};
    use std::thread;
    use std::time::Duration;

    static NEXT_TEST_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            loop {
                let id = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
                let path = std::env::temp_dir().join(format!(
                    "fe2o3-artifact-transaction-test-{}-{id}",
                    process::id()
                ));
                match fs::create_dir(&path) {
                    Ok(()) => return Self { path },
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => panic!("failed to create test directory: {error}"),
                }
            }
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[derive(Clone)]
    struct TestKernel {
        name: &'static str,
        generation: &'static str,
        valid: bool,
    }

    #[derive(Default)]
    struct Faults {
        fail_stage_create: bool,
        fail_stage_stat: bool,
        fail_artifact_rename_at: Option<usize>,
        fail_ownership_rename: bool,
        fail_invalidate_entry: Option<String>,
        fail_cleanup: bool,
        replace_output_after_commit: Option<(PathBuf, PathBuf)>,
    }

    impl TransactionHooks for Faults {
        fn before_stage_create(&mut self) -> io::Result<()> {
            if self.fail_stage_create {
                self.fail_stage_create = false;
                Err(io::Error::other("injected staging creation failure"))
            } else {
                Ok(())
            }
        }

        fn before_stage_stat(&mut self) -> io::Result<()> {
            if self.fail_stage_stat {
                self.fail_stage_stat = false;
                Err(io::Error::other("injected staging stat failure"))
            } else {
                Ok(())
            }
        }

        fn before_rename(&mut self, kind: RenameKind, completed: usize) -> io::Result<()> {
            match kind {
                RenameKind::Artifact if self.fail_artifact_rename_at == Some(completed) => {
                    self.fail_artifact_rename_at = None;
                    Err(io::Error::other("injected artifact rename failure"))
                }
                RenameKind::Ownership if self.fail_ownership_rename => {
                    self.fail_ownership_rename = false;
                    Err(io::Error::other("injected ownership rename failure"))
                }
                _ => Ok(()),
            }
        }

        fn before_invalidate(&mut self, entry: &str) -> io::Result<()> {
            if self.fail_invalidate_entry.as_deref() == Some(entry) {
                self.fail_invalidate_entry = None;
                Err(io::Error::other("injected invalidation failure"))
            } else {
                Ok(())
            }
        }

        fn before_stage_cleanup(&mut self) -> io::Result<()> {
            if self.fail_cleanup {
                self.fail_cleanup = false;
                Err(io::Error::other("injected staging cleanup failure"))
            } else {
                Ok(())
            }
        }

        fn after_registry_commit(&mut self) -> io::Result<()> {
            if let Some((output, relocated)) = self.replace_output_after_commit.take() {
                fs::rename(&output, relocated)?;
                fs::create_dir(output)?;
            }
            Ok(())
        }
    }

    fn fake_compile(llvm_ir_path: &Path, hsaco_path: &Path) -> Result<(), EmitError> {
        let llvm_ir = fs::read_to_string(llvm_ir_path)?;
        fs::write(hsaco_path.with_extension("o"), format!("object:{llvm_ir}"))?;
        fs::write(hsaco_path, format!("hsaco:{llvm_ir}"))?;
        Ok(())
    }

    fn run(
        output: &Path,
        producer: &ProducerIdentity,
        kernels: &[TestKernel],
    ) -> Result<Vec<DeviceArtifact>, EmitError> {
        emit_artifact_transaction(
            output,
            producer,
            kernels,
            |kernel| kernel.name,
            |kernel| {
                if kernel.valid {
                    Ok(format!("{}:{}", kernel.generation, kernel.name))
                } else {
                    Err(EmitError::UnsupportedKernel {
                        kernel: kernel.name.to_string(),
                        reason: "injected preflight failure".to_string(),
                    })
                }
            },
            fake_compile,
        )
    }

    fn run_with_faults(
        output: &Path,
        producer: &ProducerIdentity,
        kernels: &[TestKernel],
        faults: &mut Faults,
        compile: impl FnMut(&Path, &Path) -> Result<(), EmitError>,
    ) -> Result<Vec<DeviceArtifact>, EmitError> {
        emit_artifact_transaction_with_hooks(
            output,
            producer,
            kernels,
            |kernel| kernel.name,
            |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
            compile,
            faults,
        )
    }

    fn one(name: &'static str, generation: &'static str) -> [TestKernel; 1] {
        [TestKernel {
            name,
            generation,
            valid: true,
        }]
    }

    fn read_owned(output: &Path, producer: &ProducerIdentity) -> BTreeSet<String> {
        let pinned = PinnedOutput::open(output).unwrap();
        let _lock = pinned.lock().unwrap();
        read_registry(&pinned).unwrap().owned_by(producer)
    }

    fn assert_generation(output: &Path, names: &[&str], generation: &str) {
        for name in names {
            assert_eq!(
                fs::read_to_string(output.join(format!("{name}.ll"))).unwrap(),
                format!("{generation}:{name}")
            );
            assert_eq!(
                fs::read_to_string(output.join(format!("{name}.o"))).unwrap(),
                format!("object:{generation}:{name}")
            );
            assert_eq!(
                fs::read_to_string(output.join(format!("{name}.hsaco"))).unwrap(),
                format!("hsaco:{generation}:{name}")
            );
        }
    }

    fn assert_absent(output: &Path, names: &[&str]) {
        for name in names {
            for extension in ["ll", "o", "hsaco"] {
                assert!(!output.join(format!("{name}.{extension}")).exists());
            }
        }
    }

    fn assert_no_staging(output: &Path) {
        let staging = fs::read_dir(output)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| is_staging_name(entry.file_name().to_string_lossy().as_bytes()))
            .count();
        assert_eq!(staging, 0);
    }

    #[test]
    fn registry_is_bounded_canonical_and_non_authoritative_bookkeeping() {
        let producer = ProducerIdentity::for_test("producer", "/workspace/src/lib.rs");
        let mut registry = OwnershipRegistry::default();
        registry.set_owned(
            &producer,
            ["alpha".to_string(), "beta".to_string()]
                .into_iter()
                .collect(),
        );

        let encoded = registry.encode().unwrap();
        assert_eq!(OwnershipRegistry::decode(&encoded).unwrap(), registry);
        let mut trailing = encoded;
        trailing.push(0);
        assert!(OwnershipRegistry::decode(&trailing).is_err());
        assert!(matches!(
            ProducerIdentity::from_codegen(
                "producer",
                Some(Path::new(&"x".repeat(MAX_PRODUCER_SOURCE_BYTES + 1)))
            ),
            Err(EmitError::InvalidProducer { .. })
        ));

        let renamed = ProducerIdentity::for_test("renamed_crate", "/workspace/src/lib.rs");
        assert_eq!(producer.stable_source, renamed.stable_source);
        assert_ne!(producer.crate_name, renamed.crate_name);
    }

    #[test]
    fn rejects_unsafe_and_case_folded_duplicate_names_before_compile() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let outside = temp.path.join("escape.hsaco");
        fs::write(&outside, b"keep").unwrap();
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let compile_calls = Cell::new(0usize);
        let unsafe_names = [
            TestKernel {
                name: "valid",
                generation: "new",
                valid: true,
            },
            TestKernel {
                name: "../escape",
                generation: "new",
                valid: true,
            },
        ];

        let error = emit_artifact_transaction(
            &output,
            &producer,
            &unsafe_names,
            |kernel| kernel.name,
            |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
            |llvm_ir, hsaco| {
                compile_calls.set(compile_calls.get() + 1);
                fake_compile(llvm_ir, hsaco)
            },
        )
        .unwrap_err();
        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(matches!(
            transaction.primary.as_deref(),
            Some(EmitError::InvalidArtifactName { .. })
        ));
        assert_eq!(compile_calls.get(), 0);
        assert_eq!(fs::read(&outside).unwrap(), b"keep");
        assert_absent(&output, &["valid"]);

        let duplicate_names = [
            TestKernel {
                name: "Kernel",
                generation: "new",
                valid: true,
            },
            TestKernel {
                name: "kernel",
                generation: "new",
                valid: true,
            },
        ];
        let error = run(&output, &producer, &duplicate_names).unwrap_err();
        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(matches!(
            transaction.primary.as_deref(),
            Some(EmitError::DuplicateArtifactName { .. })
        ));
        assert_no_staging(&output);
    }

    #[test]
    fn missing_compiler_output_fails_closed() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let kernels = one("alpha", "new");

        let error = emit_artifact_transaction(
            &output,
            &producer,
            &kernels,
            |kernel| kernel.name,
            |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
            |_llvm_ir, hsaco| {
                fs::write(hsaco, b"hsaco without object")?;
                Ok(())
            },
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(matches!(
            transaction.primary.as_deref(),
            Some(EmitError::MissingStagedArtifact { .. })
        ));
        assert_absent(&output, &["alpha"]);
        assert_no_staging(&output);
    }

    #[test]
    fn output_symlink_is_rejected_without_touching_its_target() {
        use std::os::unix::fs::symlink;

        let temp = TestDirectory::new();
        let target = temp.path.join("target");
        let output = temp.path.join("output");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("unrelated"), b"keep").unwrap();
        symlink(&target, &output).unwrap();

        let error = run(
            &output,
            &ProducerIdentity::for_test("producer", "/src/producer.rs"),
            &one("alpha", "a"),
        )
        .unwrap_err();

        assert!(matches!(error, EmitError::Io(_)));
        assert_eq!(fs::read(target.join("unrelated")).unwrap(), b"keep");
        assert_absent(&target, &["alpha"]);
    }

    #[test]
    fn parent_component_symlink_is_rejected_without_touching_its_target() {
        use std::os::unix::fs::symlink;

        let temp = TestDirectory::new();
        let real_parent = temp.path.join("real-parent");
        let linked_parent = temp.path.join("linked-parent");
        let target = real_parent.join("output");
        fs::create_dir(&real_parent).unwrap();
        fs::create_dir(&target).unwrap();
        fs::write(target.join("unrelated"), b"keep").unwrap();
        symlink(&real_parent, &linked_parent).unwrap();

        let error = run(
            &linked_parent.join("output"),
            &ProducerIdentity::for_test("producer", "/src/producer.rs"),
            &one("alpha", "a"),
        )
        .unwrap_err();

        assert!(matches!(error, EmitError::Io(_)));
        assert_eq!(fs::read(target.join("unrelated")).unwrap(), b"keep");
        assert_absent(&target, &["alpha"]);
    }

    #[test]
    fn parent_directory_path_is_rejected_before_creating_any_prefix() {
        let temp = TestDirectory::new();
        let created_prefix = temp.path.join("must-not-exist");
        let output = created_prefix.join("..").join("output");

        let error = run(
            &output,
            &ProducerIdentity::for_test("producer", "/src/producer.rs"),
            &one("alpha", "a"),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            EmitError::InvalidArtifactDestination { .. }
        ));
        assert!(!created_prefix.exists());
    }

    #[test]
    fn hardlinked_lock_is_rejected_without_mutating_the_other_inode() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let unrelated = temp.path.join("unrelated");
        fs::create_dir(&output).unwrap();
        fs::write(&unrelated, b"keep").unwrap();
        fs::set_permissions(&unrelated, fs::Permissions::from_mode(0o640)).unwrap();
        fs::hard_link(&unrelated, output.join(LOCK_FILE)).unwrap();

        let error = run(
            &output,
            &ProducerIdentity::for_test("producer", "/src/producer.rs"),
            &one("alpha", "a"),
        )
        .unwrap_err();

        assert!(matches!(
            error,
            EmitError::InvalidArtifactDestination { .. }
        ));
        assert_eq!(fs::read(&unrelated).unwrap(), b"keep");
        assert_eq!(
            fs::metadata(&unrelated).unwrap().permissions().mode() & 0o777,
            0o640
        );
        assert_eq!(fs::metadata(&unrelated).unwrap().nlink(), 2);
    }

    #[test]
    fn pinned_directory_substitution_fails_without_writing_replacement() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let relocated = temp.path.join("relocated");
        fs::create_dir(&output).unwrap();
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let kernels = one("alpha", "a");

        let error = emit_artifact_transaction(
            &output,
            &producer,
            &kernels,
            |kernel| kernel.name,
            |kernel| {
                fs::rename(&output, &relocated)?;
                fs::create_dir(&output)?;
                Ok(format!("{}:{}", kernel.generation, kernel.name))
            },
            fake_compile,
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(matches!(
            transaction.primary.as_deref(),
            Some(EmitError::OutputDirectoryChanged { .. })
        ));
        assert_absent(&output, &["alpha"]);
        assert_absent(&relocated, &["alpha"]);
        assert_no_staging(&relocated);
    }

    #[test]
    fn pinned_parent_substitution_fails_without_writing_replacement() {
        let temp = TestDirectory::new();
        let parent = temp.path.join("parent");
        let output = parent.join("output");
        let relocated_parent = temp.path.join("relocated-parent");
        fs::create_dir(&parent).unwrap();
        fs::create_dir(&output).unwrap();
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let kernels = one("alpha", "a");

        let error = emit_artifact_transaction(
            &output,
            &producer,
            &kernels,
            |kernel| kernel.name,
            |kernel| {
                fs::rename(&parent, &relocated_parent)?;
                fs::create_dir(&parent)?;
                fs::create_dir(parent.join("output"))?;
                Ok(format!("{}:{}", kernel.generation, kernel.name))
            },
            fake_compile,
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(matches!(
            transaction.primary.as_deref(),
            Some(EmitError::OutputDirectoryChanged { .. })
        ));
        assert_absent(&output, &["alpha"]);
        assert_absent(&relocated_parent.join("output"), &["alpha"]);
        assert_no_staging(&relocated_parent.join("output"));
    }

    #[test]
    fn entire_collection_is_preflighted_before_compile_and_stale_outputs_are_invalidated() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let old = [
            TestKernel {
                name: "alpha",
                generation: "old",
                valid: true,
            },
            TestKernel {
                name: "beta",
                generation: "old",
                valid: true,
            },
        ];
        run(&output, &producer, &old).unwrap();
        let next = [
            TestKernel {
                name: "alpha",
                generation: "new",
                valid: true,
            },
            TestKernel {
                name: "beta",
                generation: "new",
                valid: false,
            },
        ];
        let compile_calls = Cell::new(0usize);

        let error = emit_artifact_transaction(
            &output,
            &producer,
            &next,
            |kernel| kernel.name,
            |kernel| {
                if kernel.valid {
                    Ok(format!("{}:{}", kernel.generation, kernel.name))
                } else {
                    Err(EmitError::UnsupportedKernel {
                        kernel: kernel.name.to_string(),
                        reason: "injected preflight failure".to_string(),
                    })
                }
            },
            |llvm_ir, hsaco| {
                compile_calls.set(compile_calls.get() + 1);
                fake_compile(llvm_ir, hsaco)
            },
        )
        .unwrap_err();

        assert_eq!(compile_calls.get(), 0);
        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(matches!(
            transaction.primary.as_deref(),
            Some(EmitError::UnsupportedKernel { .. })
        ));
        assert_eq!(
            transaction.publication,
            PublicationState::NotStarted {
                total_final_renames: 6,
            }
        );
        assert_absent(&output, &["alpha", "beta"]);
        assert!(read_owned(&output, &producer).is_empty());
        assert_no_staging(&output);
    }

    #[test]
    fn failure_before_kernel_discovery_invalidates_previous_outputs() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        run(&output, &producer, &one("alpha", "old")).unwrap();

        let error = emit_artifact_transaction_after_preflight(
            &output,
            &producer,
            || -> Result<Vec<TestKernel>, EmitError> {
                Err(EmitError::Preflight {
                    reason: "injected collection failure".to_string(),
                })
            },
            |kernel| kernel.name,
            |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
            fake_compile,
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(matches!(
            transaction.primary.as_deref(),
            Some(EmitError::Preflight { .. })
        ));
        assert_eq!(
            transaction.publication,
            PublicationState::NotStarted {
                total_final_renames: 0,
            }
        );
        assert_absent(&output, &["alpha"]);
        assert!(read_owned(&output, &producer).is_empty());
        assert_no_staging(&output);
    }

    #[test]
    fn staging_creation_failure_repairs_and_persists_ownership() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        run(&output, &producer, &one("alpha", "old")).unwrap();
        let mut faults = Faults {
            fail_stage_create: true,
            ..Faults::default()
        };

        let error = run_with_faults(
            &output,
            &producer,
            &one("alpha", "new"),
            &mut faults,
            fake_compile,
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(transaction.primary.is_some());
        assert!(transaction.cleanup_failures.is_empty());
        assert_absent(&output, &["alpha"]);
        assert!(read_owned(&output, &producer).is_empty());
        assert!(!output.join(RECOVERY_OWNERSHIP_FILE).exists());
        assert_no_staging(&output);
    }

    #[test]
    fn fully_absent_foreign_ownership_tombstone_is_pruned() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let old_producer = ProducerIdentity::for_test("old_producer", "/src/old.rs");
        let new_producer = ProducerIdentity::for_test("new_producer", "/src/new.rs");
        run(&output, &old_producer, &one("alpha", "old")).unwrap();
        for extension in ["ll", "o", "hsaco"] {
            fs::remove_file(output.join(format!("alpha.{extension}"))).unwrap();
        }

        run(&output, &new_producer, &one("alpha", "new")).unwrap();

        assert_generation(&output, &["alpha"], "new");
        assert!(read_owned(&output, &old_producer).is_empty());
        assert_eq!(
            read_owned(&output, &new_producer),
            ["alpha".to_string()].into()
        );
        assert_no_staging(&output);
    }

    #[test]
    fn staging_setup_cleanup_failure_is_reported_and_scavenged() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let mut faults = Faults {
            fail_stage_stat: true,
            fail_cleanup: true,
            ..Faults::default()
        };

        let error = run_with_faults(
            &output,
            &producer,
            &one("alpha", "new"),
            &mut faults,
            fake_compile,
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert_eq!(transaction.cleanup_failures.len(), 1);
        assert!(fs::read_dir(&output).unwrap().any(|entry| {
            is_staging_name(entry.unwrap().file_name().to_string_lossy().as_bytes())
        }));

        run(&output, &producer, &one("alpha", "recovered")).unwrap();
        assert_generation(&output, &["alpha"], "recovered");
        assert_no_staging(&output);
    }

    #[test]
    fn compile_failure_cleans_all_staged_outputs_and_publishes_nothing() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let kernels = [
            TestKernel {
                name: "alpha",
                generation: "new",
                valid: true,
            },
            TestKernel {
                name: "beta",
                generation: "new",
                valid: true,
            },
        ];

        let error = emit_artifact_transaction(
            &output,
            &producer,
            &kernels,
            |kernel| kernel.name,
            |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
            |llvm_ir, hsaco| {
                if fs::read_to_string(llvm_ir)?.contains("beta") {
                    Err(io::Error::other("injected compile failure").into())
                } else {
                    fake_compile(llvm_ir, hsaco)
                }
            },
        )
        .unwrap_err();

        assert!(matches!(error, EmitError::Transaction(_)));
        assert_absent(&output, &["alpha", "beta"]);
        assert!(read_owned(&output, &producer).is_empty());
        assert_no_staging(&output);
    }

    #[test]
    fn pre_registry_artifacts_are_replaced_and_adopted() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        fs::create_dir(&output).unwrap();
        for extension in ["ll", "o", "hsaco"] {
            fs::write(output.join(format!("alpha.{extension}")), b"legacy").unwrap();
        }
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");

        run(&output, &producer, &one("alpha", "new")).unwrap();

        assert_generation(&output, &["alpha"], "new");
        assert_eq!(read_owned(&output, &producer), ["alpha".to_string()].into());
        assert_no_staging(&output);
    }

    #[test]
    fn pre_registry_artifacts_are_invalidated_on_preflight_failure() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        fs::create_dir(&output).unwrap();
        for extension in ["ll", "o", "hsaco"] {
            fs::write(output.join(format!("alpha.{extension}")), b"stale").unwrap();
        }
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let invalid = [TestKernel {
            name: "alpha",
            generation: "new",
            valid: false,
        }];

        let error = run(&output, &producer, &invalid).unwrap_err();
        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(matches!(
            transaction.primary.as_deref(),
            Some(EmitError::UnsupportedKernel { .. })
        ));
        assert_absent(&output, &["alpha"]);
        assert!(read_owned(&output, &producer).is_empty());
        assert_no_staging(&output);
    }

    #[test]
    fn pre_registry_removed_and_renamed_kernels_are_scavenged() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        fs::create_dir(&output).unwrap();
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");

        for extension in ["ll", "o", "hsaco"] {
            fs::write(output.join(format!("removed.{extension}")), b"legacy").unwrap();
            fs::write(output.join(format!("old_name.{extension}")), b"legacy").unwrap();
        }
        run(&output, &producer, &one("new_name", "new")).unwrap();

        assert_absent(&output, &["removed", "old_name"]);
        assert_generation(&output, &["new_name"], "new");
        assert_eq!(
            read_owned(&output, &producer),
            ["new_name".to_string()].into()
        );

        for extension in ["ll", "o", "hsaco"] {
            fs::write(output.join(format!("zeroed.{extension}")), b"orphan").unwrap();
        }
        run(&output, &producer, &[]).unwrap();
        assert_absent(&output, &["new_name", "zeroed"]);
        assert!(read_owned(&output, &producer).is_empty());
        assert_no_staging(&output);
    }

    #[test]
    fn abandoned_staging_is_scavenged_without_touching_noncanonical_files() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let abandoned = output.join(format!("{STAGING_PREFIX}999-1"));
        fs::create_dir_all(&abandoned).unwrap();
        fs::set_permissions(&abandoned, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(abandoned.join("alpha.ll"), b"partial").unwrap();
        fs::write(output.join("keep.txt"), b"keep").unwrap();
        fs::write(output.join("not-a-kernel.ll"), b"keep").unwrap();
        fs::write(output.join(".fe2o3-stage-not-reserved"), b"keep").unwrap();
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");

        run(&output, &producer, &one("alpha", "new")).unwrap();

        assert!(!abandoned.exists());
        assert_eq!(fs::read(output.join("keep.txt")).unwrap(), b"keep");
        assert_eq!(fs::read(output.join("not-a-kernel.ll")).unwrap(), b"keep");
        assert_eq!(
            fs::read(output.join(".fe2o3-stage-not-reserved")).unwrap(),
            b"keep"
        );
        assert_generation(&output, &["alpha"], "new");
        assert_no_staging(&output);
    }

    #[test]
    fn concurrent_generations_are_serialized_and_never_mix() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let first_output = output.clone();
        let first_producer = producer.clone();
        let (first_entered_tx, first_entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let first = thread::spawn(move || {
            let kernels = [
                TestKernel {
                    name: "alpha",
                    generation: "first",
                    valid: true,
                },
                TestKernel {
                    name: "beta",
                    generation: "first",
                    valid: true,
                },
            ];
            emit_artifact_transaction(
                &first_output,
                &first_producer,
                &kernels,
                |kernel| kernel.name,
                |kernel| {
                    if kernel.name == "alpha" {
                        first_entered_tx.send(()).unwrap();
                        release_rx.recv().unwrap();
                    }
                    Ok(format!("{}:{}", kernel.generation, kernel.name))
                },
                fake_compile,
            )
        });
        first_entered_rx.recv().unwrap();

        let second_output = output.clone();
        let second_producer = producer.clone();
        let (second_entered_tx, second_entered_rx) = mpsc::channel();
        let second = thread::spawn(move || {
            let kernels = [
                TestKernel {
                    name: "alpha",
                    generation: "second",
                    valid: true,
                },
                TestKernel {
                    name: "beta",
                    generation: "second",
                    valid: true,
                },
            ];
            emit_artifact_transaction(
                &second_output,
                &second_producer,
                &kernels,
                |kernel| kernel.name,
                |kernel| {
                    if kernel.name == "alpha" {
                        second_entered_tx.send(()).unwrap();
                    }
                    Ok(format!("{}:{}", kernel.generation, kernel.name))
                },
                fake_compile,
            )
        });

        assert!(
            second_entered_rx
                .recv_timeout(Duration::from_millis(150))
                .is_err()
        );
        release_tx.send(()).unwrap();
        first.join().unwrap().unwrap();
        second_entered_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        second.join().unwrap().unwrap();

        assert_generation(&output, &["alpha", "beta"], "second");
        assert_no_staging(&output);
    }

    #[test]
    fn producers_reconcile_only_their_owned_sets_including_zero_kernels() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer_a = ProducerIdentity::for_test("producer_a", "/src/a.rs");
        let producer_b = ProducerIdentity::for_test("producer_b", "/src/b.rs");
        let barrier = Arc::new(Barrier::new(2));

        let a_output = output.clone();
        let a_producer = producer_a.clone();
        let a_barrier = Arc::clone(&barrier);
        let a = thread::spawn(move || {
            a_barrier.wait();
            run(&a_output, &a_producer, &one("alpha", "a"))
        });
        let b_output = output.clone();
        let b_producer = producer_b.clone();
        let b_barrier = Arc::clone(&barrier);
        let b = thread::spawn(move || {
            b_barrier.wait();
            run(&b_output, &b_producer, &one("beta", "b"))
        });
        a.join().unwrap().unwrap();
        b.join().unwrap().unwrap();

        assert_generation(&output, &["alpha"], "a");
        assert_generation(&output, &["beta"], "b");
        run(&output, &producer_a, &[]).unwrap();
        assert_absent(&output, &["alpha"]);
        assert_generation(&output, &["beta"], "b");
        assert!(read_owned(&output, &producer_a).is_empty());
        assert_eq!(
            read_owned(&output, &producer_b),
            ["beta".to_string()].into()
        );
    }

    #[test]
    fn renamed_kernel_removes_the_previous_owned_generation() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        run(&output, &producer, &one("old_name", "old")).unwrap();

        run(&output, &producer, &one("new_name", "new")).unwrap();

        assert_absent(&output, &["old_name"]);
        assert_generation(&output, &["new_name"], "new");
        assert_eq!(
            read_owned(&output, &producer),
            ["new_name".to_string()].into()
        );
    }

    #[test]
    fn partial_publish_is_rolled_back_and_reports_progress() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let kernels = [
            TestKernel {
                name: "alpha",
                generation: "old",
                valid: true,
            },
            TestKernel {
                name: "beta",
                generation: "old",
                valid: true,
            },
        ];
        run(&output, &producer, &kernels).unwrap();
        let next = kernels
            .iter()
            .cloned()
            .map(|mut kernel| {
                kernel.generation = "new";
                kernel
            })
            .collect::<Vec<_>>();
        let mut faults = Faults {
            fail_artifact_rename_at: Some(1),
            ..Faults::default()
        };

        let error =
            run_with_faults(&output, &producer, &next, &mut faults, fake_compile).unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert_eq!(
            transaction.publication,
            PublicationState::Partial {
                completed_final_renames: 1,
                total_final_renames: 6,
            }
        );
        assert!(transaction.primary.is_some());
        assert!(transaction.invalidation_failures.is_empty());
        assert_absent(&output, &["alpha", "beta"]);
        assert!(read_owned(&output, &producer).is_empty());
        assert_no_staging(&output);
    }

    #[test]
    fn ownership_publish_failure_rolls_back_all_final_artifacts() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let mut faults = Faults {
            fail_ownership_rename: true,
            ..Faults::default()
        };

        let error = run_with_faults(
            &output,
            &producer,
            &one("alpha", "new"),
            &mut faults,
            fake_compile,
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert_eq!(
            transaction.publication,
            PublicationState::FinalsPublished { final_renames: 3 }
        );
        assert!(
            transaction
                .primary
                .as_ref()
                .unwrap()
                .to_string()
                .contains("injected ownership rename failure")
        );
        assert_absent(&output, &["alpha"]);
        assert!(read_owned(&output, &producer).is_empty());
        assert_no_staging(&output);
    }

    #[test]
    fn committed_cleanup_failure_is_reported_without_rollback() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let mut faults = Faults {
            fail_cleanup: true,
            ..Faults::default()
        };

        let error = run_with_faults(
            &output,
            &producer,
            &one("alpha", "committed"),
            &mut faults,
            fake_compile,
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(transaction.primary.is_none());
        assert_eq!(
            transaction.publication,
            PublicationState::CommittedWithCleanupFailure { final_renames: 3 }
        );
        assert_eq!(transaction.cleanup_failures.len(), 1);
        assert_generation(&output, &["alpha"], "committed");
        assert_eq!(read_owned(&output, &producer), ["alpha".to_string()].into());
        assert_no_staging(&output);
    }

    #[test]
    fn final_identity_failure_reports_committed_publication() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let relocated = temp.path.join("relocated");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let mut faults = Faults {
            replace_output_after_commit: Some((output.clone(), relocated.clone())),
            ..Faults::default()
        };

        let error = run_with_faults(
            &output,
            &producer,
            &one("alpha", "committed"),
            &mut faults,
            fake_compile,
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(matches!(
            transaction.primary.as_deref(),
            Some(EmitError::OutputDirectoryChanged { .. })
        ));
        assert!(transaction.cleanup_failures.is_empty());
        assert_eq!(
            transaction.publication,
            PublicationState::Committed { final_renames: 3 }
        );
        assert_absent(&output, &["alpha"]);
        assert_generation(&relocated, &["alpha"], "committed");
        assert_eq!(
            read_owned(&relocated, &producer),
            ["alpha".to_string()].into()
        );
        assert_no_staging(&relocated);
    }

    #[test]
    fn final_identity_and_cleanup_failures_report_committed_with_cleanup_failure() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let relocated = temp.path.join("relocated");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let mut faults = Faults {
            fail_cleanup: true,
            replace_output_after_commit: Some((output.clone(), relocated.clone())),
            ..Faults::default()
        };

        let error = run_with_faults(
            &output,
            &producer,
            &one("alpha", "committed"),
            &mut faults,
            fake_compile,
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(matches!(
            transaction.primary.as_deref(),
            Some(EmitError::OutputDirectoryChanged { .. })
        ));
        assert_eq!(transaction.cleanup_failures.len(), 1);
        assert_eq!(
            transaction.publication,
            PublicationState::CommittedWithCleanupFailure { final_renames: 3 }
        );
        assert_absent(&output, &["alpha"]);
        assert_generation(&relocated, &["alpha"], "committed");
        assert_eq!(
            read_owned(&relocated, &producer),
            ["alpha".to_string()].into()
        );
        assert_no_staging(&relocated);
    }

    #[test]
    fn composite_error_preserves_primary_invalidation_and_cleanup_failures() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        run(&output, &producer, &one("alpha", "old")).unwrap();
        let mut faults = Faults {
            fail_invalidate_entry: Some("alpha.hsaco".to_string()),
            fail_cleanup: true,
            ..Faults::default()
        };

        let error = run_with_faults(
            &output,
            &producer,
            &one("alpha", "new"),
            &mut faults,
            |_llvm_ir, _hsaco| Err(io::Error::other("injected compiler failure").into()),
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(
            transaction
                .primary
                .as_ref()
                .unwrap()
                .to_string()
                .contains("injected compiler failure")
        );
        assert_eq!(transaction.invalidation_failures.len(), 1);
        assert_eq!(transaction.cleanup_failures.len(), 1);
        assert_eq!(
            transaction.publication,
            PublicationState::NotStarted {
                total_final_renames: 3,
            }
        );
        assert!(!output.join("alpha.ll").exists());
        assert!(!output.join("alpha.o").exists());
        assert!(output.join("alpha.hsaco").exists());
        assert_eq!(read_owned(&output, &producer), ["alpha".to_string()].into());
        assert_no_staging(&output);
    }

    #[test]
    fn private_staging_and_subprocess_boundary_publish_successfully() {
        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");
        let observed_mode = Arc::new(AtomicU64::new(0));
        let mode = Arc::clone(&observed_mode);

        emit_artifact_transaction(
            &output,
            &producer,
            &one("alpha", "a"),
            |kernel| kernel.name,
            |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
            move |llvm_ir, hsaco| {
                let staging = llvm_ir.parent().unwrap();
                mode.store(
                    fs::metadata(staging)?.permissions().mode() as u64 & 0o777,
                    Ordering::Relaxed,
                );
                fake_compile(llvm_ir, hsaco)
            },
        )
        .unwrap();

        assert_eq!(observed_mode.load(Ordering::Relaxed), 0o700);
        assert_generation(&output, &["alpha"], "a");
    }

    #[test]
    fn subprocess_fd_is_not_redirected_by_staging_name_substitution() {
        use std::os::unix::fs::symlink;

        let temp = TestDirectory::new();
        let output = temp.path.join("output");
        let relocated_stage = temp.path.join("relocated-stage");
        let outside = temp.path.join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("sentinel"), b"keep").unwrap();
        let producer = ProducerIdentity::for_test("producer", "/src/producer.rs");

        let error = emit_artifact_transaction(
            &output,
            &producer,
            &one("alpha", "a"),
            |kernel| kernel.name,
            |kernel| Ok(format!("{}:{}", kernel.generation, kernel.name)),
            |llvm_ir, hsaco| {
                let stage_entry = fs::read_dir(&output)?
                    .filter_map(Result::ok)
                    .find(|entry| {
                        entry
                            .file_name()
                            .to_string_lossy()
                            .starts_with(".fe2o3-stage-")
                    })
                    .expect("staging entry must exist")
                    .path();
                fs::rename(&stage_entry, &relocated_stage)?;
                symlink(&outside, &stage_entry)?;

                assert_eq!(
                    fs::canonicalize(llvm_ir.parent().unwrap())?,
                    fs::canonicalize(&relocated_stage)?
                );
                let object = hsaco.with_extension("o");
                let status = process::Command::new("sh")
                    .args([
                        "-c",
                        "ir=$(cat \"$1\") || exit; printf 'object:%s' \"$ir\" > \"$2\"; printf 'hsaco:%s' \"$ir\" > \"$3\"",
                        "sh",
                    ])
                    .arg(llvm_ir)
                    .arg(&object)
                    .arg(hsaco)
                    .status()?;
                if status.success() {
                    Ok(())
                } else {
                    Err(io::Error::other(format!("test subprocess failed: {status}")).into())
                }
            },
        )
        .unwrap_err();

        let EmitError::Transaction(transaction) = error else {
            panic!("expected composite transaction error");
        };
        assert!(transaction.primary.is_none());
        assert_eq!(transaction.cleanup_failures.len(), 1);
        assert_eq!(
            transaction.publication,
            PublicationState::CommittedWithCleanupFailure { final_renames: 3 }
        );
        assert_generation(&output, &["alpha"], "a");
        assert_eq!(fs::read(outside.join("sentinel")).unwrap(), b"keep");
        assert_absent(&outside, &["alpha"]);
        assert!(
            fs::symlink_metadata(
                fs::read_dir(&output)
                    .unwrap()
                    .filter_map(Result::ok)
                    .find(|entry| {
                        entry
                            .file_name()
                            .to_string_lossy()
                            .starts_with(".fe2o3-stage-")
                    })
                    .unwrap()
                    .path()
            )
            .unwrap()
            .file_type()
            .is_symlink()
        );
        assert_eq!(fs::read_dir(&relocated_stage).unwrap().count(), 0);
    }
}

#[derive(Debug)]
pub(crate) struct FilesystemFailure {
    pub(crate) operation: &'static str,
    pub(crate) entry: String,
    pub(crate) error: io::Error,
}

impl fmt::Display for FilesystemFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} entry {} failed: {}",
            self.operation, self.entry, self.error
        )
    }
}

#[derive(Debug)]
pub(crate) struct ArtifactTransactionError {
    pub(crate) primary: Option<Box<EmitError>>,
    pub(crate) cleanup_failures: Vec<FilesystemFailure>,
    pub(crate) invalidation_failures: Vec<FilesystemFailure>,
    pub(crate) publication: PublicationState,
}

impl fmt::Display for ArtifactTransactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.primary {
            Some(primary) => write!(f, "artifact transaction failed: {primary}")?,
            None => write!(f, "artifact transaction cleanup failed after commit")?,
        }
        write!(f, "; publication state: {:?}", self.publication)?;
        for failure in &self.invalidation_failures {
            write!(f, "; invalidation: {failure}")?;
        }
        for failure in &self.cleanup_failures {
            write!(f, "; cleanup: {failure}")?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RenameKind {
    Artifact,
    Ownership,
}

trait TransactionHooks {
    fn before_stage_create(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn before_stage_stat(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn before_rename(&mut self, _kind: RenameKind, _completed: usize) -> io::Result<()> {
        Ok(())
    }

    fn before_invalidate(&mut self, _entry: &str) -> io::Result<()> {
        Ok(())
    }

    fn before_stage_cleanup(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn after_registry_commit(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct NoFaults;

impl TransactionHooks for NoFaults {}

struct PinnedOutput {
    fd: OwnedFd,
    display_path: PathBuf,
    device: u64,
    inode: u64,
}

impl PinnedOutput {
    fn open(path: &Path) -> Result<Self, EmitError> {
        let fd = open_directory_walk(path, true)?;
        let stat = fstat(&fd).map_err(std::io::Error::from)?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::Directory {
            return Err(EmitError::InvalidArtifactDestination {
                path: path.to_path_buf(),
                reason: "output path is not a directory".to_string(),
            });
        }
        Ok(Self {
            fd,
            display_path: path.to_path_buf(),
            device: stat.st_dev,
            inode: stat.st_ino,
        })
    }

    fn verify_path_identity(&self) -> Result<(), EmitError> {
        let reopened = open_directory_walk(&self.display_path, false)?;
        let stat = fstat(&reopened).map_err(std::io::Error::from)?;
        if stat.st_dev != self.device || stat.st_ino != self.inode {
            return Err(EmitError::OutputDirectoryChanged {
                path: self.display_path.clone(),
            });
        }
        Ok(())
    }

    fn lock(&self) -> Result<OutputLock, EmitError> {
        let fd = openat(
            &self.fd,
            LOCK_FILE,
            OFlags::RDWR | OFlags::CREATE | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(std::io::Error::from)?;
        let validate_lock = |stat: &rustix::fs::Stat| {
            if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
                return Err(EmitError::InvalidArtifactDestination {
                    path: self.display_path.join(LOCK_FILE),
                    reason: "lock entry is not a regular file".to_string(),
                });
            }
            if stat.st_nlink != 1 || stat.st_mode & 0o077 != 0 {
                return Err(EmitError::InvalidArtifactDestination {
                    path: self.display_path.join(LOCK_FILE),
                    reason: "lock entry must be private and have exactly one link".to_string(),
                });
            }
            Ok(())
        };
        let validate_path_identity = |fd_stat: &rustix::fs::Stat| {
            let path_stat = statat(&self.fd, LOCK_FILE, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(std::io::Error::from)?;
            validate_lock(&path_stat)?;
            if path_stat.st_dev != fd_stat.st_dev || path_stat.st_ino != fd_stat.st_ino {
                return Err(EmitError::InvalidArtifactDestination {
                    path: self.display_path.join(LOCK_FILE),
                    reason: "lock entry changed while it was being acquired".to_string(),
                });
            }
            Ok(())
        };
        let stat = fstat(&fd).map_err(std::io::Error::from)?;
        validate_lock(&stat)?;
        validate_path_identity(&stat)?;
        flock(&fd, FlockOperation::LockExclusive).map_err(std::io::Error::from)?;
        let locked_stat = fstat(&fd).map_err(std::io::Error::from)?;
        if let Err(error) =
            validate_lock(&locked_stat).and_then(|()| validate_path_identity(&locked_stat))
        {
            let _ = flock(&fd, FlockOperation::Unlock);
            return Err(error);
        }
        Ok(OutputLock { _fd: fd })
    }
}

fn open_directory_walk(path: &Path, create: bool) -> Result<OwnedFd, EmitError> {
    let absolute = path.is_absolute();
    let mut names = Vec::new();
    for component in path.components() {
        match component {
            Component::RootDir | Component::CurDir => {}
            Component::Normal(name) => names.push(name),
            Component::ParentDir => {
                return Err(EmitError::InvalidArtifactDestination {
                    path: path.to_path_buf(),
                    reason: "parent-directory components are not allowed".to_string(),
                });
            }
            Component::Prefix(_) => {
                return Err(EmitError::InvalidArtifactDestination {
                    path: path.to_path_buf(),
                    reason: "platform path prefixes are not supported".to_string(),
                });
            }
        }
    }

    let mut current = open(
        if absolute {
            Path::new("/")
        } else {
            Path::new(".")
        },
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(std::io::Error::from)?;

    for name in names {
        let open_component = || {
            openat(
                &current,
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
        };
        current = match open_component() {
            Ok(fd) => fd,
            Err(error) if create && error == rustix::io::Errno::NOENT => {
                match mkdirat(
                    &current,
                    name,
                    Mode::RUSR
                        | Mode::WUSR
                        | Mode::XUSR
                        | Mode::RGRP
                        | Mode::XGRP
                        | Mode::ROTH
                        | Mode::XOTH,
                ) {
                    Ok(()) => {}
                    Err(error) if error == rustix::io::Errno::EXIST => {}
                    Err(error) => return Err(std::io::Error::from(error).into()),
                }
                open_component().map_err(std::io::Error::from)?
            }
            Err(error) => return Err(std::io::Error::from(error).into()),
        };
    }
    Ok(current)
}

struct OutputLock {
    _fd: OwnedFd,
}

struct StagingDirectory {
    output_fd: OwnedFd,
    fd: OwnedFd,
    subprocess_fd: OwnedFd,
    name: String,
    device: u64,
    inode: u64,
    active: bool,
}

struct StagingCreateError {
    primary: EmitError,
    cleanup_failures: Vec<FilesystemFailure>,
}

impl StagingCreateError {
    fn new(primary: impl Into<EmitError>) -> Self {
        Self {
            primary: primary.into(),
            cleanup_failures: Vec::new(),
        }
    }
}

impl StagingDirectory {
    fn create(
        output: &PinnedOutput,
        hooks: &mut impl TransactionHooks,
    ) -> Result<Self, StagingCreateError> {
        hooks
            .before_stage_create()
            .map_err(|error| StagingCreateError::new(EmitError::from(error)))?;
        let start = NEXT_STAGING_ID.fetch_add(MAX_STAGING_ATTEMPTS, Ordering::Relaxed);
        for offset in 0..MAX_STAGING_ATTEMPTS {
            let name = format!(
                "{STAGING_PREFIX}{}-{}",
                process::id(),
                start.wrapping_add(offset)
            );
            match mkdirat(
                &output.fd,
                name.as_str(),
                Mode::RUSR | Mode::WUSR | Mode::XUSR,
            ) {
                Ok(()) => {
                    let fd = match openat(
                        &output.fd,
                        name.as_str(),
                        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                        Mode::empty(),
                    ) {
                        Ok(fd) => fd,
                        Err(error) => {
                            let mut failure = StagingCreateError::new(EmitError::from(
                                std::io::Error::from(error),
                            ));
                            if let Err(cleanup_error) =
                                unlinkat(&output.fd, name.as_str(), AtFlags::REMOVEDIR)
                            {
                                failure.cleanup_failures.push(FilesystemFailure {
                                    operation: "remove unopened staging directory",
                                    entry: name,
                                    error: cleanup_error.into(),
                                });
                            }
                            return Err(failure);
                        }
                    };
                    let stat = match hooks
                        .before_stage_stat()
                        .and_then(|()| fstat(&fd).map_err(std::io::Error::from))
                    {
                        Ok(stat) => stat,
                        Err(error) => {
                            let mut failure = StagingCreateError::new(EmitError::from(error));
                            match fstat(&fd) {
                                Ok(stat) => {
                                    failure.cleanup_failures.extend(cleanup_created_staging(
                                        output,
                                        &fd,
                                        &name,
                                        stat.st_dev,
                                        stat.st_ino,
                                        hooks,
                                    ))
                                }
                                Err(cleanup_error) => {
                                    failure.cleanup_failures.push(FilesystemFailure {
                                        operation: "identify staging directory for cleanup",
                                        entry: name,
                                        error: cleanup_error.into(),
                                    });
                                }
                            }
                            return Err(failure);
                        }
                    };
                    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory
                        || stat.st_mode & 0o777 != 0o700
                    {
                        let mut failure =
                            StagingCreateError::new(EmitError::InvalidArtifactDestination {
                                path: output.display_path.join(&name),
                                reason: "staging directory is not a private 0700 directory"
                                    .to_string(),
                            });
                        failure.cleanup_failures.extend(cleanup_created_staging(
                            output,
                            &fd,
                            &name,
                            stat.st_dev,
                            stat.st_ino,
                            hooks,
                        ));
                        return Err(failure);
                    }
                    let output_fd = match rustix::io::fcntl_dupfd_cloexec(&output.fd, 0) {
                        Ok(fd) => fd,
                        Err(error) => {
                            let mut failure = StagingCreateError::new(EmitError::from(
                                std::io::Error::from(error),
                            ));
                            failure.cleanup_failures.extend(cleanup_created_staging(
                                output,
                                &fd,
                                &name,
                                stat.st_dev,
                                stat.st_ino,
                                hooks,
                            ));
                            return Err(failure);
                        }
                    };
                    let subprocess_fd = match rustix::io::dup(&fd) {
                        Ok(fd) => fd,
                        Err(error) => {
                            let mut failure = StagingCreateError::new(EmitError::from(
                                std::io::Error::from(error),
                            ));
                            failure.cleanup_failures.extend(cleanup_created_staging(
                                output,
                                &fd,
                                &name,
                                stat.st_dev,
                                stat.st_ino,
                                hooks,
                            ));
                            return Err(failure);
                        }
                    };
                    let proc_path = format!("/proc/self/fd/{}", subprocess_fd.as_raw_fd());
                    if !Path::new(&proc_path).is_dir() {
                        let mut failure =
                            StagingCreateError::new(EmitError::SubprocessPathBoundary {
                                reason: format!("pinned directory path {proc_path} is unavailable"),
                            });
                        failure.cleanup_failures.extend(cleanup_created_staging(
                            output,
                            &fd,
                            &name,
                            stat.st_dev,
                            stat.st_ino,
                            hooks,
                        ));
                        return Err(failure);
                    }
                    return Ok(Self {
                        output_fd,
                        fd,
                        subprocess_fd,
                        name,
                        device: stat.st_dev,
                        inode: stat.st_ino,
                        active: true,
                    });
                }
                Err(error) if error == rustix::io::Errno::EXIST => {}
                Err(error) => {
                    return Err(StagingCreateError::new(EmitError::from(
                        std::io::Error::from(error),
                    )));
                }
            }
        }
        Err(StagingCreateError::new(EmitError::StagingExhausted {
            output_dir: output.display_path.clone(),
        }))
    }

    fn write(&self, name: &str, bytes: &[u8]) -> Result<(), EmitError> {
        let fd = openat(
            &self.fd,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(std::io::Error::from)?;
        let mut file = fs::File::from(fd);
        file.write_all(bytes)?;
        file.sync_all()?;
        Ok(())
    }

    fn replace(&self, name: &str, bytes: &[u8]) -> Result<(), EmitError> {
        match unlinkat(&self.fd, name, AtFlags::empty()) {
            Ok(()) => {}
            Err(error) if error == rustix::io::Errno::NOENT => {}
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
        self.write(name, bytes)
    }

    fn subprocess_path(&self, name: &str) -> PathBuf {
        PathBuf::from(format!(
            "/proc/self/fd/{}/{}",
            self.subprocess_fd.as_raw_fd(),
            name
        ))
    }

    fn cleanup(&mut self, hooks: &mut impl TransactionHooks) -> Vec<FilesystemFailure> {
        if !self.active {
            return Vec::new();
        }
        if let Err(error) = hooks.before_stage_cleanup() {
            return vec![FilesystemFailure {
                operation: "remove staging directory",
                entry: self.name.clone(),
                error,
            }];
        }
        let failures = cleanup_staging(
            &self.output_fd,
            &self.fd,
            &self.name,
            self.device,
            self.inode,
        );
        if failures.is_empty() {
            self.active = false;
        }
        failures
    }
}

impl Drop for StagingDirectory {
    fn drop(&mut self) {
        if self.active {
            let failures = cleanup_staging(
                &self.output_fd,
                &self.fd,
                &self.name,
                self.device,
                self.inode,
            );
            if failures.is_empty() {
                self.active = false;
            }
        }
    }
}

fn cleanup_created_staging(
    output: &PinnedOutput,
    staging_fd: &OwnedFd,
    staging_name: &str,
    staging_device: u64,
    staging_inode: u64,
    hooks: &mut impl TransactionHooks,
) -> Vec<FilesystemFailure> {
    if let Err(error) = hooks.before_stage_cleanup() {
        return vec![FilesystemFailure {
            operation: "remove staging directory after setup failure",
            entry: staging_name.to_string(),
            error,
        }];
    }
    cleanup_staging(
        &output.fd,
        staging_fd,
        staging_name,
        staging_device,
        staging_inode,
    )
}

fn cleanup_staging(
    output_fd: &OwnedFd,
    staging_fd: &OwnedFd,
    staging_name: &str,
    staging_device: u64,
    staging_inode: u64,
) -> Vec<FilesystemFailure> {
    let mut failures = Vec::new();
    let mut directory = match Dir::read_from(staging_fd) {
        Ok(directory) => directory,
        Err(error) => {
            failures.push(FilesystemFailure {
                operation: "read staging directory",
                entry: staging_name.to_string(),
                error: error.into(),
            });
            return failures;
        }
    };
    for entry in &mut directory {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                failures.push(FilesystemFailure {
                    operation: "read staging entry",
                    entry: staging_name.to_string(),
                    error: error.into(),
                });
                continue;
            }
        };
        if entry.file_name().to_bytes() == b"." || entry.file_name().to_bytes() == b".." {
            continue;
        }
        let flags = if entry.file_type() == FileType::Directory {
            AtFlags::REMOVEDIR
        } else {
            AtFlags::empty()
        };
        if let Err(error) = unlinkat(staging_fd, entry.file_name(), flags) {
            failures.push(FilesystemFailure {
                operation: "remove staging entry",
                entry: entry.file_name().to_string_lossy().into_owned(),
                error: error.into(),
            });
        }
    }
    if failures.is_empty() {
        match statat(output_fd, staging_name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) if stat.st_dev == staging_device && stat.st_ino == staging_inode => {
                if let Err(error) = unlinkat(output_fd, staging_name, AtFlags::REMOVEDIR) {
                    failures.push(FilesystemFailure {
                        operation: "remove staging directory",
                        entry: staging_name.to_string(),
                        error: error.into(),
                    });
                }
            }
            Ok(_) => failures.push(FilesystemFailure {
                operation: "verify staging directory identity",
                entry: staging_name.to_string(),
                error: io::Error::other("staging directory name was substituted"),
            }),
            Err(error) => failures.push(FilesystemFailure {
                operation: "verify staging directory identity",
                entry: staging_name.to_string(),
                error: error.into(),
            }),
        }
    }
    failures
}

fn is_staging_name(name: &[u8]) -> bool {
    let Some(rest) = name.strip_prefix(STAGING_PREFIX.as_bytes()) else {
        return false;
    };
    let Some(separator) = rest.iter().position(|byte| *byte == b'-') else {
        return false;
    };
    let (pid, sequence_with_separator) = rest.split_at(separator);
    let sequence = &sequence_with_separator[1..];
    !pid.is_empty()
        && !sequence.is_empty()
        && pid.iter().all(u8::is_ascii_digit)
        && sequence.iter().all(u8::is_ascii_digit)
}

fn output_scan_fd(output: &PinnedOutput) -> Result<OwnedFd, EmitError> {
    openat(
        &output.fd,
        Path::new("."),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(std::io::Error::from)
    .map_err(EmitError::from)
}

fn cleanup_abandoned_staging(output: &PinnedOutput) -> Result<(), EmitError> {
    let recovery_removed = match statat(
        &output.fd,
        RECOVERY_OWNERSHIP_FILE,
        AtFlags::SYMLINK_NOFOLLOW,
    ) {
        Ok(stat)
            if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile
                && stat.st_nlink == 1
                && stat.st_mode & 0o077 == 0 =>
        {
            unlinkat(&output.fd, RECOVERY_OWNERSHIP_FILE, AtFlags::empty())
                .map_err(std::io::Error::from)?;
            true
        }
        Ok(_) => {
            return Err(EmitError::InvalidArtifactDestination {
                path: output.display_path.join(RECOVERY_OWNERSHIP_FILE),
                reason: "abandoned ownership recovery entry is not a private single-link file"
                    .to_string(),
            });
        }
        Err(error) if error == rustix::io::Errno::NOENT => false,
        Err(error) => return Err(std::io::Error::from(error).into()),
    };
    let scan_fd = output_scan_fd(output)?;
    let mut directory = Dir::read_from(&scan_fd).map_err(std::io::Error::from)?;
    let mut names = Vec::new();
    let mut entries = 0usize;
    for entry in &mut directory {
        let entry = entry.map_err(std::io::Error::from)?;
        let name = entry.file_name().to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        entries = entries
            .checked_add(1)
            .ok_or_else(|| ownership_error("managed artifact directory entry count overflow"))?;
        if entries > MAX_OUTPUT_ENTRIES {
            return Err(ownership_error(
                "managed artifact directory exceeds its entry bound",
            ));
        }
        if is_staging_name(name) {
            names.push(String::from_utf8(name.to_vec()).expect("staging names are ASCII"));
        }
    }

    let mut removed_any = recovery_removed;
    for name in names {
        let stat = match statat(&output.fd, &name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat) => stat,
            Err(error) if error == rustix::io::Errno::NOENT => continue,
            Err(error) => return Err(std::io::Error::from(error).into()),
        };
        if FileType::from_raw_mode(stat.st_mode) == FileType::Directory {
            if stat.st_mode & 0o777 != 0o700 {
                return Err(EmitError::InvalidArtifactDestination {
                    path: output.display_path.join(&name),
                    reason: "abandoned staging directory is not private 0700".to_string(),
                });
            }
            let fd = openat(
                &output.fd,
                &name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(std::io::Error::from)?;
            let opened = fstat(&fd).map_err(std::io::Error::from)?;
            if opened.st_dev != stat.st_dev || opened.st_ino != stat.st_ino {
                return Err(EmitError::InvalidArtifactDestination {
                    path: output.display_path.join(&name),
                    reason: "abandoned staging directory changed while opening".to_string(),
                });
            }
            let failures = cleanup_staging(&output.fd, &fd, &name, opened.st_dev, opened.st_ino);
            if !failures.is_empty() {
                return Err(EmitError::InvalidArtifactDestination {
                    path: output.display_path.join(&name),
                    reason: failures
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("; "),
                });
            }
        } else {
            unlinkat(&output.fd, &name, AtFlags::empty()).map_err(std::io::Error::from)?;
        }
        removed_any = true;
    }
    if removed_any {
        fsync(&output.fd).map_err(std::io::Error::from)?;
    }
    Ok(())
}

fn canonical_artifact_kernel(name: &[u8]) -> Option<String> {
    let name = std::str::from_utf8(name).ok()?;
    let kernel = [".ll", ".o", ".hsaco"]
        .into_iter()
        .find_map(|extension| name.strip_suffix(extension))?;
    validate_artifact_name(kernel).ok()?;
    Some(kernel.to_string())
}

fn inventory_unowned_artifacts(
    output: &PinnedOutput,
    registry: &OwnershipRegistry,
) -> Result<BTreeSet<String>, EmitError> {
    let scan_fd = output_scan_fd(output)?;
    let mut directory = Dir::read_from(&scan_fd).map_err(std::io::Error::from)?;
    let mut kernels = BTreeSet::new();
    let mut entries = 0usize;
    for entry in &mut directory {
        let entry = entry.map_err(std::io::Error::from)?;
        let name = entry.file_name().to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        entries = entries
            .checked_add(1)
            .ok_or_else(|| ownership_error("managed artifact directory entry count overflow"))?;
        if entries > MAX_OUTPUT_ENTRIES {
            return Err(ownership_error(
                "managed artifact directory exceeds its entry bound",
            ));
        }
        let Some(kernel) = canonical_artifact_kernel(name) else {
            continue;
        };
        if registry.owner_of(&kernel).is_none() {
            kernels.insert(kernel);
            if kernels.len() > MAX_TOTAL_OWNED_KERNELS {
                return Err(ownership_error(
                    "managed artifact directory has too many unowned kernels",
                ));
            }
        }
    }
    Ok(kernels)
}

#[derive(Clone, Debug)]
struct ArtifactNames {
    kernel: String,
    llvm_ir: String,
    object: String,
    hsaco: String,
}

impl ArtifactNames {
    fn new(kernel: &str) -> Result<Self, EmitError> {
        validate_artifact_name(kernel)?;
        Ok(Self {
            kernel: kernel.to_string(),
            llvm_ir: format!("{kernel}.ll"),
            object: format!("{kernel}.o"),
            hsaco: format!("{kernel}.hsaco"),
        })
    }

    fn files(&self) -> [&str; 3] {
        [&self.llvm_ir, &self.object, &self.hsaco]
    }
}

#[derive(Debug)]
struct PreparedArtifact {
    names: ArtifactNames,
    llvm_ir: String,
}

pub(crate) fn emit_artifact_transaction<T>(
    output_dir: &Path,
    producer: &ProducerIdentity,
    kernels: &[T],
    kernel_name: impl Fn(&T) -> &str,
    prepare: impl FnMut(&T) -> Result<String, EmitError>,
    compile: impl FnMut(&Path, &Path) -> Result<(), EmitError>,
) -> Result<Vec<DeviceArtifact>, EmitError> {
    emit_artifact_transaction_with_hooks(
        output_dir,
        producer,
        kernels,
        kernel_name,
        prepare,
        compile,
        &mut NoFaults,
    )
}

pub(crate) fn emit_artifact_transaction_after_preflight<T, P>(
    output_dir: &Path,
    producer: &ProducerIdentity,
    preflight: impl FnOnce() -> Result<P, EmitError>,
    kernel_name: impl Fn(&T) -> &str,
    prepare: impl FnMut(&T) -> Result<String, EmitError>,
    compile: impl FnMut(&Path, &Path) -> Result<(), EmitError>,
) -> Result<Vec<DeviceArtifact>, EmitError>
where
    P: AsRef<[T]>,
{
    emit_artifact_transaction_after_preflight_with_hooks(
        output_dir,
        producer,
        preflight,
        kernel_name,
        prepare,
        compile,
        &mut NoFaults,
    )
}

fn emit_artifact_transaction_with_hooks<T>(
    output_dir: &Path,
    producer: &ProducerIdentity,
    kernels: &[T],
    kernel_name: impl Fn(&T) -> &str,
    prepare: impl FnMut(&T) -> Result<String, EmitError>,
    compile: impl FnMut(&Path, &Path) -> Result<(), EmitError>,
    hooks: &mut impl TransactionHooks,
) -> Result<Vec<DeviceArtifact>, EmitError> {
    emit_artifact_transaction_after_preflight_with_hooks(
        output_dir,
        producer,
        || Ok(kernels),
        kernel_name,
        prepare,
        compile,
        hooks,
    )
}

fn emit_artifact_transaction_after_preflight_with_hooks<T, P>(
    output_dir: &Path,
    producer: &ProducerIdentity,
    preflight: impl FnOnce() -> Result<P, EmitError>,
    kernel_name: impl Fn(&T) -> &str,
    mut prepare: impl FnMut(&T) -> Result<String, EmitError>,
    mut compile: impl FnMut(&Path, &Path) -> Result<(), EmitError>,
    hooks: &mut impl TransactionHooks,
) -> Result<Vec<DeviceArtifact>, EmitError>
where
    P: AsRef<[T]>,
{
    let output = PinnedOutput::open(output_dir)?;
    let _lock = output.lock()?;
    output.verify_path_identity()?;
    cleanup_abandoned_staging(&output)?;
    let mut original_registry = read_registry(&output)?;
    if prune_absent_ownership(&output, &mut original_registry)? {
        commit_registry_direct(&output, &original_registry)?;
    }
    let orphaned = inventory_unowned_artifacts(&output, &original_registry)?;
    let old_owned = original_registry.owned_by(producer);

    let preflight = match preflight() {
        Ok(preflight) => preflight,
        Err(error) => {
            let invalidation_set = old_owned.union(&orphaned).cloned().collect::<BTreeSet<_>>();
            return Err(abort_without_staging(
                &output,
                &original_registry,
                producer,
                &invalidation_set,
                &old_owned,
                error,
                hooks,
            ));
        }
    };
    let kernels = preflight.as_ref();
    if kernels.len() > MAX_KERNELS_PER_PRODUCER {
        let invalidation_set = old_owned.union(&orphaned).cloned().collect::<BTreeSet<_>>();
        return Err(abort_without_staging(
            &output,
            &original_registry,
            producer,
            &invalidation_set,
            &old_owned,
            ownership_error("too many kernels in one compiler transaction"),
            hooks,
        ));
    }

    let mut names = Vec::with_capacity(kernels.len());
    let mut used_names = HashSet::with_capacity(kernels.len());
    let mut primary = None;
    for kernel in kernels {
        let name = kernel_name(kernel);
        match ArtifactNames::new(name) {
            Ok(artifact_names) => {
                if !used_names.insert(name.to_ascii_lowercase()) && primary.is_none() {
                    primary = Some(EmitError::DuplicateArtifactName {
                        kernel: name.to_string(),
                    });
                }
                names.push(artifact_names);
            }
            Err(error) if primary.is_none() => primary = Some(error),
            Err(_) => {}
        }
    }

    let mut protected_names = BTreeSet::new();
    for artifact in &names {
        match original_registry.owner_of(&artifact.kernel) {
            Some(owner) if owner != producer.stable_source => {
                protected_names.insert(artifact.kernel.clone());
                if primary.is_none() {
                    primary = Some(EmitError::ArtifactOwnedByOtherProducer {
                        kernel: artifact.kernel.clone(),
                    });
                }
            }
            Some(_) => {
                if let Err(error) = validate_owned_destinations(&output, artifact)
                    && primary.is_none()
                {
                    primary = Some(error);
                }
            }
            // The output directory is a generated-artifact namespace. Files from the
            // pre-registry emitter are adopted on success and invalidated on failure.
            None => {}
        }
    }

    let new_owned = names
        .iter()
        .map(|artifact| artifact.kernel.clone())
        .collect::<BTreeSet<_>>();
    let recovery_candidates = old_owned
        .union(&new_owned)
        .filter(|kernel| !protected_names.contains(*kernel))
        .cloned()
        .collect::<BTreeSet<_>>();
    let invalidation_set = recovery_candidates
        .iter()
        .chain(orphaned.iter())
        .cloned()
        .collect::<BTreeSet<_>>();

    if old_owned.is_empty() && names.is_empty() && orphaned.is_empty() && primary.is_none() {
        return Ok(Vec::new());
    }

    let rollback = RollbackContext {
        output: &output,
        original_registry: &original_registry,
        producer,
        invalidation_set: &invalidation_set,
        recovery_candidates: &recovery_candidates,
    };

    let mut staging = match StagingDirectory::create(&output, hooks) {
        Ok(staging) => staging,
        Err(staging_error) => {
            let StagingCreateError {
                primary: staging_primary,
                mut cleanup_failures,
            } = staging_error;
            let abort_primary = match primary {
                Some(primary) => {
                    cleanup_failures.push(FilesystemFailure {
                        operation: "create staging directory",
                        entry: output.display_path.display().to_string(),
                        error: io::Error::other(staging_primary.to_string()),
                    });
                    primary
                }
                None => staging_primary,
            };
            let mut error = abort_without_staging(
                &output,
                &original_registry,
                producer,
                &invalidation_set,
                &recovery_candidates,
                abort_primary,
                hooks,
            );
            if let EmitError::Transaction(transaction) = &mut error {
                transaction.cleanup_failures.splice(0..0, cleanup_failures);
                transaction.publication = PublicationState::NotStarted {
                    total_final_renames: names.len() * 3,
                };
            }
            return Err(error);
        }
    };

    if let Some(error) = primary {
        return Err(rollback.abort(
            &mut staging,
            PublicationState::NotStarted {
                total_final_renames: names.len() * 3,
            },
            error,
            hooks,
        ));
    }

    let mut prepared = Vec::with_capacity(kernels.len());
    for (kernel, artifact_names) in kernels.iter().zip(names) {
        match prepare(kernel) {
            Ok(llvm_ir) => prepared.push(PreparedArtifact {
                names: artifact_names,
                llvm_ir,
            }),
            Err(error) => {
                return Err(rollback.abort(
                    &mut staging,
                    PublicationState::NotStarted {
                        total_final_renames: kernels.len() * 3,
                    },
                    error,
                    hooks,
                ));
            }
        }
    }

    for artifact in &prepared {
        let result = (|| {
            staging.write(&artifact.names.llvm_ir, artifact.llvm_ir.as_bytes())?;
            let llvm_ir_path = staging.subprocess_path(&artifact.names.llvm_ir);
            let hsaco_path = staging.subprocess_path(&artifact.names.hsaco);
            compile(&llvm_ir_path, &hsaco_path)?;
            validate_staged_artifacts(&staging, &artifact.names, &output)
        })();
        if let Err(error) = result {
            return Err(rollback.abort(
                &mut staging,
                PublicationState::NotStarted {
                    total_final_renames: prepared.len() * 3,
                },
                error,
                hooks,
            ));
        }
    }

    let mut next_registry = original_registry.clone();
    next_registry.set_owned(producer, new_owned.clone());
    if let Err(error) = stage_registry(&staging, &next_registry) {
        return Err(rollback.abort(
            &mut staging,
            PublicationState::NotStarted {
                total_final_renames: prepared.len() * 3,
            },
            error,
            hooks,
        ));
    }
    if let Err(error) = fsync(&staging.fd).map_err(std::io::Error::from) {
        return Err(rollback.abort(
            &mut staging,
            PublicationState::NotStarted {
                total_final_renames: prepared.len() * 3,
            },
            error.into(),
            hooks,
        ));
    }

    if let Err(error) = output.verify_path_identity() {
        return Err(rollback.abort(
            &mut staging,
            PublicationState::NotStarted {
                total_final_renames: prepared.len() * 3,
            },
            error,
            hooks,
        ));
    }

    let total_final_renames = prepared.len() * 3;
    let mut completed_final_renames = 0usize;
    for artifact in &prepared {
        for entry in artifact.names.files() {
            if let Err(error) = hooks
                .before_rename(RenameKind::Artifact, completed_final_renames)
                .and_then(|()| {
                    renameat(&staging.fd, entry, &output.fd, entry).map_err(std::io::Error::from)
                })
            {
                return Err(rollback.abort(
                    &mut staging,
                    PublicationState::Partial {
                        completed_final_renames,
                        total_final_renames,
                    },
                    error.into(),
                    hooks,
                ));
            }
            completed_final_renames += 1;
        }
    }

    let stale = old_owned
        .union(&orphaned)
        .filter(|kernel| !new_owned.contains(*kernel))
        .cloned()
        .collect::<BTreeSet<_>>();
    let (_, stale_failures) = invalidate_kernels(&output, &stale, hooks);
    if !stale_failures.is_empty() {
        let mut error = rollback.abort(
            &mut staging,
            PublicationState::FinalsPublished {
                final_renames: completed_final_renames,
            },
            ownership_error("failed to remove stale producer artifacts"),
            hooks,
        );
        if let EmitError::Transaction(transaction) = &mut error {
            transaction
                .invalidation_failures
                .splice(0..0, stale_failures);
        }
        return Err(error);
    }

    if let Err(error) = commit_registry(
        &output,
        &staging,
        &next_registry,
        completed_final_renames,
        hooks,
    ) {
        return Err(rollback.abort(
            &mut staging,
            PublicationState::FinalsPublished {
                final_renames: completed_final_renames,
            },
            error,
            hooks,
        ));
    }

    let mut primary = fsync(&output.fd)
        .map_err(std::io::Error::from)
        .err()
        .map(EmitError::from);
    if let Err(error) = hooks.after_registry_commit()
        && primary.is_none()
    {
        primary = Some(error.into());
    }
    if let Err(error) = output.verify_path_identity()
        && primary.is_none()
    {
        primary = Some(error);
    }

    let mut cleanup_failures = staging.cleanup(hooks);
    if let Err(error) = fsync(&output.fd).map_err(std::io::Error::from) {
        cleanup_failures.push(FilesystemFailure {
            operation: "persist staging cleanup",
            entry: output.display_path.display().to_string(),
            error,
        });
    }
    if primary.is_some() || !cleanup_failures.is_empty() {
        let publication = if cleanup_failures.is_empty() {
            PublicationState::Committed {
                final_renames: completed_final_renames,
            }
        } else {
            PublicationState::CommittedWithCleanupFailure {
                final_renames: completed_final_renames,
            }
        };
        return Err(EmitError::Transaction(Box::new(ArtifactTransactionError {
            primary: primary.map(Box::new),
            cleanup_failures,
            invalidation_failures: Vec::new(),
            publication,
        })));
    }
    Ok(prepared
        .iter()
        .map(|artifact| DeviceArtifact {
            kernel_name: artifact.names.kernel.clone(),
            llvm_ir_path: output.display_path.join(&artifact.names.llvm_ir),
            hsaco_path: output.display_path.join(&artifact.names.hsaco),
        })
        .collect())
}

fn read_registry(output: &PinnedOutput) -> Result<OwnershipRegistry, EmitError> {
    let fd = match openat(
        &output.fd,
        OWNERSHIP_FILE,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(error) if error == rustix::io::Errno::NOENT => {
            return Ok(OwnershipRegistry::default());
        }
        Err(error) => return Err(std::io::Error::from(error).into()),
    };
    let stat = fstat(&fd).map_err(std::io::Error::from)?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return Err(EmitError::InvalidArtifactDestination {
            path: output.display_path.join(OWNERSHIP_FILE),
            reason: "ownership registry is not a regular file".to_string(),
        });
    }
    let mut bytes = Vec::new();
    fs::File::from(fd)
        .take((MAX_OWNERSHIP_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    OwnershipRegistry::decode(&bytes)
}

fn prune_absent_ownership(
    output: &PinnedOutput,
    registry: &mut OwnershipRegistry,
) -> Result<bool, EmitError> {
    let mut absent = Vec::new();
    for (source, ownership) in &registry.producers {
        for kernel in &ownership.kernels {
            let artifact = ArtifactNames::new(kernel)?;
            let mut any_present = false;
            for entry in artifact.files() {
                match statat(&output.fd, entry, AtFlags::SYMLINK_NOFOLLOW) {
                    Ok(_) => any_present = true,
                    Err(error) if error == rustix::io::Errno::NOENT => {}
                    Err(error) => return Err(std::io::Error::from(error).into()),
                }
            }
            if !any_present {
                absent.push((source.clone(), kernel.clone()));
            }
        }
    }
    if absent.is_empty() {
        return Ok(false);
    }
    for (source, kernel) in absent {
        if let Some(ownership) = registry.producers.get_mut(&source) {
            ownership.kernels.remove(&kernel);
        }
    }
    registry
        .producers
        .retain(|_, ownership| !ownership.kernels.is_empty());
    Ok(true)
}

fn commit_registry_direct(
    output: &PinnedOutput,
    registry: &OwnershipRegistry,
) -> Result<(), EmitError> {
    if registry.producers.is_empty() {
        match unlinkat(&output.fd, OWNERSHIP_FILE, AtFlags::empty()) {
            Ok(()) => {}
            Err(error) if error == rustix::io::Errno::NOENT => {}
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
        fsync(&output.fd).map_err(std::io::Error::from)?;
        return Ok(());
    }

    let bytes = registry.encode()?;
    let fd = openat(
        &output.fd,
        RECOVERY_OWNERSHIP_FILE,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )
    .map_err(std::io::Error::from)?;
    let result = (|| {
        let mut file = fs::File::from(fd);
        file.write_all(&bytes)?;
        file.sync_all()?;
        output.verify_path_identity()?;
        renameat(
            &output.fd,
            RECOVERY_OWNERSHIP_FILE,
            &output.fd,
            OWNERSHIP_FILE,
        )
        .map_err(std::io::Error::from)?;
        fsync(&output.fd).map_err(std::io::Error::from)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = unlinkat(&output.fd, RECOVERY_OWNERSHIP_FILE, AtFlags::empty());
        let _ = fsync(&output.fd);
    }
    result
}

fn stage_registry(
    staging: &StagingDirectory,
    registry: &OwnershipRegistry,
) -> Result<(), EmitError> {
    if registry.producers.is_empty() {
        match unlinkat(&staging.fd, STAGED_OWNERSHIP_FILE, AtFlags::empty()) {
            Ok(()) => {}
            Err(error) if error == rustix::io::Errno::NOENT => {}
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
        Ok(())
    } else {
        staging.replace(STAGED_OWNERSHIP_FILE, &registry.encode()?)
    }
}

fn commit_registry(
    output: &PinnedOutput,
    staging: &StagingDirectory,
    registry: &OwnershipRegistry,
    completed_final_renames: usize,
    hooks: &mut impl TransactionHooks,
) -> Result<(), EmitError> {
    if registry.producers.is_empty() {
        match unlinkat(&output.fd, OWNERSHIP_FILE, AtFlags::empty()) {
            Ok(()) => Ok(()),
            Err(error) if error == rustix::io::Errno::NOENT => Ok(()),
            Err(error) => Err(std::io::Error::from(error).into()),
        }
    } else {
        hooks
            .before_rename(RenameKind::Ownership, completed_final_renames)
            .map_err(EmitError::from)?;
        renameat(
            &staging.fd,
            STAGED_OWNERSHIP_FILE,
            &output.fd,
            OWNERSHIP_FILE,
        )
        .map_err(std::io::Error::from)?;
        Ok(())
    }
}

struct RollbackContext<'a> {
    output: &'a PinnedOutput,
    original_registry: &'a OwnershipRegistry,
    producer: &'a ProducerIdentity,
    invalidation_set: &'a BTreeSet<String>,
    recovery_candidates: &'a BTreeSet<String>,
}

fn abort_without_staging(
    output: &PinnedOutput,
    original_registry: &OwnershipRegistry,
    producer: &ProducerIdentity,
    invalidation_set: &BTreeSet<String>,
    recovery_candidates: &BTreeSet<String>,
    primary: EmitError,
    hooks: &mut impl TransactionHooks,
) -> EmitError {
    let (failed_kernels, invalidation_failures) =
        invalidate_kernels(output, invalidation_set, hooks);
    let failed_owned = failed_kernels
        .intersection(recovery_candidates)
        .cloned()
        .collect();
    let mut recovery_registry = original_registry.clone();
    recovery_registry.set_owned(producer, failed_owned);
    let mut cleanup_failures = Vec::new();
    if let Err(error) = commit_registry_direct(output, &recovery_registry) {
        cleanup_failures.push(FilesystemFailure {
            operation: "reconcile ownership without staging",
            entry: OWNERSHIP_FILE.to_string(),
            error: io::Error::other(error.to_string()),
        });
    }
    if let Err(error) = fsync(&output.fd).map_err(std::io::Error::from) {
        cleanup_failures.push(FilesystemFailure {
            operation: "persist rollback without staging",
            entry: output.display_path.display().to_string(),
            error,
        });
    }
    EmitError::Transaction(Box::new(ArtifactTransactionError {
        primary: Some(Box::new(primary)),
        cleanup_failures,
        invalidation_failures,
        publication: PublicationState::NotStarted {
            total_final_renames: 0,
        },
    }))
}

impl RollbackContext<'_> {
    fn abort(
        &self,
        staging: &mut StagingDirectory,
        publication: PublicationState,
        primary: EmitError,
        hooks: &mut impl TransactionHooks,
    ) -> EmitError {
        let (failed_kernels, invalidation_failures) =
            invalidate_kernels(self.output, self.invalidation_set, hooks);
        let failed_owned = failed_kernels
            .intersection(self.recovery_candidates)
            .cloned()
            .collect();
        let mut recovery_registry = self.original_registry.clone();
        recovery_registry.set_owned(self.producer, failed_owned);
        let mut cleanup_failures = Vec::new();
        if let Err(error) = stage_registry(staging, &recovery_registry)
            .and_then(|()| commit_registry(self.output, staging, &recovery_registry, 0, hooks))
        {
            cleanup_failures.push(FilesystemFailure {
                operation: "reconcile ownership after failure",
                entry: OWNERSHIP_FILE.to_string(),
                error: io::Error::other(error.to_string()),
            });
        }
        cleanup_failures.extend(staging.cleanup(hooks));
        if let Err(error) = fsync(&self.output.fd).map_err(std::io::Error::from) {
            cleanup_failures.push(FilesystemFailure {
                operation: "persist transaction rollback",
                entry: self.output.display_path.display().to_string(),
                error,
            });
        }
        EmitError::Transaction(Box::new(ArtifactTransactionError {
            primary: Some(Box::new(primary)),
            cleanup_failures,
            invalidation_failures,
            publication,
        }))
    }
}

fn invalidate_kernels(
    output: &PinnedOutput,
    kernels: &BTreeSet<String>,
    hooks: &mut impl TransactionHooks,
) -> (BTreeSet<String>, Vec<FilesystemFailure>) {
    let mut failed_kernels = BTreeSet::new();
    let mut failures = Vec::new();
    for kernel in kernels {
        let artifact = match ArtifactNames::new(kernel) {
            Ok(artifact) => artifact,
            Err(error) => {
                failed_kernels.insert(kernel.clone());
                failures.push(FilesystemFailure {
                    operation: "validate owned artifact name",
                    entry: kernel.clone(),
                    error: io::Error::other(error.to_string()),
                });
                continue;
            }
        };
        for entry in artifact.files() {
            let result = hooks.before_invalidate(entry).and_then(|()| {
                unlinkat(&output.fd, entry, AtFlags::empty()).map_err(std::io::Error::from)
            });
            match result {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    failed_kernels.insert(kernel.clone());
                    failures.push(FilesystemFailure {
                        operation: "invalidate artifact",
                        entry: entry.to_string(),
                        error,
                    });
                }
            }
        }
    }
    (failed_kernels, failures)
}

fn validate_owned_destinations(
    output: &PinnedOutput,
    artifact: &ArtifactNames,
) -> Result<(), EmitError> {
    for entry in artifact.files() {
        match statat(&output.fd, entry, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat)
                if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile
                    || FileType::from_raw_mode(stat.st_mode) == FileType::Symlink => {}
            Ok(_) => {
                return Err(EmitError::InvalidArtifactDestination {
                    path: output.display_path.join(entry),
                    reason: "owned destination is not a regular file or symlink".to_string(),
                });
            }
            Err(error) if error == rustix::io::Errno::NOENT => {}
            Err(error) => return Err(std::io::Error::from(error).into()),
        }
    }
    Ok(())
}

fn validate_staged_artifacts(
    staging: &StagingDirectory,
    artifact: &ArtifactNames,
    output: &PinnedOutput,
) -> Result<(), EmitError> {
    for entry in artifact.files() {
        let fd = match openat(
            &staging.fd,
            entry,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => fd,
            Err(_) => {
                return Err(EmitError::MissingStagedArtifact {
                    path: output.display_path.join(&staging.name).join(entry),
                });
            }
        };
        let stat = fstat(&fd).map_err(std::io::Error::from)?;
        if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
            return Err(EmitError::MissingStagedArtifact {
                path: output.display_path.join(&staging.name).join(entry),
            });
        }
        fsync(&fd).map_err(std::io::Error::from)?;
    }
    Ok(())
}

fn validate_simple_name(name: &str, label: &str) -> Result<(), EmitError> {
    if name.is_empty()
        || name.len() > MAX_ARTIFACT_NAME_BYTES
        || !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(EmitError::InvalidProducer {
            reason: format!(
                "{label} must contain 1 to {MAX_ARTIFACT_NAME_BYTES} ASCII letters, digits, or underscores"
            ),
        });
    }
    Ok(())
}

fn validate_artifact_name(kernel_name: &str) -> Result<(), EmitError> {
    let display_name = if kernel_name.len() <= MAX_ARTIFACT_NAME_BYTES {
        kernel_name.to_string()
    } else {
        format!("<{}-byte name>", kernel_name.len())
    };
    if kernel_name.is_empty() {
        return Err(EmitError::InvalidArtifactName {
            kernel: display_name,
            reason: "name is empty".to_string(),
        });
    }
    if kernel_name.len() > MAX_ARTIFACT_NAME_BYTES {
        return Err(EmitError::InvalidArtifactName {
            kernel: display_name,
            reason: format!("name exceeds {MAX_ARTIFACT_NAME_BYTES} bytes"),
        });
    }
    let mut bytes = kernel_name.bytes();
    if !bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        || !bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(EmitError::InvalidArtifactName {
            kernel: display_name,
            reason: "name must be an ASCII identifier".to_string(),
        });
    }
    Ok(())
}
