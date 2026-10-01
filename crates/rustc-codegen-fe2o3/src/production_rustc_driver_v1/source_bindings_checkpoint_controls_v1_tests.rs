//! Inert controls only. No authenticated owner or successful compiler output is built.
use super::*;
use std::io::{Cursor, Read};

fn config(mode: &str, oracle: serde_json::Value) -> Config {
    serde_json::from_value(serde_json::json!({
        "schema":SCHEMA,"mode":mode,
        "preparation":{"path":"/fixture/prepared.json","sha256":"11".repeat(32)},
        "ordinary_oracle":oracle
    }))
    .unwrap()
}
fn preparation() -> Preparation {
    Preparation {
        schema: "fe2o3-recipe-series-prepared-v1".into(),
        repository_cwd: PathBuf::from("/fixture"),
        source_relative: PathBuf::from("source.rs"),
        source_absolute: PathBuf::from("/fixture/source.rs"),
        source_bytes: 3,
        source_sha256: "11".repeat(32),
        rustc_args: vec!["rustc".into(), "/fixture/source.rs".into()],
        required_child_environment: (0..6)
            .map(|i| (format!("KEY{i}"), "value".into()))
            .collect(),
        compiler_frontends_entered: 0,
        recipe_created: false,
        child_processes_spawned: 0,
        grants_artifact_or_launch_authority: false,
    }
}
#[test]
fn checkpoint_config_is_closed_and_observed_requires_independent_oracle() {
    let oracle = serde_json::json!({"path":"/fixture/normal.json","sha256":"22".repeat(32)});
    assert!(validate_config(&config("ordinary", serde_json::Value::Null)).is_ok());
    assert!(validate_config(&config("observed", oracle.clone())).is_ok());
    assert!(validate_config(&config("observed", serde_json::Value::Null)).is_err());
    for mode in ["ordinary", "deny_bytes", "deny_items"] {
        assert!(validate_config(&config(mode, oracle.clone())).is_err());
    }
    let valid = serde_json::json!({"schema":SCHEMA,"mode":"ordinary",
        "preparation":{"path":"/fixture/p.json","sha256":"11".repeat(32)},"ordinary_oracle":null});
    for mode in ["warm", "replay", "caller_selected"] {
        let mut v = valid.clone();
        v["mode"] = mode.into();
        assert!(serde_json::from_value::<Config>(v).is_err());
    }
    let mut v = valid;
    v["unrecognized"] = true.into();
    assert!(serde_json::from_value::<Config>(v).is_err());
}
#[test]
fn checkpoint_preparation_requires_original_unentered_non_authoritative_shape() {
    assert!(validate_preparation_shape(&preparation()).is_ok());
    for change in 0..6 {
        let mut p = preparation();
        match change {
            0 => p.compiler_frontends_entered = 1,
            1 => p.recipe_created = true,
            2 => p.child_processes_spawned = 1,
            3 => p.grants_artifact_or_launch_authority = true,
            4 => p.source_bytes = 0,
            _ => p.source_bytes = SOURCE_CAP + 1,
        }
        assert!(validate_preparation_shape(&p).is_err());
    }
}
#[test]
fn checkpoint_preparation_rejects_source_argv_path_and_environment_mismatch() {
    for change in 0..7 {
        let mut p = preparation();
        match change {
            0 => p.source_relative = PathBuf::from("../source.rs"),
            1 => p.source_absolute = PathBuf::from("/different/source.rs"),
            2 => p.rustc_args.push("/fixture/source.rs".into()),
            3 => p.rustc_args[1] = "/fixture/other.rs".into(),
            4 => p.rustc_args.push("x".repeat(4097)),
            5 => {
                p.required_child_environment.remove("KEY0");
            }
            _ => {
                p.required_child_environment
                    .insert("KEY0".into(), "\0".into());
            }
        }
        assert!(validate_preparation_shape(&p).is_err());
    }
}
fn error(error: LogicalStorageErrorV1) -> CheckpointError {
    CheckpointError::Storage(BindingsStorageErrorV1::Counter(error))
}
#[test]
fn checkpoint_denial_preserves_exact_original_counter_layer_and_variant() {
    assert_eq!(
        expected_denial(Mode::DenyBytes, error(LogicalStorageErrorV1::ByteLimit)).unwrap(),
        "ByteLimit"
    );
    assert_eq!(
        expected_denial(Mode::DenyItems, error(LogicalStorageErrorV1::ItemLimit)).unwrap(),
        "ItemLimit"
    );
    assert!(expected_denial(Mode::DenyBytes, error(LogicalStorageErrorV1::ItemLimit)).is_err());
    assert!(expected_denial(Mode::DenyItems, error(LogicalStorageErrorV1::ByteLimit)).is_err());
}
#[test]
fn checkpoint_denial_does_not_accept_other_errors_or_positive_modes() {
    for mode in [
        Mode::Ordinary,
        Mode::Observed,
        Mode::DenyBytes,
        Mode::DenyItems,
    ] {
        assert!(expected_denial(mode, error(LogicalStorageErrorV1::Arithmetic)).is_err());
        assert!(
            expected_denial(
                mode,
                CheckpointError::Storage(BindingsStorageErrorV1::UnsupportedProtectedCustody)
            )
            .is_err()
        );
    }
    assert!(expected_denial(Mode::Ordinary, error(LogicalStorageErrorV1::ByteLimit)).is_err());
    assert!(expected_denial(Mode::Observed, error(LogicalStorageErrorV1::ItemLimit)).is_err());
}
#[test]
fn checkpoint_fatal_absent_and_repeated_callback_never_yield_success() {
    for (entries, fatal) in [(0, false), (2, false), (1, true), (0, true), (2, true)] {
        let inert = Outcome::Verified {
            normal: b"{}".to_vec(),
            storage: None,
        };
        assert!(finish(entries, fatal, Some(Ok(inert))).is_err());
    }
    assert!(finish(1, false, None).is_err());
    assert_eq!(
        finish(1, false, Some(Err("original failure".into())))
            .err()
            .unwrap(),
        "original failure"
    );
    assert!(matches!(
        finish(1, false, Some(Ok(Outcome::Denied("ByteLimit")))).unwrap(),
        Outcome::Denied("ByteLimit")
    ));
}
#[test]
fn checkpoint_observation_limits_are_distinct_and_not_constructor_changes() {
    assert_eq!(
        Mode::Observed.limits(),
        LogicalStorageLimitsV1 {
            max_bytes: Some(134_217_728),
            max_items: 1_000_000
        }
    );
    assert_eq!(
        Mode::DenyBytes.limits(),
        LogicalStorageLimitsV1 {
            max_bytes: Some(0),
            max_items: 1_000_000
        }
    );
    assert_eq!(
        Mode::DenyItems.limits(),
        LogicalStorageLimitsV1 {
            max_bytes: Some(134_217_728),
            max_items: 0
        }
    );
    assert!(!Mode::Ordinary.denied());
    assert!(!Mode::Observed.denied());
}
#[test]
fn checkpoint_oracle_compares_complete_bytes_not_only_digest_or_counts() {
    let a = br#"{"ranked_ir":"write a","sites":[[1,2,3]]}"#;
    let b = br#"{"ranked_ir":"write b","sites":[[1,2,3]]}"#;
    assert!(compare(a, a).is_ok());
    assert!(compare(a, b).is_err());
    assert!(compare(a, &a[..a.len() - 1]).is_err());
}
#[test]
fn checkpoint_bounded_reader_accepts_exact_and_short_reads_with_real_eof() {
    struct Short<'a>(&'a [u8]);
    impl Read for Short<'_> {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            let n = out.len().min(self.0.len()).min(3);
            out[..n].copy_from_slice(&self.0[..n]);
            self.0 = &self.0[n..];
            Ok(n)
        }
    }
    let bytes = vec![17; 2051];
    assert_eq!(
        io::read_bounded(&mut Short(&bytes), bytes.len()).unwrap(),
        bytes
    );
    assert_eq!(
        io::read_bounded(&mut bytes.as_slice(), bytes.len() + 1).unwrap(),
        bytes
    );
}
#[test]
fn checkpoint_reader_rejects_empty_oversize_and_overflow_before_unbounded_read() {
    assert!(io::read_bounded(&mut &b""[..], 4).is_err());
    let mut reader = Cursor::new([1_u8; 16]);
    assert!(io::read_bounded(&mut reader, 4).is_err());
    assert_eq!(reader.position(), 5);
    let mut reader = Cursor::new([1_u8; 16]);
    assert!(io::read_bounded(&mut reader, usize::MAX).is_err());
    assert_eq!(reader.position(), 0);
}
#[test]
fn checkpoint_reader_does_not_treat_io_error_as_eof() {
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("original read failure"))
        }
    }
    assert_eq!(
        io::read_bounded(&mut Broken, 16).err().unwrap(),
        "original read failure"
    );
}
#[test]
fn checkpoint_json_envelope_refuses_one_short_without_success_bytes() {
    let value = serde_json::json!({"a":"line\nquoted\""});
    let ordinary = serde_json::to_vec(&value).unwrap();
    assert_eq!(json_bytes(&value, ordinary.len()).unwrap(), ordinary);
    assert!(json_bytes(&value, ordinary.len() - 1).is_err());
    assert!(!ordinary.contains(&b'\n'));
    assert!(!ordinary.contains(&b'\r'));
}
#[test]
fn checkpoint_hash_shape_and_exact_content_hash_are_discriminating() {
    assert!(hash(&"ab".repeat(32)).is_ok());
    for bad in ["", "AA", &"AA".repeat(32), &"0".repeat(63), &"g".repeat(64)] {
        assert!(hash(bad).is_err());
    }
    assert_eq!(
        digest(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_ne!(digest(b"abc"), digest(b"abd"));
}
#[test]
fn checkpoint_failure_diagnostic_is_bounded_on_utf8_boundaries() {
    let value = diagnostic("é".repeat(300));
    assert!(value.len() <= 512);
    assert!(std::str::from_utf8(value.as_bytes()).is_ok());
    assert_eq!(diagnostic("original reason".into()), "original reason");
}
