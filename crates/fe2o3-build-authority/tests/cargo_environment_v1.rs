use fe2o3_build_authority::{
    AUTHORITY_CARGO_ENVIRONMENT_ENTRY_COUNT_V1, AUTHORITY_CARGO_ENVIRONMENT_HEADER_LEN_V1,
    AUTHORITY_CARGO_ENVIRONMENT_IDENTITY_DOMAIN_V1, AUTHORITY_CARGO_ENVIRONMENT_MAGIC_V1,
    AUTHORITY_CARGO_ENVIRONMENT_MAX_PATH_LEN_V1, AUTHORITY_CARGO_ENVIRONMENT_MAX_RAW_VALUE_LEN_V1,
    AUTHORITY_CARGO_ENVIRONMENT_MAX_WIRE_LEN_V1, AUTHORITY_CARGO_ENVIRONMENT_TARGET_V1,
    AUTHORITY_CARGO_ENVIRONMENT_VERSION_V1, AUTHORITY_CARGO_MODE_ARGV_V1,
    AuthorityCargoEnvironmentErrorV1, AuthorityCargoEnvironmentPathErrorV1,
    AuthorityCargoEnvironmentV1, AuthorityCargoEnvironmentVariableV1, CapabilityBindingV3,
    ForbiddenCargoEnvironmentChannelV1, PipelineV1, authority_cargo_environment_identity_sha256_v1,
    decode_authority_cargo_environment_v1,
};

const CACHE_IDENTITY: [u8; 32] = [0xa5; 32];
const GOLDEN_WIRE_HEX: &str = "46324155454e5631010040000900000011010000030000000000000000000000a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a50a001000434152474f5f484f4d452f617574686f726974792f636172676f11000400434152474f5f4e45545f4f46464c494e457472756510001100434152474f5f5441524745545f4449522f617574686f726974792f7461726765740c000d004645324f335f5441524745546766783934323a786e61636b2d04000f00484f4d452f617574686f726974792f686f6d65040007004c414e47432e5554462d38060007004c435f414c4c432e5554462d3806000e00544d504449522f617574686f726974792f746d7002000300545a555443";
const GOLDEN_IDENTITY: [u8; 32] = [
    0x3d, 0x99, 0x2a, 0x1f, 0xc6, 0x09, 0x93, 0xc4, 0xa6, 0xd7, 0xce, 0x52, 0xaf, 0x8e, 0x6e, 0x89,
    0x96, 0xe6, 0xfd, 0x49, 0x8e, 0xf8, 0xc3, 0xf1, 0xed, 0x9c, 0xb1, 0x6d, 0xe6, 0xd7, 0x76, 0xa3,
];

fn golden_entries() -> Vec<(Vec<u8>, Vec<u8>)> {
    [
        ("CARGO_HOME", "/authority/cargo"),
        ("CARGO_NET_OFFLINE", "true"),
        ("CARGO_TARGET_DIR", "/authority/target"),
        ("FE2O3_TARGET", "gfx942:xnack-"),
        ("HOME", "/authority/home"),
        ("LANG", "C.UTF-8"),
        ("LC_ALL", "C.UTF-8"),
        ("TMPDIR", "/authority/tmp"),
        ("TZ", "UTC"),
    ]
    .into_iter()
    .map(|(name, value)| (name.as_bytes().to_vec(), value.as_bytes().to_vec()))
    .collect()
}

fn from_entries(
    entries: &[(Vec<u8>, Vec<u8>)],
    cache_identity: [u8; 32],
) -> Result<AuthorityCargoEnvironmentV1, AuthorityCargoEnvironmentErrorV1> {
    AuthorityCargoEnvironmentV1::new(
        entries
            .iter()
            .map(|(name, value)| (name.as_slice(), value.as_slice())),
        cache_identity,
    )
}

fn golden_environment() -> AuthorityCargoEnvironmentV1 {
    from_entries(&golden_entries(), CACHE_IDENTITY).unwrap()
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").unwrap();
    }
    output
}

fn set_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn set_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn raw_wire(entries: &[(Vec<u8>, Vec<u8>)]) -> Vec<u8> {
    let total_len = usize::from(AUTHORITY_CARGO_ENVIRONMENT_HEADER_LEN_V1)
        + entries
            .iter()
            .map(|(name, value)| 4 + name.len() + value.len())
            .sum::<usize>();
    let mut wire = Vec::with_capacity(total_len);
    wire.extend_from_slice(&AUTHORITY_CARGO_ENVIRONMENT_MAGIC_V1);
    wire.extend_from_slice(&AUTHORITY_CARGO_ENVIRONMENT_VERSION_V1.to_le_bytes());
    wire.extend_from_slice(&AUTHORITY_CARGO_ENVIRONMENT_HEADER_LEN_V1.to_le_bytes());
    wire.extend_from_slice(&AUTHORITY_CARGO_ENVIRONMENT_ENTRY_COUNT_V1.to_le_bytes());
    wire.extend_from_slice(&0_u16.to_le_bytes());
    wire.extend_from_slice(&(total_len as u32).to_le_bytes());
    wire.extend_from_slice(&3_u32.to_le_bytes());
    wire.extend_from_slice(&[0; 8]);
    wire.extend_from_slice(&CACHE_IDENTITY);
    for (name, value) in entries {
        wire.extend_from_slice(&(name.len() as u16).to_le_bytes());
        wire.extend_from_slice(&(value.len() as u16).to_le_bytes());
        wire.extend_from_slice(name);
        wire.extend_from_slice(value);
    }
    wire
}

#[test]
fn roundtrip_and_cross_implementation_golden_are_stable() {
    let environment = golden_environment();
    let wire = environment.encode();

    assert_eq!(wire.len(), 273);
    assert_eq!(hex(&wire), GOLDEN_WIRE_HEX);
    assert_eq!(&wire[..8], &AUTHORITY_CARGO_ENVIRONMENT_MAGIC_V1);
    assert_eq!(environment.identity_sha256(), GOLDEN_IDENTITY);
    assert_eq!(
        authority_cargo_environment_identity_sha256_v1(&wire),
        Ok(GOLDEN_IDENTITY)
    );
    assert_eq!(
        decode_authority_cargo_environment_v1(&wire),
        Ok(environment.clone())
    );
    assert_eq!(environment.provisioned_cargo_cache_sha256(), CACHE_IDENTITY);
    assert!(environment.offline());
    assert!(environment.frozen());
    assert_eq!(environment.cargo_mode_argv(), ["--offline", "--frozen"]);
    assert_eq!(environment.cargo_mode_argv(), AUTHORITY_CARGO_MODE_ARGV_V1);
    assert_eq!(
        AUTHORITY_CARGO_ENVIRONMENT_IDENTITY_DOMAIN_V1,
        b"FE2O3/AUTHORITY-CARGO-ENVIRONMENT/V1\0"
    );
    assert_eq!(AUTHORITY_CARGO_ENVIRONMENT_TARGET_V1, "gfx942:xnack-");
    assert!(wire.len() <= AUTHORITY_CARGO_ENVIRONMENT_MAX_WIRE_LEN_V1);
}

#[test]
fn exact_sorted_map_is_insertion_order_independent() {
    let entries = golden_entries();
    let canonical = from_entries(&entries, CACHE_IDENTITY).unwrap();

    let mut reversed = entries.clone();
    reversed.reverse();
    let mut rotated = entries.clone();
    rotated.rotate_left(4);

    for candidate in [&reversed, &rotated] {
        let environment = from_entries(candidate, CACHE_IDENTITY).unwrap();
        assert_eq!(environment, canonical);
        assert_eq!(environment.encode(), canonical.encode());
        assert_eq!(environment.identity_sha256(), canonical.identity_sha256());
    }

    let map = canonical.environment();
    assert_eq!(map.len(), 9);
    assert!(map.windows(2).all(|pair| pair[0].0 < pair[1].0));
    assert_eq!(
        map,
        [
            ("CARGO_HOME", "/authority/cargo"),
            ("CARGO_NET_OFFLINE", "true"),
            ("CARGO_TARGET_DIR", "/authority/target"),
            ("FE2O3_TARGET", "gfx942:xnack-"),
            ("HOME", "/authority/home"),
            ("LANG", "C.UTF-8"),
            ("LC_ALL", "C.UTF-8"),
            ("TMPDIR", "/authority/tmp"),
            ("TZ", "UTC"),
        ]
    );
}

#[test]
fn identity_is_accepted_by_capability_binding_v3() {
    let identity = golden_environment().identity_sha256();
    let binding = CapabilityBindingV3::new(
        [1; 32],
        [2; 32],
        [3; 32],
        PipelineV1::CollectedTiledGemm,
        identity,
        [4; 32],
        [5; 32],
        [6; 32],
        [7; 32],
        [8; 32],
        None,
    )
    .unwrap();
    assert_eq!(binding.cargo_environment_identity(), identity);
}

#[test]
fn every_valid_field_and_cache_byte_is_identity_sensitive() {
    let entries = golden_entries();
    let original = from_entries(&entries, CACHE_IDENTITY)
        .unwrap()
        .identity_sha256();

    for index in [0, 2, 4, 7] {
        let mut changed = entries.clone();
        changed[index].1.extend_from_slice(b"-changed");
        assert_ne!(
            from_entries(&changed, CACHE_IDENTITY)
                .unwrap()
                .identity_sha256(),
            original,
            "path field {index}"
        );
    }

    for byte_index in 0..32 {
        let mut cache = CACHE_IDENTITY;
        cache[byte_index] ^= 1;
        let changed = from_entries(&entries, cache).unwrap();
        assert_ne!(changed.encode(), golden_environment().encode());
        assert_ne!(
            changed.identity_sha256(),
            original,
            "cache byte {byte_index}"
        );
        assert_eq!(
            decode_authority_cargo_environment_v1(&changed.encode()),
            Ok(changed)
        );
    }
}

#[test]
fn every_single_wire_bit_mutation_fails_or_changes_identity() {
    let wire = golden_environment().encode();
    for byte_index in 0..wire.len() {
        for bit in 0..8 {
            let mut changed = wire.clone();
            changed[byte_index] ^= 1 << bit;
            if let Ok(decoded) = decode_authority_cargo_environment_v1(&changed) {
                assert_eq!(decoded.encode(), changed, "byte {byte_index}, bit {bit}");
                assert_ne!(
                    decoded.identity_sha256(),
                    GOLDEN_IDENTITY,
                    "byte {byte_index}, bit {bit}"
                );
            }
        }
    }
}

#[test]
fn duplicate_missing_unknown_and_non_utf8_inputs_fail_closed() {
    let entries = golden_entries();

    for (index, variable) in [
        AuthorityCargoEnvironmentVariableV1::CargoHome,
        AuthorityCargoEnvironmentVariableV1::CargoNetOffline,
        AuthorityCargoEnvironmentVariableV1::CargoTargetDir,
        AuthorityCargoEnvironmentVariableV1::Fe2o3Target,
        AuthorityCargoEnvironmentVariableV1::Home,
        AuthorityCargoEnvironmentVariableV1::Lang,
        AuthorityCargoEnvironmentVariableV1::LcAll,
        AuthorityCargoEnvironmentVariableV1::Tmpdir,
        AuthorityCargoEnvironmentVariableV1::Tz,
    ]
    .into_iter()
    .enumerate()
    {
        let mut missing = entries.clone();
        missing.remove(index);
        assert_eq!(
            from_entries(&missing, CACHE_IDENTITY),
            Err(AuthorityCargoEnvironmentErrorV1::MissingVariable { variable })
        );
    }

    let mut duplicate = entries.clone();
    duplicate[1] = entries[0].clone();
    assert_eq!(
        from_entries(&duplicate, CACHE_IDENTITY),
        Err(AuthorityCargoEnvironmentErrorV1::DuplicateVariable {
            name: "CARGO_HOME".to_owned(),
        })
    );

    let mut too_many = entries.clone();
    too_many.push((b"EXTRA".to_vec(), b"value".to_vec()));
    assert_eq!(
        from_entries(&too_many, CACHE_IDENTITY),
        Err(AuthorityCargoEnvironmentErrorV1::TooManyVariables { actual: 10 })
    );

    let mut unknown = entries.clone();
    unknown[0].0 = b"UNRELATED_SETTING".to_vec();
    assert_eq!(
        from_entries(&unknown, CACHE_IDENTITY),
        Err(AuthorityCargoEnvironmentErrorV1::UnknownVariable {
            name: "UNRELATED_SETTING".to_owned(),
        })
    );

    let mut non_utf8_name = entries.clone();
    non_utf8_name[0].0 = vec![0xff];
    assert_eq!(
        from_entries(&non_utf8_name, CACHE_IDENTITY),
        Err(AuthorityCargoEnvironmentErrorV1::NonUtf8VariableName)
    );

    let mut non_utf8_value = entries.clone();
    non_utf8_value[0].1 = vec![0xff];
    assert_eq!(
        from_entries(&non_utf8_value, CACHE_IDENTITY),
        Err(AuthorityCargoEnvironmentErrorV1::NonUtf8VariableValue {
            name: "CARGO_HOME".to_owned(),
        })
    );

    assert_eq!(
        from_entries(&entries, [0; 32]),
        Err(AuthorityCargoEnvironmentErrorV1::ZeroCargoCacheIdentity)
    );
}

#[test]
fn oversized_raw_values_fail_before_utf8_and_channel_classification() {
    let entries = golden_entries();
    let oversized = vec![0xff; AUTHORITY_CARGO_ENVIRONMENT_MAX_RAW_VALUE_LEN_V1 + 1];

    let mut invalid_utf8 = entries.clone();
    invalid_utf8[0].1 = oversized.clone();
    assert_eq!(
        from_entries(&invalid_utf8, CACHE_IDENTITY),
        Err(AuthorityCargoEnvironmentErrorV1::VariableValueTooLong {
            name: "CARGO_HOME".to_owned(),
            actual: AUTHORITY_CARGO_ENVIRONMENT_MAX_RAW_VALUE_LEN_V1 + 1,
        })
    );

    let mut forbidden = entries;
    forbidden[0] = (b"CARGO_ENCODED_RUSTFLAGS".to_vec(), oversized);
    assert_eq!(
        from_entries(&forbidden, CACHE_IDENTITY),
        Err(AuthorityCargoEnvironmentErrorV1::VariableValueTooLong {
            name: "CARGO_ENCODED_RUSTFLAGS".to_owned(),
            actual: AUTHORITY_CARGO_ENVIRONMENT_MAX_RAW_VALUE_LEN_V1 + 1,
        })
    );
}

#[test]
fn every_fixed_value_is_exact() {
    let entries = golden_entries();
    for (index, variable, replacement) in [
        (
            1,
            AuthorityCargoEnvironmentVariableV1::CargoNetOffline,
            "false",
        ),
        (
            3,
            AuthorityCargoEnvironmentVariableV1::Fe2o3Target,
            "gfx942:xnack+",
        ),
        (5, AuthorityCargoEnvironmentVariableV1::Lang, "C"),
        (6, AuthorityCargoEnvironmentVariableV1::LcAll, "en_US.UTF-8"),
        (8, AuthorityCargoEnvironmentVariableV1::Tz, "PST8PDT"),
    ] {
        let mut changed = entries.clone();
        changed[index].1 = replacement.as_bytes().to_vec();
        assert_eq!(
            from_entries(&changed, CACHE_IDENTITY),
            Err(AuthorityCargoEnvironmentErrorV1::InvalidFixedValue { variable })
        );
    }
}

#[test]
fn path_rejection_corpus_is_strict_and_bounded() {
    let entries = golden_entries();
    let exact_maximum = format!(
        "/{}",
        "a".repeat(AUTHORITY_CARGO_ENVIRONMENT_MAX_PATH_LEN_V1 - 1)
    );
    let long = format!(
        "/{}",
        "a".repeat(AUTHORITY_CARGO_ENVIRONMENT_MAX_PATH_LEN_V1)
    );
    let mut exact = entries.clone();
    exact[0].1 = exact_maximum.into_bytes();
    assert!(from_entries(&exact, CACHE_IDENTITY).is_ok());
    let cases = [
        ("", AuthorityCargoEnvironmentPathErrorV1::Empty),
        (
            "relative/path",
            AuthorityCargoEnvironmentPathErrorV1::Relative,
        ),
        (
            "/authority//cargo",
            AuthorityCargoEnvironmentPathErrorV1::NonCanonicalComponent,
        ),
        (
            "/authority/./cargo",
            AuthorityCargoEnvironmentPathErrorV1::NonCanonicalComponent,
        ),
        (
            "/authority/../cargo",
            AuthorityCargoEnvironmentPathErrorV1::NonCanonicalComponent,
        ),
        (
            "/authority/cargo/",
            AuthorityCargoEnvironmentPathErrorV1::NonCanonicalComponent,
        ),
        (
            "/authority/cargo home",
            AuthorityCargoEnvironmentPathErrorV1::NonPortableByte,
        ),
        (
            "/authority/cargo\\home",
            AuthorityCargoEnvironmentPathErrorV1::NonPortableByte,
        ),
        (
            "/authority/caf\u{e9}",
            AuthorityCargoEnvironmentPathErrorV1::NonPortableByte,
        ),
    ];
    for (value, reason) in cases {
        let mut changed = entries.clone();
        changed[0].1 = value.as_bytes().to_vec();
        assert_eq!(
            from_entries(&changed, CACHE_IDENTITY),
            Err(AuthorityCargoEnvironmentErrorV1::InvalidPath {
                variable: AuthorityCargoEnvironmentVariableV1::CargoHome,
                reason,
            }),
            "{value:?}"
        );
    }

    let mut changed = entries;
    changed[0].1 = long.as_bytes().to_vec();
    assert_eq!(
        from_entries(&changed, CACHE_IDENTITY),
        Err(AuthorityCargoEnvironmentErrorV1::InvalidPath {
            variable: AuthorityCargoEnvironmentVariableV1::CargoHome,
            reason: AuthorityCargoEnvironmentPathErrorV1::TooLong {
                actual: AUTHORITY_CARGO_ENVIRONMENT_MAX_PATH_LEN_V1 + 1,
            },
        })
    );
}

#[test]
fn lexical_paths_deliberately_do_not_claim_object_separation() {
    let mut entries = golden_entries();
    for index in [0, 2, 4, 7] {
        entries[index].1 = b"/authority/shared".to_vec();
    }
    assert!(from_entries(&entries, CACHE_IDENTITY).is_ok());

    entries[2].1 = b"/authority/shared/target".to_vec();
    entries[7].1 = b"/authority/shared/target/tmp".to_vec();
    assert!(from_entries(&entries, CACHE_IDENTITY).is_ok());
}

#[test]
fn forbidden_ambient_channel_corpus_is_classified() {
    let entries = golden_entries();
    let cases = [
        (
            "LD_LIBRARY_PATH",
            ForbiddenCargoEnvironmentChannelV1::DynamicLoader,
        ),
        (
            "LD_PRELOAD",
            ForbiddenCargoEnvironmentChannelV1::DynamicLoader,
        ),
        (
            "DYLD_INSERT_LIBRARIES",
            ForbiddenCargoEnvironmentChannelV1::DynamicLoader,
        ),
        (
            "RUSTFLAGS",
            ForbiddenCargoEnvironmentChannelV1::ToolOverride,
        ),
        (
            "RUSTDOCFLAGS",
            ForbiddenCargoEnvironmentChannelV1::ToolOverride,
        ),
        (
            "RUSTC_WRAPPER",
            ForbiddenCargoEnvironmentChannelV1::ToolOverride,
        ),
        (
            "RUSTC_WORKSPACE_WRAPPER",
            ForbiddenCargoEnvironmentChannelV1::ToolOverride,
        ),
        (
            "RUSTC_BOOTSTRAP",
            ForbiddenCargoEnvironmentChannelV1::ToolOverride,
        ),
        (
            "CARGO_ENCODED_RUSTFLAGS",
            ForbiddenCargoEnvironmentChannelV1::ToolOverride,
        ),
        (
            "CARGO_PROFILE_RELEASE_LTO",
            ForbiddenCargoEnvironmentChannelV1::ToolOverride,
        ),
        (
            "CARGO_BUILD_RUSTC",
            ForbiddenCargoEnvironmentChannelV1::ToolOverride,
        ),
        (
            "CARGO_TARGET_AMDGCN_LINKER",
            ForbiddenCargoEnvironmentChannelV1::ToolOverride,
        ),
        (
            "FE2O3_RUSTC",
            ForbiddenCargoEnvironmentChannelV1::ToolOverride,
        ),
        (
            "LIBRARY_PATH",
            ForbiddenCargoEnvironmentChannelV1::ToolOverride,
        ),
        ("CPATH", ForbiddenCargoEnvironmentChannelV1::ToolOverride),
        (
            "CPLUS_INCLUDE_PATH",
            ForbiddenCargoEnvironmentChannelV1::ToolOverride,
        ),
        (
            "PKG_CONFIG_PATH",
            ForbiddenCargoEnvironmentChannelV1::ToolOverride,
        ),
        (
            "NIX_LDFLAGS",
            ForbiddenCargoEnvironmentChannelV1::ToolOverride,
        ),
        (
            "BINDGEN_EXTRA_CLANG_ARGS",
            ForbiddenCargoEnvironmentChannelV1::ToolOverride,
        ),
        ("PATH", ForbiddenCargoEnvironmentChannelV1::ToolOverride),
        (
            "RUSTUP_TOOLCHAIN",
            ForbiddenCargoEnvironmentChannelV1::RustupSelection,
        ),
        (
            "RUSTUP_HOME",
            ForbiddenCargoEnvironmentChannelV1::RustupSelection,
        ),
        ("HTTPS_PROXY", ForbiddenCargoEnvironmentChannelV1::Network),
        ("NO_PROXY", ForbiddenCargoEnvironmentChannelV1::Network),
        (
            "CARGO_HTTP_PROXY",
            ForbiddenCargoEnvironmentChannelV1::Network,
        ),
        (
            "CARGO_NET_GIT_FETCH_WITH_CLI",
            ForbiddenCargoEnvironmentChannelV1::Network,
        ),
        (
            "CARGO_REGISTRIES_CRATES_IO_INDEX",
            ForbiddenCargoEnvironmentChannelV1::RegistryCredentialGitSsh,
        ),
        (
            "CARGO_CREDENTIAL_ALIAS",
            ForbiddenCargoEnvironmentChannelV1::RegistryCredentialGitSsh,
        ),
        (
            "GIT_CONFIG",
            ForbiddenCargoEnvironmentChannelV1::RegistryCredentialGitSsh,
        ),
        (
            "SSH_AUTH_SOCK",
            ForbiddenCargoEnvironmentChannelV1::RegistryCredentialGitSsh,
        ),
        (
            "EDITOR",
            ForbiddenCargoEnvironmentChannelV1::RegistryCredentialGitSsh,
        ),
        (
            "AWS_SECRET_ACCESS_KEY",
            ForbiddenCargoEnvironmentChannelV1::SecretLike,
        ),
        (
            "SERVICE_API_TOKEN",
            ForbiddenCargoEnvironmentChannelV1::SecretLike,
        ),
    ];
    for (name, channel) in cases {
        let mut changed = entries.clone();
        changed[0].0 = name.as_bytes().to_vec();
        assert_eq!(
            from_entries(&changed, CACHE_IDENTITY),
            Err(AuthorityCargoEnvironmentErrorV1::ForbiddenVariable {
                name: name.to_owned(),
                channel,
            }),
            "{name}"
        );
    }
}

#[test]
fn noncanonical_variable_names_fail_before_allowlist_matching() {
    let entries = golden_entries();
    for name in ["", "cargo_home", "9BAD", "BAD-NAME", "BAD NAME"] {
        let mut changed = entries.clone();
        changed[0].0 = name.as_bytes().to_vec();
        assert_eq!(
            from_entries(&changed, CACHE_IDENTITY),
            Err(AuthorityCargoEnvironmentErrorV1::NonCanonicalVariableName {
                name: name.to_owned(),
            })
        );
    }
}

#[test]
fn wire_header_and_bounds_rejection_corpus_fails_closed() {
    let wire = golden_environment().encode();
    for length in [0, 1, 31] {
        assert_eq!(
            decode_authority_cargo_environment_v1(&wire[..length]),
            Err(AuthorityCargoEnvironmentErrorV1::InvalidWireLength { actual: length })
        );
    }
    let oversized = vec![0; AUTHORITY_CARGO_ENVIRONMENT_MAX_WIRE_LEN_V1 + 1];
    assert_eq!(
        decode_authority_cargo_environment_v1(&oversized),
        Err(AuthorityCargoEnvironmentErrorV1::InvalidWireLength {
            actual: AUTHORITY_CARGO_ENVIRONMENT_MAX_WIRE_LEN_V1 + 1,
        })
    );

    let mut changed = wire.clone();
    changed[0] ^= 1;
    assert_eq!(
        decode_authority_cargo_environment_v1(&changed),
        Err(AuthorityCargoEnvironmentErrorV1::InvalidMagic)
    );

    let mut changed = wire.clone();
    set_u16(&mut changed, 8, 2);
    assert_eq!(
        decode_authority_cargo_environment_v1(&changed),
        Err(AuthorityCargoEnvironmentErrorV1::UnsupportedVersion { actual: 2 })
    );

    let mut changed = wire.clone();
    set_u16(&mut changed, 10, 31);
    assert_eq!(
        decode_authority_cargo_environment_v1(&changed),
        Err(AuthorityCargoEnvironmentErrorV1::InvalidHeaderLength { actual: 31 })
    );

    let mut changed = wire.clone();
    set_u16(&mut changed, 12, 8);
    assert_eq!(
        decode_authority_cargo_environment_v1(&changed),
        Err(AuthorityCargoEnvironmentErrorV1::InvalidEntryCount { actual: 8 })
    );

    for offset in [14, 15, 24, 25, 26, 27, 28, 29, 30, 31] {
        let mut changed = wire.clone();
        changed[offset] = 1;
        assert_eq!(
            decode_authority_cargo_environment_v1(&changed),
            Err(AuthorityCargoEnvironmentErrorV1::NonzeroHeaderReserved)
        );
    }

    let mut changed = wire.clone();
    changed[32..64].fill(0);
    assert_eq!(
        decode_authority_cargo_environment_v1(&changed),
        Err(AuthorityCargoEnvironmentErrorV1::ZeroCargoCacheIdentity)
    );

    let mut changed = wire.clone();
    set_u32(&mut changed, 16, 240);
    assert_eq!(
        decode_authority_cargo_environment_v1(&changed),
        Err(AuthorityCargoEnvironmentErrorV1::InvalidDeclaredLength { actual: 240 })
    );

    for mode in [0, 1, 2, 4, u32::MAX] {
        let mut changed = wire.clone();
        set_u32(&mut changed, 20, mode);
        assert_eq!(
            decode_authority_cargo_environment_v1(&changed),
            Err(AuthorityCargoEnvironmentErrorV1::InvalidMode { actual: mode })
        );
    }

    let mut trailing = wire;
    trailing.push(0);
    assert_eq!(
        decode_authority_cargo_environment_v1(&trailing),
        Err(AuthorityCargoEnvironmentErrorV1::InvalidDeclaredLength { actual: 273 })
    );
}

#[test]
fn malformed_duplicate_and_reordered_wire_entries_fail_closed() {
    let entries = golden_entries();

    let mut reordered = entries.clone();
    reordered.swap(0, 1);
    assert_eq!(
        decode_authority_cargo_environment_v1(&raw_wire(&reordered)),
        Err(AuthorityCargoEnvironmentErrorV1::NonCanonicalEntryOrder { index: 0 })
    );

    let mut duplicate = entries.clone();
    duplicate[1] = duplicate[0].clone();
    assert_eq!(
        decode_authority_cargo_environment_v1(&raw_wire(&duplicate)),
        Err(AuthorityCargoEnvironmentErrorV1::NonCanonicalEntryOrder { index: 1 })
    );

    let mut wire = raw_wire(&entries);
    set_u16(&mut wire, 64, 0);
    assert_eq!(
        decode_authority_cargo_environment_v1(&wire),
        Err(AuthorityCargoEnvironmentErrorV1::InvalidWireNameLength {
            index: 0,
            actual: 0,
        })
    );

    let mut wire = raw_wire(&entries);
    set_u16(
        &mut wire,
        66,
        (AUTHORITY_CARGO_ENVIRONMENT_MAX_PATH_LEN_V1 + 1) as u16,
    );
    assert_eq!(
        decode_authority_cargo_environment_v1(&wire),
        Err(AuthorityCargoEnvironmentErrorV1::InvalidWireValueLength {
            index: 0,
            actual: (AUTHORITY_CARGO_ENVIRONMENT_MAX_PATH_LEN_V1 + 1) as u16,
        })
    );

    let mut truncated = raw_wire(&entries);
    truncated.pop();
    let truncated_len = truncated.len() as u32;
    set_u32(&mut truncated, 16, truncated_len);
    assert_eq!(
        decode_authority_cargo_environment_v1(&truncated),
        Err(AuthorityCargoEnvironmentErrorV1::TruncatedEntry { index: 8 })
    );

    let mut non_utf8_value = entries;
    non_utf8_value[0].1[0] = 0xff;
    assert_eq!(
        decode_authority_cargo_environment_v1(&raw_wire(&non_utf8_value)),
        Err(AuthorityCargoEnvironmentErrorV1::NonUtf8VariableValue {
            name: "CARGO_HOME".to_owned(),
        })
    );
}
