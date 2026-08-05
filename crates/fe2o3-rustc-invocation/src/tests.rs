use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::*;

fn tool(version: &str, byte: u8) -> ToolIdentityV1 {
    ToolIdentityV1::new(version, [byte; 32]).expect("valid tool identity")
}

fn fixture() -> RustcInvocationDescriptorV1 {
    let cargo = CargoIdentityV1::new(
        tool("cargo 1.96.0-nightly", 0x11),
        CargoPackageV1::new("fe2o3-device", "0.1.0", "crates/fe2o3-device/Cargo.toml")
            .expect("valid package"),
        CargoTargetV1::new(
            "vecadd",
            CargoTargetKindV1::Library,
            vec![CrateTypeV1::Lib, CrateTypeV1::Rlib],
            EditionV1::Rust2024,
            "crates/fe2o3-device/src/lib.rs",
            vec!["fast".into(), "verify".into()],
        )
        .expect("valid target"),
    );
    let rustc = RustcIdentityV1::new(
        tool("rustc 1.96.0-nightly (55e86c996 2026-04-02)", 0x22),
        RustcUnitV1::new(
            "fe2o3_device",
            "x86_64-unknown-linux-gnu",
            "amdgcn-amd-amdhsa",
            TestStateV1::NotTest,
            vec![
                "--crate-name".into(),
                "fe2o3_device".into(),
                "crates/fe2o3-device/src/lib.rs".into(),
                "--crate-type=lib".into(),
                "-Cmetadata=0123".into(),
                "-Zcodegen-backend=/opt/fe2o3/librustc_codegen_fe2o3.so".into(),
            ],
        )
        .expect("valid rustc unit"),
    );
    let tools = BackendToolsV1::new(
        tool("rustc-codegen-fe2o3 0.1.0", 0x33),
        tool("clang version 23.0.0", 0x44),
        tool("LLD 23.0.0", 0x55),
        Some(tool("LLVM readobj 23.0.0", 0x66)),
    );
    let device = DeviceConfigurationV1::new(
        AmdTargetIdTextV1::new("gfx942:sramecc+:xnack-").expect("valid AMD target"),
        VerificationModeV1::Required,
    );
    let output = OutputDomainV1::new("/workspace/fe2o3", "/workspace/fe2o3/target/fe2o3")
        .expect("valid output domain");
    let environment = vec![
        CompileEnvironmentEntryV1::new("CARGO_CFG_TARGET_ARCH", "amdgcn")
            .expect("valid environment"),
        CompileEnvironmentEntryV1::new("CARGO_FEATURE_FAST", "1").expect("valid environment"),
        CompileEnvironmentEntryV1::new("FE2O3_TARGET", "gfx942:sramecc+:xnack-")
            .expect("valid environment"),
    ];
    RustcInvocationDescriptorV1::new(cargo, rustc, tools, device, output, environment)
        .expect("valid descriptor")
}

fn from_hex(value: &str) -> Vec<u8> {
    assert!(value.len().is_multiple_of(2));
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let value = std::str::from_utf8(pair).expect("ASCII hex");
            u8::from_str_radix(value, 16).expect("valid hex")
        })
        .collect()
}

fn find(bytes: &[u8], needle: &[u8]) -> usize {
    bytes
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("needle occurs in fixture")
}

fn decode_error(bytes: &[u8]) -> DecodeError {
    decode_descriptor_v1(bytes).expect_err("mutated descriptor must be rejected")
}

fn digest(descriptor: &RustcInvocationDescriptorV1) -> InvocationDigest {
    InvocationDigest::calculate(descriptor).expect("valid descriptor digest")
}

#[test]
fn full_round_trip_reencodes_byte_identically() {
    let descriptor = fixture();
    let encoded = encode_descriptor_v1(&descriptor).expect("encode");
    let decoded = decode_descriptor_v1(&encoded).expect("decode");
    assert_eq!(decoded, descriptor);
    assert_eq!(encode_descriptor_v1(&decoded).expect("re-encode"), encoded);

    assert_eq!(decoded.cargo().package().name(), "fe2o3-device");
    assert_eq!(decoded.cargo().target().features().count(), 2);
    assert_eq!(decoded.rustc().unit().argv().count(), 6);
    assert_eq!(
        decoded.tools().inspector().unwrap().version(),
        "LLVM readobj 23.0.0"
    );
    assert_eq!(
        decoded.device().amd_target().as_str(),
        "gfx942:sramecc+:xnack-"
    );
    assert_eq!(decoded.output().workspace_root(), "/workspace/fe2o3");
    assert_eq!(decoded.compile_environment().len(), 3);
}

#[test]
fn golden_encoding_and_digest_are_stable() {
    const GOLDEN_WIRE_HEX: &str = concat!(
        "4645324f33524900010000008b030000000000001400636172676f20312e39362e302d6e696768746c7911111111111111111111111111111111111111111111111111111111111111110c006665326f",
        "332d6465766963650500302e312e301e0000006372617465732f6665326f332d6465766963652f436172676f2e746f6d6c060076656361646401000400000002000200000000001e0000006372617465",
        "732f6665326f332d6465766963652f7372632f6c69622e72730100020004006661737406007665726966792b00727573746320312e39362e302d6e696768746c79202835356538366339393620323032",
        "362d30342d30322922222222222222222222222222222222222222222222222222222222222222220c006665326f335f64657669636518007838365f36342d756e6b6e6f776e2d6c696e75782d676e75",
        "1100616d6467636e2d616d642d616d6468736101000000060000000c0000002d2d63726174652d6e616d650c0000006665326f335f6465766963651e0000006372617465732f6665326f332d64657669",
        "63652f7372632f6c69622e7273100000002d2d63726174652d747970653d6c69620f0000002d436d657461646174613d30313233360000002d5a636f646567656e2d6261636b656e643d2f6f70742f66",
        "65326f332f6c696272757374635f636f646567656e5f6665326f332e736f190072757374632d636f646567656e2d6665326f3320302e312e303333333333333333333333333333333333333333333333",
        "3333333333333333331400636c616e672076657273696f6e2032332e302e3044444444444444444444444444444444444444444444444444444444444444440a004c4c442032332e302e305555555555",
        "5555555555555555555555555555555555555555555555555555550100000013004c4c564d20726561646f626a2032332e302e3066666666666666666666666666666666666666666666666666666666",
        "6666666616006766783934323a7372616d6563632b3a786e61636b2d02000000100000002f776f726b73706163652f6665326f331d0000002f776f726b73706163652f6665326f332f7461726765742f",
        "6665326f33030000001500434152474f5f4346475f5441524745545f4152434806000000616d6467636e1200434152474f5f464541545552455f4641535401000000310c004645324f335f5441524745",
        "54160000006766783934323a7372616d6563632b3a786e61636b2d",
    );
    const GOLDEN_DIGEST_HEX: &str =
        "9b775db43bec49442f98f00ddd31bb0435e7c5ff5227f94ca6eb33245e82e01f";
    let encoded = encode_descriptor_v1(&fixture()).expect("encode");
    assert_eq!(encoded, from_hex(GOLDEN_WIRE_HEX));
    assert_eq!(digest(&fixture()).to_hex(), GOLDEN_DIGEST_HEX);
}

#[test]
fn every_semantic_field_affects_the_digest() {
    let original = fixture();
    let expected = digest(&original);
    let mut mutations: Vec<(&str, RustcInvocationDescriptorV1)> = Vec::new();

    macro_rules! mutation {
        ($name:literal, $body:expr) => {{
            let mut value = original.clone();
            $body(&mut value);
            mutations.push(($name, value));
        }};
    }

    mutation!(
        "Cargo version",
        |value: &mut RustcInvocationDescriptorV1| {
            value.cargo.executable.version =
                crate::model::Text::new("cargo 1.96.1", "tool version").unwrap();
        }
    );
    mutation!(
        "Cargo executable",
        |value: &mut RustcInvocationDescriptorV1| {
            value.cargo.executable.executable_sha256[0] ^= 1;
        }
    );
    mutation!("package name", |value: &mut RustcInvocationDescriptorV1| {
        value.cargo.package.name =
            crate::model::Name::new("fe2o3-host", "Cargo package name").unwrap();
    });
    mutation!(
        "package version",
        |value: &mut RustcInvocationDescriptorV1| {
            value.cargo.package.version =
                crate::model::Text::new("0.2.0", "Cargo package version").unwrap();
        }
    );
    mutation!("manifest", |value: &mut RustcInvocationDescriptorV1| {
        value.cargo.package.manifest_path =
            crate::model::RelativePath::new("crates/fe2o3-host/Cargo.toml", "Cargo manifest path")
                .unwrap();
    });
    mutation!("target name", |value: &mut RustcInvocationDescriptorV1| {
        value.cargo.target.name = crate::model::Name::new("saxpy", "Cargo target name").unwrap();
    });
    mutation!("target kind", |value: &mut RustcInvocationDescriptorV1| {
        value.cargo.target.kind = CargoTargetKindV1::Example;
    });
    mutation!("crate types", |value: &mut RustcInvocationDescriptorV1| {
        value.cargo.target.crate_types = vec![CrateTypeV1::Rlib];
    });
    mutation!("edition", |value: &mut RustcInvocationDescriptorV1| {
        value.cargo.target.edition = EditionV1::Rust2021;
    });
    mutation!("source", |value: &mut RustcInvocationDescriptorV1| {
        value.cargo.target.source_path = crate::model::RelativePath::new(
            "crates/fe2o3-device/src/device.rs",
            "Cargo target source path",
        )
        .unwrap();
    });
    mutation!("features", |value: &mut RustcInvocationDescriptorV1| {
        value.cargo.target.features[0] =
            crate::model::Name::new("faster", "Cargo feature").unwrap();
    });
    mutation!(
        "rustc version",
        |value: &mut RustcInvocationDescriptorV1| {
            value.rustc.executable.version =
                crate::model::Text::new("rustc changed", "tool version").unwrap();
        }
    );
    mutation!(
        "rustc executable",
        |value: &mut RustcInvocationDescriptorV1| {
            value.rustc.executable.executable_sha256[0] ^= 1;
        }
    );
    mutation!("crate name", |value: &mut RustcInvocationDescriptorV1| {
        value.rustc.unit.crate_name =
            crate::model::Name::new("fe2o3_host", "rustc crate name").unwrap();
    });
    mutation!("host target", |value: &mut RustcInvocationDescriptorV1| {
        value.rustc.unit.host_target =
            crate::model::Name::new("aarch64-unknown-linux-gnu", "rustc host target").unwrap();
    });
    mutation!(
        "effective target",
        |value: &mut RustcInvocationDescriptorV1| {
            value.rustc.unit.effective_target =
                crate::model::Name::new("x86_64-unknown-linux-gnu", "rustc effective target")
                    .unwrap();
        }
    );
    mutation!("test state", |value: &mut RustcInvocationDescriptorV1| {
        value.rustc.unit.test_state = TestStateV1::Test;
    });
    mutation!("argv", |value: &mut RustcInvocationDescriptorV1| {
        value.rustc.unit.argv[1] = crate::model::Argument::new("other_crate").unwrap();
    });
    mutation!("backend", |value: &mut RustcInvocationDescriptorV1| {
        value.tools.backend.executable_sha256[0] ^= 1;
    });
    mutation!("clang", |value: &mut RustcInvocationDescriptorV1| {
        value.tools.clang.executable_sha256[0] ^= 1;
    });
    mutation!("linker", |value: &mut RustcInvocationDescriptorV1| {
        value.tools.linker.executable_sha256[0] ^= 1;
    });
    mutation!("inspector", |value: &mut RustcInvocationDescriptorV1| {
        value.tools.inspector = None;
    });
    mutation!("AMD target", |value: &mut RustcInvocationDescriptorV1| {
        value.device.amd_target = AmdTargetIdTextV1::new("gfx950:sramecc+:xnack-").unwrap();
    });
    mutation!("verification", |value: &mut RustcInvocationDescriptorV1| {
        value.device.verification = VerificationModeV1::Disabled;
    });
    mutation!(
        "workspace root",
        |value: &mut RustcInvocationDescriptorV1| {
            value.output.workspace_root =
                crate::model::AbsolutePath::new("/workspace/other", "workspace root").unwrap();
        }
    );
    mutation!(
        "artifact output",
        |value: &mut RustcInvocationDescriptorV1| {
            value.output.artifact_output_directory = crate::model::AbsolutePath::new(
                "/workspace/fe2o3/target/other",
                "artifact output directory",
            )
            .unwrap();
        }
    );
    mutation!(
        "environment key",
        |value: &mut RustcInvocationDescriptorV1| {
            value.compile_environment[0].key =
                crate::model::Name::new("CARGO_CFG_TARGET_CPU", "compile environment key").unwrap();
        }
    );
    mutation!(
        "environment value",
        |value: &mut RustcInvocationDescriptorV1| {
            value.compile_environment[0].value =
                crate::model::EnvironmentValue::new("amdgpu").unwrap();
        }
    );

    for (field, value) in mutations {
        assert_ne!(digest(&value), expected, "{field} did not affect digest");
    }
}

#[test]
fn every_truncation_and_trailing_data_are_rejected() {
    let encoded = encode_descriptor_v1(&fixture()).expect("encode");
    for length in 0..encoded.len() {
        assert!(
            decode_descriptor_v1(&encoded[..length]).is_err(),
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
fn header_utf8_tags_and_reserved_fields_are_rejected() {
    let encoded = encode_descriptor_v1(&fixture()).expect("encode");

    let mut invalid = encoded.clone();
    invalid[0] ^= 1;
    assert_eq!(decode_error(&invalid), DecodeError::InvalidMagic);

    let mut invalid = encoded.clone();
    invalid[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(decode_error(&invalid), DecodeError::UnknownVersion(2));

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

    let package_name = find(&encoded, b"fe2o3-device");
    let mut invalid = encoded.clone();
    invalid[package_name] = 0xff;
    assert_eq!(
        decode_error(&invalid),
        DecodeError::InvalidUtf8 {
            field: "Cargo package name"
        }
    );

    let target_name = find(&encoded, b"vecadd");
    let kind = target_name + b"vecadd".len();
    let edition = kind + 2;
    let target_flags = edition + 2;
    for (offset, expected_kind) in [(kind, "Cargo target kind"), (edition, "Rust edition")] {
        let mut invalid = encoded.clone();
        invalid[offset..offset + 2].copy_from_slice(&0_u16.to_le_bytes());
        assert!(matches!(
            decode_error(&invalid),
            DecodeError::UnknownTag { kind, tag: 0 } if kind == expected_kind
        ));
    }
    let mut invalid = encoded.clone();
    invalid[target_flags] = 1;
    assert!(matches!(
        decode_error(&invalid),
        DecodeError::NonzeroReserved {
            field: "Cargo target flags"
        }
    ));

    let source = find(&encoded, b"crates/fe2o3-device/src/lib.rs");
    let first_crate_type = source + b"crates/fe2o3-device/src/lib.rs".len();
    let mut invalid = encoded.clone();
    invalid[first_crate_type..first_crate_type + 2].copy_from_slice(&0_u16.to_le_bytes());
    assert!(matches!(
        decode_error(&invalid),
        DecodeError::UnknownTag {
            kind: "crate type",
            tag: 0
        }
    ));

    let effective = find(&encoded, b"amdgcn-amd-amdhsa");
    let test_state = effective + b"amdgcn-amd-amdhsa".len();
    let mut invalid = encoded.clone();
    invalid[test_state] = 0;
    assert!(matches!(
        decode_error(&invalid),
        DecodeError::UnknownTag {
            kind: "rustc test state",
            tag: 0
        }
    ));

    let linker_digest = find(&encoded, &[0x55; 32]);
    let inspector_presence = linker_digest + 32;
    let mut invalid = encoded.clone();
    invalid[inspector_presence] = 2;
    assert!(matches!(
        decode_error(&invalid),
        DecodeError::UnknownTag {
            kind: "inspector presence",
            tag: 2
        }
    ));

    let amd_target = find(&encoded, b"gfx942:sramecc+:xnack-");
    let verification = amd_target + b"gfx942:sramecc+:xnack-".len();
    let mut invalid = encoded;
    invalid[verification] = 0;
    assert!(matches!(
        decode_error(&invalid),
        DecodeError::UnknownTag {
            kind: "verification mode",
            tag: 0
        }
    ));
}

#[test]
fn decoder_checks_lengths_and_counts_before_allocation() {
    let encoded = encode_descriptor_v1(&fixture()).expect("encode");

    let package_name = find(&encoded, b"fe2o3-device");
    let mut invalid = encoded.clone();
    invalid[package_name - 2..package_name]
        .copy_from_slice(&((MAX_NAME_BYTES + 1) as u16).to_le_bytes());
    assert!(matches!(
        decode_error(&invalid),
        DecodeError::CountOutOfRange {
            field: "Cargo package name",
            ..
        }
    ));

    let first_argument = find(&encoded, b"--crate-name");
    let argument_count = first_argument - 8;
    let mut invalid = encoded.clone();
    invalid[argument_count..argument_count + 4]
        .copy_from_slice(&((MAX_RUSTC_ARGUMENTS + 1) as u32).to_le_bytes());
    assert!(matches!(
        decode_error(&invalid),
        DecodeError::CountOutOfRange {
            field: "rustc arguments",
            ..
        }
    ));

    let first_environment_key = find(&encoded, b"CARGO_CFG_TARGET_ARCH");
    let environment_count = first_environment_key - 6;
    let mut invalid = encoded.clone();
    invalid[environment_count..environment_count + 2]
        .copy_from_slice(&((MAX_COMPILE_ENVIRONMENT_ENTRIES + 1) as u16).to_le_bytes());
    assert!(matches!(
        decode_error(&invalid),
        DecodeError::CountOutOfRange {
            field: "compile environment",
            ..
        }
    ));

    let mut oversized = vec![0_u8; MAX_DESCRIPTOR_BYTES + 1];
    oversized[..8].copy_from_slice(&INVOCATION_DESCRIPTOR_MAGIC);
    assert_eq!(
        decode_error(&oversized),
        DecodeError::TooLarge {
            max: MAX_DESCRIPTOR_BYTES
        }
    );
}

#[test]
fn names_text_and_paths_enforce_their_bounds() {
    assert!(CargoPackageV1::new("", "1", "Cargo.toml").is_err());
    assert!(CargoPackageV1::new("a\0b", "1", "Cargo.toml").is_err());
    assert!(CargoPackageV1::new("a".repeat(MAX_NAME_BYTES + 1), "1", "Cargo.toml").is_err());
    assert!(ToolIdentityV1::new("", [0; 32]).is_err());
    assert!(ToolIdentityV1::new("v\0x", [0; 32]).is_err());
    assert!(ToolIdentityV1::new("v".repeat(MAX_TEXT_BYTES + 1), [0; 32]).is_err());

    for relative in [
        "",
        "/Cargo.toml",
        "./Cargo.toml",
        "a/../Cargo.toml",
        "a//Cargo.toml",
        "a/./Cargo.toml",
        "a/Cargo.toml/",
        "a\\Cargo.toml",
        "a\0Cargo.toml",
    ] {
        assert!(
            CargoPackageV1::new("pkg", "1", relative).is_err(),
            "accepted relative path {relative:?}"
        );
    }
    assert!(CargoPackageV1::new("pkg", "1", "a/Cargo.toml").is_ok());

    for absolute in [
        "",
        "workspace",
        "//workspace",
        "/workspace//repo",
        "/workspace/./repo",
        "/workspace/../repo",
        "/workspace/repo/",
        "/workspace\\repo",
        "/workspace\0repo",
    ] {
        assert!(
            OutputDomainV1::new(absolute, "/output").is_err(),
            "accepted absolute path {absolute:?}"
        );
    }
    assert!(OutputDomainV1::new("/", "/output").is_ok());
    assert!(OutputDomainV1::new("/workspace", "/output").is_ok());
    assert!(OutputDomainV1::new(format!("/{}", "a".repeat(MAX_PATH_BYTES)), "/output").is_err());
}

#[test]
fn amd_target_text_requires_known_canonical_supported_spelling() {
    for valid in [
        "gfx1151",
        "gfx942:sramecc-",
        "gfx942:xnack+",
        "gfx950:sramecc+:xnack-",
    ] {
        assert_eq!(AmdTargetIdTextV1::new(valid).unwrap().as_str(), valid);
    }
    for invalid in [
        "",
        "GFX942",
        "gfx999",
        "gfx942:",
        "gfx942:xnack",
        "gfx942:xnack-:sramecc+",
        "gfx942:sramecc+:sramecc-",
        "gfx1151:xnack+",
        "gfx942:unknown+",
    ] {
        assert!(
            AmdTargetIdTextV1::new(invalid).is_err(),
            "accepted AMD target {invalid:?}"
        );
    }
}

#[test]
fn set_like_inputs_must_be_strictly_sorted_and_unique() {
    let target = |crate_types, features| {
        CargoTargetV1::new(
            "target",
            CargoTargetKindV1::Library,
            crate_types,
            EditionV1::Rust2024,
            "src/lib.rs",
            features,
        )
    };
    assert!(matches!(
        target(vec![CrateTypeV1::Rlib, CrateTypeV1::Lib], vec![]),
        Err(ValidationError::NonCanonicalOrder {
            field: "crate types"
        })
    ));
    assert!(matches!(
        target(vec![CrateTypeV1::Rlib, CrateTypeV1::Rlib], vec![]),
        Err(ValidationError::Duplicate {
            field: "crate types"
        })
    ));
    assert!(matches!(
        target(vec![CrateTypeV1::Lib], vec!["z".into(), "a".into()]),
        Err(ValidationError::NonCanonicalOrder {
            field: "Cargo features"
        })
    ));
    assert!(matches!(
        target(vec![CrateTypeV1::Lib], vec!["a".into(), "a".into()]),
        Err(ValidationError::Duplicate {
            field: "Cargo features"
        })
    ));

    let mut original = fixture();
    let reversed = vec![
        CompileEnvironmentEntryV1::new("Z", "1").unwrap(),
        CompileEnvironmentEntryV1::new("A", "1").unwrap(),
    ];
    assert!(matches!(
        RustcInvocationDescriptorV1::new(
            original.cargo.clone(),
            original.rustc.clone(),
            original.tools.clone(),
            original.device.clone(),
            original.output.clone(),
            reversed,
        ),
        Err(ValidationError::NonCanonicalOrder {
            field: "compile environment"
        })
    ));
    original.compile_environment = vec![
        CompileEnvironmentEntryV1::new("A", "1").unwrap(),
        CompileEnvironmentEntryV1::new("A", "2").unwrap(),
    ];
    assert!(matches!(
        RustcInvocationDescriptorV1::new(
            original.cargo,
            original.rustc,
            original.tools,
            original.device,
            original.output,
            original.compile_environment,
        ),
        Err(ValidationError::Duplicate {
            field: "compile environment"
        })
    ));
}

#[test]
fn argv_is_ordered_repeatable_and_distinct_from_environment_sets() {
    let base = fixture();
    let mut repeated = base.clone();
    repeated.rustc.unit.argv = vec![
        crate::model::Argument::new("--cfg").unwrap(),
        crate::model::Argument::new("feature=fast").unwrap(),
        crate::model::Argument::new("--cfg").unwrap(),
        crate::model::Argument::new("").unwrap(),
    ];
    let encoded = encode_descriptor_v1(&repeated).expect("repeated argv encodes");
    assert_eq!(decode_descriptor_v1(&encoded).unwrap(), repeated);

    let mut reordered = repeated.clone();
    reordered.rustc.unit.argv.swap(0, 1);
    assert_ne!(digest(&reordered), digest(&repeated));
    assert_eq!(digest(&fixture()), digest(&base));

    assert!(CompileEnvironmentEntryV1::new("EMPTY", "").is_ok());
    assert!(CompileEnvironmentEntryV1::new("BAD=KEY", "x").is_err());
    assert!(
        RustcUnitV1::new(
            "crate",
            "host",
            "target",
            TestStateV1::NotTest,
            vec!["a\0b".into()]
        )
        .is_err()
    );
}

#[test]
fn complete_descriptor_and_collection_limits_are_enforced() {
    assert!(
        RustcUnitV1::new(
            "crate",
            "host",
            "target",
            TestStateV1::NotTest,
            vec![String::new(); MAX_RUSTC_ARGUMENTS + 1],
        )
        .is_err()
    );

    let mut oversized = fixture();
    oversized.rustc.unit.argv = vec!["x".repeat(MAX_ARGUMENT_BYTES); 65]
        .into_iter()
        .map(|argument| crate::model::Argument::new(argument).unwrap())
        .collect();
    assert_eq!(
        encode_descriptor_v1(&oversized),
        Err(ValidationError::EncodedDescriptorTooLarge {
            max: MAX_DESCRIPTOR_BYTES
        })
    );

    let environment = (0..=MAX_COMPILE_ENVIRONMENT_ENTRIES)
        .map(|index| CompileEnvironmentEntryV1::new(format!("K{index:04}"), "v").unwrap())
        .collect();
    let base = fixture();
    assert!(matches!(
        RustcInvocationDescriptorV1::new(
            base.cargo,
            base.rustc,
            base.tools,
            base.device,
            base.output,
            environment,
        ),
        Err(ValidationError::TooMany {
            field: "compile environment",
            max: MAX_COMPILE_ENVIRONMENT_ENTRIES
        })
    ));
}

#[test]
fn digest_bytes_and_hex_are_canonical_and_nonzero() {
    let value = digest(&fixture());
    assert_eq!(
        InvocationDigest::from_bytes(value.into_bytes()).unwrap(),
        value
    );
    assert_eq!(InvocationDigest::from_hex(&value.to_hex()).unwrap(), value);
    assert_eq!(value.to_string(), value.to_hex());
    assert_eq!(value.to_hex().parse::<InvocationDigest>().unwrap(), value);
    assert_eq!(
        InvocationDigest::from_bytes([0; 32]),
        Err(DigestError::ReservedAllZero)
    );
    assert_eq!(
        InvocationDigest::from_hex(&"0".repeat(64)),
        Err(DigestError::ReservedAllZero)
    );
    assert_eq!(
        InvocationDigest::from_hex("00"),
        Err(DigestError::InvalidHexLength)
    );
    assert!(matches!(
        InvocationDigest::from_hex(&"A".repeat(64)),
        Err(DigestError::InvalidHexCharacter { index: 0 })
    ));
}

#[test]
fn deterministic_corruption_corpus_never_panics() {
    let encoded = encode_descriptor_v1(&fixture()).expect("encode");
    for index in 0..encoded.len() {
        for mask in [1, 0x80, 0xff] {
            let mut mutated = encoded.clone();
            mutated[index] ^= mask;
            assert!(
                catch_unwind(AssertUnwindSafe(|| decode_descriptor_v1(&mutated))).is_ok(),
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
        assert!(catch_unwind(|| decode_descriptor_v1(&bytes)).is_ok());
    }
}
