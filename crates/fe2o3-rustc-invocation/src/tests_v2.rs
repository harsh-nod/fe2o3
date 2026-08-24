use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::*;

fn fixture() -> RustcInvocationDescriptorV2 {
    let rustc = RustcUnitV2::new(
        "/workspace/fe2o3",
        vec![
            "/opt/fe2o3/rustc".into(),
            "--crate-name".into(),
            "fe2o3_device".into(),
            "crates/fe2o3-device/src/lib.rs".into(),
            "--crate-type=lib".into(),
            "--edition=2024".into(),
            "-Cmetadata=0123".into(),
            "-Zcodegen-backend=/opt/fe2o3/librustc_codegen_fe2o3.so".into(),
        ],
    )
    .expect("valid rustc unit");
    let environment = CompileEnvironmentV2::from_entries_for_test([
        ("CARGO_CFG_TARGET_ARCH", "amdgcn"),
        ("CARGO_FEATURE_FAST", "1"),
        ("FE2O3_HSACO_DIR", "/workspace/fe2o3/target/fe2o3"),
        ("FE2O3_TARGET", "gfx942:sramecc+:xnack-"),
        ("FE2O3_VERIFY_KERNEL_IR", "1"),
    ])
    .expect("valid environment");
    RustcInvocationDescriptorV2::new([0x22; 32], [0x33; 32], rustc, environment)
        .expect("valid descriptor")
}

fn procfd_fixture() -> RustcInvocationDescriptorV2 {
    let mut descriptor = fixture();
    *descriptor.rustc.argv.last_mut().unwrap() =
        crate::model_v2::Argument::new("-Zcodegen-backend=/proc/./self/fd/198").unwrap();
    revalidate(&descriptor).expect("valid procfd codegen backend capability")
}

fn find(bytes: &[u8], needle: &[u8]) -> usize {
    bytes
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("needle occurs in fixture")
}

fn replace_encoded_argument(bytes: &[u8], original: &str, replacement: &str) -> Vec<u8> {
    let offset = find(bytes, original.as_bytes());
    let length_offset = offset.checked_sub(4).expect("argument length prefix");
    assert_eq!(
        u32::from_le_bytes(bytes[length_offset..offset].try_into().unwrap()) as usize,
        original.len()
    );

    let mut replaced = bytes.to_vec();
    replaced.splice(
        length_offset..offset + original.len(),
        (replacement.len() as u32)
            .to_le_bytes()
            .into_iter()
            .chain(replacement.bytes()),
    );
    let replaced_len = replaced.len() as u32;
    replaced[12..16].copy_from_slice(&replaced_len.to_le_bytes());
    replaced
}

fn decode_error(bytes: &[u8]) -> DecodeError {
    decode_descriptor_v2(bytes).expect_err("mutated descriptor must be rejected")
}

fn digest(descriptor: &RustcInvocationDescriptorV2) -> InvocationDigestV2 {
    InvocationDigestV2::calculate(descriptor).expect("valid descriptor digest")
}

fn revalidate(
    descriptor: &RustcInvocationDescriptorV2,
) -> Result<RustcInvocationDescriptorV2, ValidationError> {
    RustcInvocationDescriptorV2::new(
        descriptor.rustc_executable_sha256,
        descriptor.codegen_backend_sha256,
        descriptor.rustc.clone(),
        descriptor.compile_environment.clone(),
    )
}

#[test]
fn full_round_trip_reencodes_byte_identically() {
    let descriptor = fixture();
    let encoded = encode_descriptor_v2(&descriptor).expect("encode");
    let decoded = decode_descriptor_v2(&encoded).expect("decode");
    assert_eq!(decoded, descriptor);
    assert_eq!(encode_descriptor_v2(&decoded).expect("re-encode"), encoded);

    assert_eq!(decoded.rustc().argv().count(), 8);
    assert_eq!(decoded.rustc_executable_sha256(), &[0x22; 32]);
    assert_eq!(decoded.codegen_backend_sha256(), &[0x33; 32]);
    assert_eq!(decoded.rustc_executable_path(), "/opt/fe2o3/rustc");
    assert_eq!(
        decoded.codegen_backend_path(),
        "/opt/fe2o3/librustc_codegen_fe2o3.so"
    );
    assert_eq!(decoded.amd_target(), "gfx942:sramecc+:xnack-");
    assert_eq!(
        decoded.artifact_output_directory(),
        "/workspace/fe2o3/target/fe2o3"
    );
    assert!(decoded.verification_required());
    assert_eq!(decoded.compile_environment().entries().len(), 5);
}

#[test]
fn procfd_codegen_backend_round_trip_preserves_exact_canonical_wire_bytes() {
    let descriptor = procfd_fixture();
    let encoded = encode_descriptor_v2(&descriptor).expect("encode procfd descriptor");
    let decoded = decode_descriptor_v2(&encoded).expect("decode procfd descriptor");

    assert_eq!(decoded, descriptor);
    assert_eq!(decoded.codegen_backend_path(), "/proc/./self/fd/198");
    assert_eq!(encode_descriptor_v2(&decoded).unwrap(), encoded);
}

#[test]
fn procfd_codegen_backend_codec_mutations_fail_closed() {
    const ORIGINAL: &str = "-Zcodegen-backend=/proc/./self/fd/198";
    let encoded = encode_descriptor_v2(&procfd_fixture()).expect("encode procfd descriptor");

    for path in [
        "/proc/./self/fd/",
        "/proc/./self/fd/0",
        "/proc/./self/fd/1",
        "/proc/./self/fd/2",
        "/proc/./self/fd/3",
        "/proc/./self/fd/197",
        "/proc/./self/fd/199",
        "/proc/./self/fd/00",
        "/proc/./self/fd/0198",
        "/proc/./self/fd/+198",
        "/proc/./self/fd/-1",
        "/proc/./self/fd/not-a-fd",
        "/proc/./self/fd/198.so",
        "/proc/./self/fd/198/extra",
        "/proc/./self/fd//198",
        "/proc/../self/fd/198",
        "/proc/././self/fd/198",
        "/proc/../../self/fd/198",
        "/proc/self/fd/198",
        "/proc/self/fd/0",
        "/proc/self/fd/1",
        "/proc/self/fd/2",
        "/proc/thread-self/fd/198",
        "/proc/thread-self/fd/0",
        "/proc/thread-self/fd/1",
        "/proc/thread-self/fd/2",
        "/proc/1/fd/198",
        "/proc/self/fd/../fd/198",
        "/proc//self/fd/198",
        "/proc/./self//fd/198",
        "/proc/./self/fd/198/",
        "/dev/fd/198",
        "/dev/fd/0",
        "/dev/fd/1",
        "/dev/fd/2",
        "/dev/fd/198/extra",
        "/dev/fd",
        "/dev/stdin",
        "/dev/stdout",
        "/dev/stderr",
        "/dev/stdin/0",
        "/dev/stdout/1",
        "/dev/stderr/2",
        "/dev/./fd/198",
        "/./proc/./self/fd/198",
        "//proc/./self/fd/198",
        "/proc/./self/fd/2147483648",
        "/proc/./self/fd/18446744073709551616",
    ] {
        let replacement = format!("-Zcodegen-backend={path}");
        let mutated = replace_encoded_argument(&encoded, ORIGINAL, &replacement);
        assert!(
            matches!(
                decode_error(&mutated),
                DecodeError::Validation(ValidationError::InvalidPath {
                    field: "codegen backend path"
                })
            ),
            "accepted malformed procfd capability {path}"
        );
    }
}

#[test]
fn ordinary_non_proc_backend_pathnames_remain_distinct_from_fd_aliases() {
    for path in [
        "/opt/fe2o3/backend.so",
        "/dev/stdin-backend.so",
        "/dev/stdout-backend.so",
        "/dev/stderr-backend.so",
        "/dev/fd-backend.so",
        "/proc-backend.so",
    ] {
        let mut descriptor = fixture();
        *descriptor.rustc.argv.last_mut().unwrap() =
            crate::model_v2::Argument::new(format!("-Zcodegen-backend={path}")).unwrap();
        assert!(
            revalidate(&descriptor).is_ok(),
            "rejected ordinary backend pathname {path}"
        );
    }
}

#[test]
fn procfd_capability_spelling_is_backend_selector_only() {
    assert!(RustcUnitV2::new("/proc/./self/fd/198", vec!["/rustc".into()]).is_err());

    let base = procfd_fixture();
    let mut argv_zero = base.clone();
    argv_zero.rustc.argv[0] = crate::model_v2::Argument::new("/proc/./self/fd/198").unwrap();
    assert!(revalidate(&argv_zero).is_err());

    let mut output = base;
    output
        .compile_environment
        .entries
        .iter_mut()
        .find(|entry| entry.key.as_str() == "FE2O3_HSACO_DIR")
        .unwrap()
        .value = crate::model_v2::EnvironmentValue::new("/proc/./self/fd/198").unwrap();
    assert!(revalidate(&output).is_err());

    let base = procfd_fixture();
    for argument in [
        "/proc/./self/fd/198",
        "--extern=backend=/proc/./self/fd/198",
        "--cfg=path=\"/proc/./self/fd/198\"",
    ] {
        let mut outside_selector = base.clone();
        outside_selector
            .rustc
            .argv
            .insert(1, crate::model_v2::Argument::new(argument).unwrap());
        assert!(
            revalidate(&outside_selector).is_err(),
            "accepted procfd capability outside the backend selector: {argument}"
        );
    }

    let mut environment = base;
    environment.compile_environment.entries[0].value =
        crate::model_v2::EnvironmentValue::new("/proc/./self/fd/198").unwrap();
    assert!(revalidate(&environment).is_err());
}

#[test]
fn golden_encoding_and_digest_are_stable() {
    const GOLDEN_WIRE_BYTES: usize = 490;
    const GOLDEN_DIGEST_HEX: &str =
        "3cd510a5f2dc2e63fa32c214bd6479ff058b929aace775d17827ab4d498e77ff";
    let encoded = encode_descriptor_v2(&fixture()).expect("encode");
    assert_eq!(encoded.len(), GOLDEN_WIRE_BYTES);
    assert_eq!(digest(&fixture()).to_hex(), GOLDEN_DIGEST_HEX);
}

#[test]
fn v1_and_v2_wire_versions_are_explicitly_disjoint() {
    let v2 = encode_descriptor_v2(&fixture()).unwrap();
    assert_eq!(
        decode_descriptor_v1(&v2),
        Err(DecodeError::UnknownVersion(2))
    );

    let v1 = encode_descriptor_v1(&super::tests::fixture()).unwrap();
    assert_eq!(
        decode_descriptor_v2(&v1),
        Err(DecodeError::UnknownVersion(1))
    );
}

#[test]
fn compiler_visible_settings_are_single_sourced() {
    let base = fixture();
    for (index, argument) in [
        (0, "/opt/other/rustc"),
        (2, "other_crate"),
        (3, "../device/src/lib.rs"),
        (4, "--crate-type=rlib"),
        (5, "--edition=2021"),
        (7, "-Zcodegen-backend=/opt/other/backend.so"),
    ] {
        let mut changed = base.clone();
        changed.rustc.argv[index] = crate::model_v2::Argument::new(argument).unwrap();
        let validated = revalidate(&changed).expect("complete compile shape remains valid");
        assert_ne!(digest(&validated), digest(&base));
    }

    for (key, value) in [
        ("FE2O3_TARGET", "gfx950"),
        ("FE2O3_HSACO_DIR", "/workspace/other"),
        ("FE2O3_VERIFY_KERNEL_IR", "0"),
    ] {
        let mut changed = base.clone();
        changed
            .compile_environment
            .entries
            .iter_mut()
            .find(|entry| entry.key.as_str() == key)
            .unwrap()
            .value = crate::model_v2::EnvironmentValue::new(value).unwrap();
        let validated = revalidate(&changed).expect("canonical setting remains valid");
        assert_ne!(digest(&validated), digest(&base));
    }
}

#[test]
fn classifier_separates_queries_from_compiles_and_rejects_ambiguity() {
    let base = fixture();
    for argument in [
        "--explain=E0001",
        "--help",
        "-h",
        "--version",
        "-V",
        "-vV",
        "-Zhelp",
    ] {
        let mut argv = base
            .rustc()
            .argv()
            .map(std::ffi::OsString::from)
            .collect::<Vec<_>>();
        argv.insert(1, argument.into());
        assert!(
            matches!(
                classify_rustc_invocation_v2(&argv),
                Ok(RustcInvocationV2::Terminal(_)) | Ok(RustcInvocationV2::Query(_))
            ),
            "classified {argument} as a compile"
        );
    }

    for argument in [
        "--print=file-names",
        "--print=native-static-libs",
        "--print=link-args",
        "-Zunpretty=hir",
        "-Zno-codegen",
    ] {
        let mut argv = base
            .rustc()
            .argv()
            .map(std::ffi::OsString::from)
            .collect::<Vec<_>>();
        argv.insert(1, argument.into());
        assert!(
            matches!(
                classify_rustc_invocation_v2(&argv),
                Ok(RustcInvocationV2::Compile(_))
            ),
            "{argument} bypassed compile classification"
        );
    }

    for warning_args in [&[][..], &["-Wwarnings"][..], &["-W", "warnings"][..]] {
        let cargo_probe = [
            "/opt/fe2o3/rustc",
            "-",
            "--crate-name",
            "___",
            "--print=file-names",
            "--crate-type=bin",
            "--crate-type=rlib",
        ]
        .into_iter()
        .chain(warning_args.iter().copied())
        .map(std::ffi::OsString::from)
        .collect::<Vec<_>>();
        assert!(matches!(
            classify_rustc_invocation_v2(&cargo_probe),
            Ok(RustcInvocationV2::Query(_))
        ));
    }

    for argument in [
        "@args.rsp",
        "unexpected",
        "--crate-name=duplicate",
        "src/other.rs",
    ] {
        let mut argv = base
            .rustc()
            .argv()
            .map(std::ffi::OsString::from)
            .collect::<Vec<_>>();
        argv.insert(1, argument.into());
        assert!(
            classify_rustc_invocation_v2(&argv).is_err(),
            "accepted {argument}"
        );
    }
}

#[test]
fn ordered_codegen_metadata_accepts_every_rustc_spelling_and_preserves_order() {
    let argv = [
        "rustc",
        "--crate-name",
        "unit",
        "unit.rs",
        "-C",
        "metadata=first",
        "-Cmetadata=second",
        "--codegen",
        "metadata=third",
        "--codegen=metadata=fourth",
        "-Copt-level=2",
        "--codegen",
        "debuginfo=0",
        "-Cmetadata=first",
    ]
    .map(std::ffi::OsString::from);
    let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&argv).unwrap() else {
        panic!("fixture must be a compile invocation");
    };

    assert_eq!(
        ordered_rustc_codegen_metadata_v1(compile).unwrap(),
        ["first", "second", "third", "fourth", "first"]
    );
}

#[test]
fn cargo_metadata_observation_preserves_order_duplicates_and_exact_domain() {
    let ordered = derive_cargo_metadata_build_observation_v2(&["first", "second", "first"]);
    let reordered = derive_cargo_metadata_build_observation_v2(&["first", "first", "second"]);
    let deduplicated = derive_cargo_metadata_build_observation_v2(&["first", "second"]);

    assert_ne!(ordered, reordered);
    assert_ne!(ordered, deduplicated);
    assert_eq!(
        ordered.to_hex(),
        "02bb68e8c8b5aa67c836f32263beaa4738b50f4689f0622d75130fbf9f7008a9"
    );
    assert_ne!(ordered.into_bytes(), [0; 32]);
}

#[test]
fn ordered_codegen_metadata_distinguishes_absent_and_empty_values() {
    let absent =
        ["rustc", "--crate-name", "unit", "unit.rs", "-Copt-level=2"].map(std::ffi::OsString::from);
    let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&absent).unwrap() else {
        panic!("fixture must be a compile invocation");
    };
    assert!(
        ordered_rustc_codegen_metadata_v1(compile)
            .unwrap()
            .is_empty()
    );

    for (tail, argument_index) in [
        (vec!["-C", "metadata="], 5),
        (vec!["-Cmetadata="], 4),
        (vec!["--codegen", "metadata="], 5),
        (vec!["--codegen=metadata="], 4),
    ] {
        let argv = ["rustc", "--crate-name", "unit", "unit.rs"]
            .into_iter()
            .chain(tail)
            .map(std::ffi::OsString::from)
            .collect::<Vec<_>>();
        let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&argv).unwrap()
        else {
            panic!("fixture must be a compile invocation");
        };
        assert_eq!(
            ordered_rustc_codegen_metadata_v1(compile),
            Err(RustcCodegenMetadataErrorV1::EmptyMetadata { argument_index })
        );
    }
}

#[cfg(unix)]
#[test]
fn ordered_codegen_metadata_rejects_non_utf8_split_and_joined_values() {
    use std::os::unix::ffi::OsStringExt;

    for (tail, argument_index) in [
        (
            vec![
                std::ffi::OsString::from("-C"),
                std::ffi::OsString::from_vec(b"metadata=private-\xff".to_vec()),
            ],
            5,
        ),
        (
            vec![std::ffi::OsString::from_vec(
                b"-Cmetadata=private-\xff".to_vec(),
            )],
            4,
        ),
        (
            vec![
                std::ffi::OsString::from("--codegen"),
                std::ffi::OsString::from_vec(b"metadata=private-\xff".to_vec()),
            ],
            5,
        ),
        (
            vec![std::ffi::OsString::from_vec(
                b"--codegen=metadata=private-\xff".to_vec(),
            )],
            4,
        ),
    ] {
        let argv = ["rustc", "--crate-name", "unit", "unit.rs"]
            .map(std::ffi::OsString::from)
            .into_iter()
            .chain(tail)
            .collect::<Vec<_>>();
        let RustcInvocationV2::Compile(compile) = classify_rustc_invocation_v2(&argv).unwrap()
        else {
            panic!("fixture must be a compile invocation");
        };
        let error = ordered_rustc_codegen_metadata_v1(compile).unwrap_err();
        assert_eq!(
            error,
            RustcCodegenMetadataErrorV1::NonUtf8CodegenOption { argument_index }
        );
        assert_eq!(
            error.to_string(),
            format!("rustc codegen option at argv[{argument_index}] is not valid UTF-8")
        );
    }
}

#[test]
fn backend_assignment_is_a_unique_canonical_final_argument() {
    let base = fixture();

    let mut missing = base.clone();
    missing.rustc.argv.pop();
    assert!(revalidate(&missing).is_err());

    for duplicate in [
        vec!["-Zcodegen-backend=/opt/other.so"],
        vec!["-Zcodegen_backend=/opt/other.so"],
        vec!["-Z=codegen-backend=/opt/other.so"],
        vec!["-Z=codegen_backend=/opt/other.so"],
        vec!["-Z", "codegen-backend=/opt/other.so"],
        vec!["-Z", "codegen_backend=/opt/other.so"],
        vec!["-Z", "codegen-backend", "/opt/other.so"],
        vec!["-Z", "codegen_backend", "/opt/other.so"],
        vec!["-Zcodegen-backend", "/opt/other.so"],
        vec!["-Zcodegen_backend", "/opt/other.so"],
        vec!["codegen-backend=/opt/other.so"],
        vec!["codegen_backend=/opt/other.so"],
    ] {
        let mut changed = base.clone();
        for argument in duplicate.into_iter().rev() {
            changed
                .rustc
                .argv
                .insert(1, crate::model_v2::Argument::new(argument).unwrap());
        }
        assert!(revalidate(&changed).is_err());
    }

    let mut separate = base.clone();
    separate.rustc.argv.pop();
    separate
        .rustc
        .argv
        .push(crate::model_v2::Argument::new("-Z").unwrap());
    separate
        .rustc
        .argv
        .push(crate::model_v2::Argument::new("codegen-backend=/opt/fe2o3/backend.so").unwrap());
    assert!(revalidate(&separate).is_err());

    for replacement in [
        "-Zcodegen_backend=/opt/fe2o3/backend.so",
        "-Z=codegen-backend=/opt/fe2o3/backend.so",
        "-Z=codegen_backend=/opt/fe2o3/backend.so",
        "codegen-backend=/opt/fe2o3/backend.so",
    ] {
        let mut changed = base.clone();
        *changed.rustc.argv.last_mut().unwrap() =
            crate::model_v2::Argument::new(replacement).unwrap();
        assert!(revalidate(&changed).is_err(), "accepted {replacement}");
    }
}

#[test]
fn option_terminator_before_managed_backend_is_rejected() {
    let mut changed = fixture();
    changed
        .rustc
        .argv
        .insert(7, crate::model_v2::Argument::new("--").unwrap());
    assert!(revalidate(&changed).is_err());

    const FINAL: &str = "-Zcodegen-backend=/opt/fe2o3/librustc_codegen_fe2o3.so";
    let encoded = encode_descriptor_v2(&fixture()).unwrap();
    let mutated = replace_encoded_argument(&encoded, "-Cmetadata=0123", "--");
    assert!(matches!(decode_error(&mutated), DecodeError::Validation(_)));

    let replaced = replace_encoded_argument(&encoded, FINAL, "--");
    assert!(matches!(
        decode_error(&replaced),
        DecodeError::Validation(_)
    ));
}

#[test]
fn backend_selector_codec_mutations_fail_closed() {
    const FINAL: &str = "-Zcodegen-backend=/opt/fe2o3/librustc_codegen_fe2o3.so";
    let encoded = encode_descriptor_v2(&fixture()).unwrap();

    for replacement in [
        "-Zcodegen_backend=/opt/other.so",
        "-Z=codegen-backend=/opt/other.so",
        "-Z=codegen_backend=/opt/other.so",
        "codegen-backend=/opt/other.so",
    ] {
        let mutated = replace_encoded_argument(&encoded, FINAL, replacement);
        assert!(
            matches!(decode_error(&mutated), DecodeError::Validation(_)),
            "decoded final selector mutation {replacement}"
        );
    }

    for duplicate in [
        "-Zcodegen-backend=/opt/other.so",
        "-Zcodegen_backend=/opt/other.so",
        "-Z=codegen-backend=/opt/other.so",
        "-Z=codegen_backend=/opt/other.so",
        "codegen-backend=/opt/other.so",
        "codegen_backend=/opt/other.so",
    ] {
        let mutated = replace_encoded_argument(&encoded, "-Cmetadata=0123", duplicate);
        assert!(
            matches!(decode_error(&mutated), DecodeError::Validation(_)),
            "decoded duplicate selector mutation {duplicate}"
        );
    }
}

#[test]
fn source_paths_are_exact_and_interpreted_under_the_recorded_cwd() {
    let base = fixture();
    for source in [
        "./src/lib.rs",
        "../device/src/lib.rs",
        "/workspace/src/lib.rs",
    ] {
        let mut changed = base.clone();
        changed.rustc.argv[3] = crate::model_v2::Argument::new(source).unwrap();
        assert!(revalidate(&changed).is_ok(), "rejected source {source}");
        assert_ne!(digest(&changed), digest(&base));
    }
}

#[test]
fn every_semantic_field_affects_the_digest() {
    let original = fixture();
    let expected = digest(&original);
    let mut mutations: Vec<(&str, RustcInvocationDescriptorV2)> = Vec::new();

    macro_rules! mutation {
        ($name:literal, $body:expr) => {{
            let mut value = original.clone();
            $body(&mut value);
            mutations.push(($name, value));
        }};
    }

    mutation!("rustc digest", |value: &mut RustcInvocationDescriptorV2| {
        value.rustc_executable_sha256[0] ^= 1;
    });
    mutation!(
        "backend digest",
        |value: &mut RustcInvocationDescriptorV2| {
            value.codegen_backend_sha256[0] ^= 1;
        }
    );
    mutation!(
        "working directory",
        |value: &mut RustcInvocationDescriptorV2| {
            value.rustc.working_directory =
                crate::model_v2::AbsolutePath::new("/workspace/other", "rustc working directory")
                    .unwrap();
        }
    );
    mutation!("argv", |value: &mut RustcInvocationDescriptorV2| {
        value.rustc.argv[2] = crate::model_v2::Argument::new("other_crate").unwrap();
    });
    mutation!(
        "environment key",
        |value: &mut RustcInvocationDescriptorV2| {
            value.compile_environment.entries[0].key =
                crate::model_v2::Name::new("CARGO_CFG_TARGET_CPU", "compile environment key")
                    .unwrap();
        }
    );
    mutation!(
        "environment value",
        |value: &mut RustcInvocationDescriptorV2| {
            value.compile_environment.entries[0].value =
                crate::model_v2::EnvironmentValue::new("amdgpu").unwrap();
        }
    );

    for (field, value) in mutations {
        assert_ne!(digest(&value), expected, "{field} did not affect digest");
    }
}

#[test]
fn every_truncation_and_trailing_data_are_rejected() {
    let encoded = encode_descriptor_v2(&fixture()).expect("encode");
    for length in 0..encoded.len() {
        assert!(
            decode_descriptor_v2(&encoded[..length]).is_err(),
            "accepted truncation at {length}"
        );
    }

    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(decode_error(&trailing), DecodeError::TrailingBytes);

    let trailing_len = trailing.len() as u32;
    trailing[12..16].copy_from_slice(&trailing_len.to_le_bytes());
    assert_eq!(decode_error(&trailing), DecodeError::TrailingBytes);
}

#[test]
fn header_reserved_fields_and_utf8_are_rejected() {
    let encoded = encode_descriptor_v2(&fixture()).expect("encode");

    let mut invalid = encoded.clone();
    invalid[0] ^= 1;
    assert_eq!(decode_error(&invalid), DecodeError::InvalidMagic);

    let mut invalid = encoded.clone();
    invalid[8..10].copy_from_slice(&3_u16.to_le_bytes());
    assert_eq!(decode_error(&invalid), DecodeError::UnknownVersion(3));

    let mut invalid = encoded.clone();
    invalid[10..12].copy_from_slice(&1_u16.to_le_bytes());
    assert_eq!(decode_error(&invalid), DecodeError::UnsupportedFlags(1));

    let mut invalid = encoded.clone();
    invalid[12..16].copy_from_slice(&19_u32.to_le_bytes());
    assert_eq!(
        decode_error(&invalid),
        DecodeError::InvalidLength { declared: 19 }
    );

    let mut invalid = encoded.clone();
    invalid[16] = 1;
    assert_eq!(
        decode_error(&invalid),
        DecodeError::NonzeroReserved {
            field: "descriptor header"
        }
    );

    for (needle, field) in [
        (b"/workspace/fe2o3".as_slice(), "rustc working directory"),
        (b"/opt/fe2o3/rustc".as_slice(), "rustc argument"),
        (
            b"CARGO_CFG_TARGET_ARCH".as_slice(),
            "compile environment key",
        ),
        (b"amdgcn".as_slice(), "compile environment value"),
    ] {
        let mut invalid = encoded.clone();
        invalid[find(&encoded, needle)] = 0xff;
        assert_eq!(
            decode_error(&invalid),
            DecodeError::InvalidUtf8 { field },
            "field {field}"
        );
    }

    let first_key = find(&encoded, b"CARGO_CFG_TARGET_ARCH");
    let environment_count = first_key - 6;
    let mut invalid = encoded;
    invalid[environment_count + 2] = 1;
    assert_eq!(
        decode_error(&invalid),
        DecodeError::NonzeroReserved {
            field: "compile environment"
        }
    );
}

#[test]
fn decoder_checks_every_length_class_and_count_before_allocation() {
    let encoded = encode_descriptor_v2(&fixture()).expect("encode");

    for (needle, prefix, raw, field) in [
        (
            b"/workspace/fe2o3".as_slice(),
            4,
            (MAX_PATH_BYTES_V2 + 1) as u32,
            "rustc working directory",
        ),
        (
            b"/opt/fe2o3/rustc".as_slice(),
            4,
            (MAX_ARGUMENT_BYTES_V2 + 1) as u32,
            "rustc argument",
        ),
        (
            b"amdgcn".as_slice(),
            4,
            (MAX_ENVIRONMENT_VALUE_BYTES_V2 + 1) as u32,
            "compile environment value",
        ),
    ] {
        let offset = find(&encoded, needle);
        let mut invalid = encoded.clone();
        invalid[offset - prefix..offset].copy_from_slice(&raw.to_le_bytes());
        assert!(matches!(
            decode_error(&invalid),
            DecodeError::CountOutOfRange { field: actual, .. } if actual == field
        ));
    }

    let key = find(&encoded, b"CARGO_CFG_TARGET_ARCH");
    let mut invalid = encoded.clone();
    invalid[key - 2..key].copy_from_slice(&((MAX_NAME_BYTES_V2 + 1) as u16).to_le_bytes());
    assert!(matches!(
        decode_error(&invalid),
        DecodeError::CountOutOfRange {
            field: "compile environment key",
            ..
        }
    ));

    let argv_zero = find(&encoded, b"/opt/fe2o3/rustc");
    let argument_count = argv_zero - 8;
    let mut invalid = encoded.clone();
    invalid[argument_count..argument_count + 4]
        .copy_from_slice(&((MAX_RUSTC_ARGUMENTS_V2 + 1) as u32).to_le_bytes());
    assert!(matches!(
        decode_error(&invalid),
        DecodeError::CountOutOfRange {
            field: "rustc arguments",
            ..
        }
    ));

    let first_key = find(&encoded, b"CARGO_CFG_TARGET_ARCH");
    let environment_count = first_key - 6;
    let mut invalid = encoded.clone();
    invalid[environment_count..environment_count + 2]
        .copy_from_slice(&((MAX_COMPILE_ENVIRONMENT_ENTRIES_V2 + 1) as u16).to_le_bytes());
    assert!(matches!(
        decode_error(&invalid),
        DecodeError::CountOutOfRange {
            field: "compile environment",
            ..
        }
    ));

    let mut oversized = vec![0_u8; MAX_DESCRIPTOR_BYTES_V2 + 1];
    oversized[..8].copy_from_slice(&INVOCATION_DESCRIPTOR_MAGIC_V2);
    assert_eq!(
        decode_error(&oversized),
        DecodeError::TooLarge {
            max: MAX_DESCRIPTOR_BYTES_V2
        }
    );
}

#[test]
fn paths_and_typed_environment_views_fail_closed() {
    assert!(RustcUnitV2::new("relative", vec!["/rustc".into()]).is_err());
    assert!(RustcUnitV2::new("/workspace/../other", vec!["/rustc".into()]).is_err());
    assert!(RustcUnitV2::new("/workspace\\other", vec!["/rustc".into()]).is_err());
    assert!(RustcUnitV2::new("/workspace", vec![]).is_err());
    assert!(RustcUnitV2::new("/workspace", vec!["a\0b".into()]).is_err());

    let base = fixture();
    for (key, value) in [
        ("FE2O3_TARGET", "gfx999"),
        ("FE2O3_HSACO_DIR", "relative/output"),
        ("FE2O3_VERIFY_KERNEL_IR", "true"),
    ] {
        let mut changed = base.clone();
        changed
            .compile_environment
            .entries
            .iter_mut()
            .find(|entry| entry.key.as_str() == key)
            .unwrap()
            .value = crate::model_v2::EnvironmentValue::new(value).unwrap();
        assert!(revalidate(&changed).is_err(), "accepted {key}={value}");
    }

    for required_key in ["FE2O3_TARGET", "FE2O3_HSACO_DIR"] {
        let mut changed = base.clone();
        changed
            .compile_environment
            .entries
            .retain(|entry| entry.key.as_str() != required_key);
        assert!(
            revalidate(&changed).is_err(),
            "accepted missing {required_key}"
        );
    }
}

#[test]
fn compile_environment_capture_is_complete_sorted_and_closed() {
    let environment =
        CompileEnvironmentV2::from_entries_for_test([("Z", "last"), ("EMPTY", ""), ("A", "first")])
            .unwrap();
    assert_eq!(
        environment
            .entries()
            .iter()
            .map(|entry| (entry.key(), entry.value()))
            .collect::<Vec<_>>(),
        [("A", "first"), ("EMPTY", ""), ("Z", "last")]
    );

    assert!(matches!(
        CompileEnvironmentV2::from_entries_for_test([("A", "1"), ("A", "2")]),
        Err(ValidationError::Duplicate {
            field: "compile environment"
        })
    ));
    for key in [
        "FE2O3_TRANSPORT_BUILD_SESSION",
        "FE2O3_TRANSPORT_ARTIFACT_ATTEMPT",
        "FE2O3_TRANSPORT_DESCRIPTOR_FD",
        "FE2O3_TRANSPORT_FUTURE_FIELD",
    ] {
        assert!(matches!(
            CompileEnvironmentV2::from_entries_for_test([(key, "value")]),
            Err(ValidationError::ForbiddenEnvironmentVariable { key: rejected })
                if rejected == key
        ));
    }

    let explicit = CompileEnvironmentV2::from_child_environment([
        (std::ffi::OsString::from("B"), std::ffi::OsString::from("2")),
        (std::ffi::OsString::from("A"), std::ffi::OsString::from("1")),
    ])
    .unwrap();
    let mut command = std::process::Command::new("rustc");
    command.env("MUST_BE_CLEARED", "value");
    explicit.configure_command(&mut command);
    assert_eq!(
        command
            .get_envs()
            .map(|(key, value)| (
                key.to_string_lossy().into_owned(),
                value.unwrap().to_string_lossy().into_owned(),
            ))
            .collect::<Vec<_>>(),
        [("A".into(), "1".into()), ("B".into(), "2".into())]
    );
}

#[test]
fn compile_environment_stops_at_the_entry_bound() {
    use std::cell::Cell;
    use std::rc::Rc;

    let pulled = Rc::new(Cell::new(0));
    let observed = Rc::clone(&pulled);
    let entries = std::iter::from_fn(move || {
        let index = observed.get();
        observed.set(index + 1);
        Some((
            std::ffi::OsString::from(format!("K{index:04}")),
            std::ffi::OsString::from("value"),
        ))
    });
    assert!(matches!(
        CompileEnvironmentV2::from_child_environment(entries),
        Err(ValidationError::TooMany {
            field: "compile environment",
            max: MAX_COMPILE_ENVIRONMENT_ENTRIES_V2,
        })
    ));
    assert_eq!(pulled.get(), MAX_COMPILE_ENVIRONMENT_ENTRIES_V2 + 1);
}

#[cfg(unix)]
#[test]
fn compile_environment_rejects_non_utf8() {
    use std::os::unix::ffi::OsStringExt;

    assert!(matches!(
        CompileEnvironmentV2::from_entries_for_test([(
            std::ffi::OsString::from_vec(vec![0xff]),
            std::ffi::OsString::from("value"),
        )]),
        Err(ValidationError::NonUtf8Environment { field: "key" })
    ));
    assert!(matches!(
        CompileEnvironmentV2::from_entries_for_test([(
            std::ffi::OsString::from("KEY"),
            std::ffi::OsString::from_vec(vec![0xff]),
        )]),
        Err(ValidationError::NonUtf8Environment { field: "value" })
    ));
}

#[test]
fn canonical_collection_and_complete_descriptor_limits_are_enforced() {
    let reversed = vec![
        CompileEnvironmentEntryV2::new("Z", "1").unwrap(),
        CompileEnvironmentEntryV2::new("A", "1").unwrap(),
    ];
    assert!(matches!(
        CompileEnvironmentV2::from_encoded_entries(reversed),
        Err(ValidationError::NonCanonicalOrder {
            field: "compile environment"
        })
    ));

    assert!(
        RustcUnitV2::new(
            "/workspace",
            vec![String::new(); MAX_RUSTC_ARGUMENTS_V2 + 1],
        )
        .is_err()
    );

    let mut oversized = fixture();
    oversized.rustc.argv = vec!["x".repeat(MAX_ARGUMENT_BYTES_V2); 65]
        .into_iter()
        .map(|argument| crate::model_v2::Argument::new(argument).unwrap())
        .collect();
    assert_eq!(
        encode_descriptor_v2(&oversized),
        Err(ValidationError::EncodedDescriptorTooLarge {
            max: MAX_DESCRIPTOR_BYTES_V2
        })
    );
}

#[test]
fn digest_bytes_and_hex_are_canonical_and_nonzero() {
    let value = digest(&fixture());
    assert_eq!(
        InvocationDigestV2::from_bytes(value.into_bytes()).unwrap(),
        value
    );
    assert_eq!(
        InvocationDigestV2::from_hex(&value.to_hex()).unwrap(),
        value
    );
    assert_eq!(value.to_string(), value.to_hex());
    assert_eq!(value.to_hex().parse::<InvocationDigestV2>().unwrap(), value);
    assert_eq!(
        InvocationDigestV2::from_bytes([0; 32]),
        Err(DigestError::ReservedAllZero)
    );
    assert_eq!(
        InvocationDigestV2::from_hex(&"0".repeat(64)),
        Err(DigestError::ReservedAllZero)
    );
    assert_eq!(
        InvocationDigestV2::from_hex("00"),
        Err(DigestError::InvalidHexLength)
    );
    assert!(matches!(
        InvocationDigestV2::from_hex(&"A".repeat(64)),
        Err(DigestError::InvalidHexCharacter { index: 0 })
    ));
}

#[test]
fn deterministic_corruption_corpus_never_panics() {
    let encoded = encode_descriptor_v2(&fixture()).expect("encode");
    for index in 0..encoded.len() {
        for mask in [1, 0x80, 0xff] {
            let mut mutated = encoded.clone();
            mutated[index] ^= mask;
            assert!(
                catch_unwind(AssertUnwindSafe(|| decode_descriptor_v2(&mutated))).is_ok(),
                "decoder panicked at byte {index}, mask {mask:#x}"
            );
        }
    }

    let mut state = 0x9e37_79b9_u32;
    for _ in 0..2048 {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let length = (state as usize) % (encoded.len() + 65);
        let mut bytes = vec![0_u8; length];
        for byte in &mut bytes {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *byte = (state >> 24) as u8;
        }
        assert!(catch_unwind(|| decode_descriptor_v2(&bytes)).is_ok());
    }
}
