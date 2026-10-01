//! Optional observation replay after an unchanged genuine source export fails.

#[cfg(unix)]
pub(super) use supported::run;

#[cfg(not(unix))]
pub(super) fn run(command: &mut std::process::Command, _names: &[&str]) -> std::process::Output {
    command
        .output()
        .expect("run genuine pinned core source extraction")
}

#[cfg(unix)]
mod supported {
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};
    use std::ffi::OsStr;
    use std::fs::{self, OpenOptions};
    use std::io::{Read, Write};
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt};
    use std::path::{Path, PathBuf};
    use std::process::{Command, Output, Stdio};

    const DIRECTORY: &str = "FE2O3_TEST_SOURCE_FAILURE_MIR_DIRECTORY_V1";
    const FLAGS: &str = "CARGO_TARGET_AMDGCN_AMD_AMDHSA_RUSTFLAGS";
    const MAX_FILES: usize = 256;
    const MAX_FILE_BYTES: usize = 64 * 1024;
    const MAX_DUMP_BYTES: usize = 8 * 1024 * 1024;
    const MAX_INLINE_BYTES: usize = 256 * 1024;
    const MAX_OUTPUT_BYTES: usize = 2 * 1024 * 1024;

    fn error(error: impl std::fmt::Display) -> String {
        error.to_string()
    }
    fn text(value: &OsStr) -> Result<String, String> {
        value
            .to_str()
            .map(str::to_owned)
            .ok_or_else(|| "non-UTF8 command argument".into())
    }
    fn digest(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn read_regular(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
        let before = fs::symlink_metadata(path).map_err(error)?;
        if !before.is_file() || before.len() > limit as u64 {
            return Err("regular-file byte bound".into());
        }
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .open(path)
            .map_err(error)?;
        let opened = file.metadata().map_err(error)?;
        if (before.dev(), before.ino()) != (opened.dev(), opened.ino()) || !opened.is_file() {
            return Err("file identity changed".into());
        }
        let mut bytes = Vec::new();
        Read::by_ref(&mut file)
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(error)?;
        let after = file.metadata().map_err(error)?;
        let current = fs::symlink_metadata(path).map_err(error)?;
        let identity = |m: &fs::Metadata| (m.dev(), m.ino(), m.len(), m.mtime(), m.mtime_nsec());
        if bytes.len() > limit
            || bytes.len() as u64 != opened.len()
            || identity(&opened) != identity(&after)
            || identity(&after) != identity(&current)
        {
            return Err("file changed during bounded read".into());
        }
        Ok(bytes)
    }

    fn fresh_json(path: &Path, value: &Value) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(value).map_err(error)?;
        fresh_bytes(path, &bytes, 1024 * 1024)
    }

    fn fresh_bytes(path: &Path, bytes: &[u8], limit: usize) -> Result<(), String> {
        if bytes.len() > limit {
            return Err("diagnostic file byte bound".into());
        }
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(error)?
            .write_all(bytes)
            .map_err(error)
    }

    fn observation_filter(names: &[&str]) -> Result<String, String> {
        if names.is_empty()
            || names.len() > 8
            || names.iter().any(|name| {
                name.is_empty()
                    || name.len() > 80
                    || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            })
        {
            return Err("invalid bounded diagnostic item filter".into());
        }
        Ok(names
            .iter()
            .map(|name| format!("{name}&runtime-optimized"))
            .collect::<Vec<_>>()
            .join("|"))
    }

    fn configure_replay(command: &mut Command, flags: &str, bundle: &Path) {
        // Reuse the command: get_envs cannot expose or reconstruct env_clear.
        command
            .env(FLAGS, flags)
            .env("FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V1", bundle);
    }

    fn source_identity(fixture: &Path) -> Result<Value, String> {
        let mut rows = Vec::new();
        for name in ["Cargo.toml", "Cargo.lock", "src/lib.rs"] {
            let bytes = read_regular(&fixture.join(name), MAX_OUTPUT_BYTES)?;
            rows.push(json!({"path": name, "bytes": bytes.len(), "sha256": digest(&bytes)}));
        }
        Ok(json!(rows))
    }

    fn inventory(directory: &Path) -> Result<Value, String> {
        let mut paths = Vec::new();
        for entry in fs::read_dir(directory).map_err(error)? {
            if paths.len() == MAX_FILES {
                return Err("dump file-count bound".into());
            }
            paths.push(entry.map_err(error)?.path());
        }
        paths.sort();
        let mut total = 0usize;
        let mut inline = 0usize;
        let mut rows = Vec::new();
        for path in paths {
            let name = text(path.file_name().ok_or("missing dump filename")?)?;
            if name.len() > 512 || !name.ends_with(".runtime-optimized.after.mir") {
                return Err("unexpected dump artifact".into());
            }
            let bytes = read_regular(&path, MAX_FILE_BYTES)?;
            total = total.checked_add(bytes.len()).ok_or("dump byte overflow")?;
            if total > MAX_DUMP_BYTES {
                return Err("aggregate dump byte bound".into());
            }
            let body = std::str::from_utf8(&bytes).map_err(error)?;
            let retained = if inline + bytes.len() <= MAX_INLINE_BYTES {
                inline += bytes.len();
                Some(body)
            } else {
                None
            };
            rows.push(json!({"file": name, "bytes": bytes.len(), "sha256": digest(&bytes),
            "runtimeOptimizedMir": retained, "textStatus": if retained.is_some() { "retained" } else { "omitted_inline_bound_raw_file_retained" }}));
        }
        if rows.is_empty() {
            return Err("no actual runtime-optimized MIR dumps".into());
        }
        Ok(json!({"files": rows, "bytes": total, "inlineBytes": inline}))
    }

    fn replay(
        original: &mut Command,
        original_output: &Output,
        names: &[&str],
        directory: PathBuf,
    ) -> Result<Value, String> {
        let parent = directory
            .parent()
            .ok_or("missing diagnostic parent")?
            .canonicalize()
            .map_err(error)?;
        let scratch = std::env::temp_dir().canonicalize().map_err(error)?;
        if !directory.is_absolute()
            || parent != scratch
            || directory.exists()
            || directory.is_symlink()
        {
            return Err(
                "diagnostic directory must be a fresh direct accounted-TMPDIR child".into(),
            );
        }
        let directory = parent.join(directory.file_name().ok_or("missing directory name")?);
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .map_err(error)?;
        let dumps = directory.join("mir");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&dumps)
            .map_err(error)?;
        let fixture = original
            .get_current_dir()
            .ok_or("missing original cwd")?
            .to_owned();
        let before = source_identity(&fixture)?;
        let original_flags = original
            .get_envs()
            .find(|(key, _)| *key == OsStr::new(FLAGS))
            .and_then(|(_, value)| value)
            .ok_or("missing explicit original target flags")?;
        let original_flags = text(original_flags)?;
        let filter = observation_filter(names)?;
        let dump_flags = format!(
            "-Zdump-mir={filter} -Zdump-mir-dir={} -Zdump-mir-exclude-pass-number",
            dumps.display()
        );
        if dumps
            .to_str()
            .is_none_or(|path| path.chars().any(char::is_whitespace))
        {
            return Err("unrepresentable dump path".into());
        }
        let replay_flags = format!("{original_flags} {dump_flags}");
        let program = text(original.get_program())?;
        let args: Vec<_> = original.get_args().map(text).collect::<Result<_, _>>()?;
        // The existing target is already accounted scratch. Dump-only flags change
        // its Cargo fingerprint; do not alter target paths or functional settings.
        let replay_bundle = directory.join("observation.fe2sim");
        fresh_bytes(
            &directory.join("original.stdout"),
            &original_output.stdout,
            MAX_OUTPUT_BYTES,
        )?;
        fresh_bytes(
            &directory.join("original.stderr"),
            &original_output.stderr,
            MAX_OUTPUT_BYTES,
        )?;
        let stdout = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(directory.join("replay.stdout"))
            .map_err(error)?;
        let stderr = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(directory.join("replay.stderr"))
            .map_err(error)?;
        configure_replay(original, &replay_flags, &replay_bundle);
        original
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        let invocation = json!({"schema": "fe2o3-source-failure-mir-observation-v1", "grantsAuthority": false,
        "semanticFunctionIdAssociation": "unavailable_not_inferred_from_dump_names",
        "program": program, "args": args, "cwd": text(fixture.as_os_str())?,
        "originalTargetFlags": original_flags, "replayTargetFlags": replay_flags,
        "onlyFlagAddition": dump_flags, "observationOutputPathOverride": replay_bundle,
        "originalStatus": original_output.status.code(),
        "originalStdoutSha256": digest(&original_output.stdout), "originalStderrSha256": digest(&original_output.stderr),
        "originalStdoutFile": "original.stdout", "originalStderrFile": "original.stderr",
        "fixture": before, "storagePolicy": "all_dump_and_Cargo_outputs_under_existing_accounted_scratch_and_outer_V5_cap"});
        fresh_json(&directory.join("invocation.json"), &invocation)?;
        // output() preserves the first invocation's implicit null stdin; both
        // output streams now go to the retained files rather than memory.
        let status = original.output().map_err(error)?.status;
        let after = source_identity(&fixture)?;
        if before != after {
            return Err("fixture changed during observation replay".into());
        }
        let stdout = read_regular(&directory.join("replay.stdout"), MAX_OUTPUT_BYTES)?;
        let stderr = read_regular(&directory.join("replay.stderr"), MAX_OUTPUT_BYTES)?;
        let dumps = inventory(&dumps).map_or_else(
            |detail| json!({"status": "unavailable", "detail": detail}),
            |rows| rows,
        );
        let report = json!({"invocation": invocation, "replayStatus": status.code(),
        "replayStdoutSha256": digest(&stdout), "replayStderrSha256": digest(&stderr),
        "replayStderr": std::str::from_utf8(&stderr).map_err(error)?, "dumps": dumps,
        "originalFailurePreserved": true, "directory": directory});
        fresh_json(&directory.join("observation.json"), &report)?;
        Ok(report)
    }

    pub(crate) fn run(command: &mut Command, names: &[&str]) -> Output {
        let mut output = command
            .output()
            .expect("run genuine pinned core source extraction");
        if !output.status.success()
            && let Some(directory) = std::env::var_os(DIRECTORY)
        {
            let observation = replay(command, &output, names, directory.into()).unwrap_or_else(|detail|
            json!({"status": "unavailable", "detail": detail, "originalFailurePreserved": true, "grantsAuthority": false}));
            output.stderr.extend_from_slice(
                b"\nObservation-only failed-source MIR replay (original status unchanged):\n",
            );
            output
                .stderr
                .extend_from_slice(serde_json::to_string(&observation).unwrap().as_bytes());
            output.stderr.push(b'\n');
        }
        output
    }

    #[test]
    fn diagnostic_filters_are_bounded_and_never_accept_flag_injection() {
        assert_eq!(
            observation_filter(&["first", "second"]).unwrap(),
            "first&runtime-optimized|second&runtime-optimized"
        );
        for names in [
            vec![],
            vec![""],
            vec!["a b"],
            vec!["x|-Copt-level=0"],
            vec!["*"],
            vec!["x"; 9],
        ] {
            assert!(observation_filter(&names).is_err());
        }
    }

    #[test]
    fn replay_mutation_preserves_program_arguments_cwd_and_other_environment() {
        for clear_environment in [false, true] {
            let mut command = Command::new("cargo");
            if clear_environment {
                command.env_clear();
            }
            command
                .args(["rustc", "--", "--cfg", "example=\"original\""])
                .current_dir("/tmp")
                .env("RETAIN", "value")
                .env(FLAGS, "original flags")
                .env("FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V1", "/tmp/original")
                .env_remove("REMOVE");
            let program = command.get_program().to_owned();
            let args: Vec<_> = command.get_args().map(OsStr::to_owned).collect();
            let cwd = command.get_current_dir().unwrap().to_owned();
            let mut expected: std::collections::BTreeMap<_, _> = command
                .get_envs()
                .map(|(key, value)| (key.to_owned(), value.map(OsStr::to_owned)))
                .collect();
            expected.insert(FLAGS.into(), Some("replay flags".into()));
            expected.insert(
                "FE2O3_EXTRACT_SIMULATION_BUNDLE_PATH_V1".into(),
                Some("/tmp/observation".into()),
            );
            configure_replay(&mut command, "replay flags", Path::new("/tmp/observation"));
            assert_eq!(command.get_program(), program);
            assert_eq!(
                command.get_args().map(OsStr::to_owned).collect::<Vec<_>>(),
                args
            );
            assert_eq!(command.get_current_dir(), Some(cwd.as_path()));
            let actual: std::collections::BTreeMap<_, _> = command
                .get_envs()
                .map(|(key, value)| (key.to_owned(), value.map(OsStr::to_owned)))
                .collect();
            assert_eq!(actual, expected);
        }
    }
}
