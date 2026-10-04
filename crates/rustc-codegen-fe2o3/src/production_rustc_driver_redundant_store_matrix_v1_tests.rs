//! Separate opt0 matrix; the original source smoke remains independently selectable.
pub(super) use super::super::simulation::constant_shift::Integer;
use super::*;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Target {
    Gfx942,
    Gfx950,
}
impl Target {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Gfx942 => "gfx942",
            Self::Gfx950 => "gfx950",
        }
    }
    pub(super) fn device(self) -> fe2o3_compiler_ffi::DeviceTargetV1 {
        fe2o3_compiler_ffi::DeviceTargetV1::parse(match self {
            Self::Gfx942 => "gfx942:xnack-",
            Self::Gfx950 => "gfx950:xnack-",
        })
        .unwrap()
    }
    pub(super) fn producer(self) -> &'static str {
        match self {
            Self::Gfx942 => "production-policy7-checked-gfx942-cov6-v1",
            Self::Gfx950 => "production-policy7-checked-gfx950-cov6-v1",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum Case {
    Smoke {},
    Matrix { integer: Integer, target: Target },
}
impl Case {
    pub(super) fn integer(self) -> Integer {
        match self {
            Self::Smoke {} => Integer::U32,
            Self::Matrix { integer, .. } => integer,
        }
    }
    pub(super) fn target(self) -> Target {
        match self {
            Self::Smoke {} => Target::Gfx942,
            Self::Matrix { target, .. } => target,
        }
    }
    pub(super) fn root(self) -> &'static str {
        match self {
            Self::Smoke {} => ROOT,
            Self::Matrix { .. } => "private_store_policy7_matrix",
        }
    }
    pub(super) fn fixture(self) -> &'static str {
        match self {
            Self::Smoke {} => "redundant_store_policy7.rs",
            Self::Matrix { .. } => "redundant_store_policy7_matrix.rs",
        }
    }
    pub(super) fn feature(self) -> &'static str {
        match self {
            Self::Smoke {} => "redundant-store-policy7",
            Self::Matrix { integer, .. } => match integer {
                Integer::I8 => "redundant-store-policy7-i8",
                Integer::U8 => "redundant-store-policy7-u8",
                Integer::I16 => "redundant-store-policy7-i16",
                Integer::U16 => "redundant-store-policy7-u16",
                Integer::I32 => "redundant-store-policy7-i32",
                Integer::U32 => "redundant-store-policy7-u32",
                Integer::I64 => "redundant-store-policy7-i64",
                Integer::U64 => "redundant-store-policy7-u64",
            },
        }
    }
    pub(super) fn label(self) -> String {
        match self {
            Self::Smoke {} => "redundant-store-policy7-u32-gfx942-opt0".into(),
            Self::Matrix { .. } => format!(
                "redundant-store-policy7-matrix-{}-{}-opt0",
                self.integer().name(),
                self.target().name()
            ),
        }
    }
}

fn cases() -> Vec<Case> {
    Integer::ALL
        .into_iter()
        .flat_map(|integer| {
            [Target::Gfx942, Target::Gfx950].map(|target| Case::Matrix { integer, target })
        })
        .collect()
}

#[test]
#[ignore = "strict fixed7 opt0 matrix: 16 actual mutation configurations and 32 compiler children"]
fn ordinary_rust_all_fixed_integer_private_stores_reach_actual_j_on_both_profiles() {
    let scratch = crate::test_temp_dir::TestTempDir::create("fe2o3-policy7-store-matrix");
    let mut completed = Vec::new();
    let mut children = 0;
    let mut scenarios = 0;
    for case in cases() {
        let directory = scratch.path().join(case.label());
        std::fs::create_dir(&directory).unwrap();
        let captured = capture(case, &directory, &scratch.path().join("target"));
        let observed = qualify(&captured, &directory, case);
        assert_eq!(observed.case, case);
        assert_eq!(observed.sim.scenarios.len(), 60);
        assert!(observed.deletions > 0);
        children += 2;
        scenarios += observed.sim.scenarios.len();
        completed.push(case);
        eprintln!(
            "POLICY7 MATRIX: {} qualified; {} actual I-to-J deletions; {} LLVM bytes; 60 SIM scenarios; 2 compiler children",
            case.label(),
            observed.deletions,
            observed.llvm_bytes
        );
    }
    assert_eq!(completed, cases());
    assert_eq!((completed.len(), children, scenarios), (16, 32, 960));
    eprintln!(
        "POLICY7 MATRIX TOTAL: 16 direct opt0 configurations; 16 roots; 32 compiler children; 960 SIM scenarios; 1920 deterministic executions"
    );
}

#[test]
fn policy7_matrix_cases_are_closed_distinct_and_bound_to_current_protocol() {
    let smoke = r#"{"kind":"smoke"}"#;
    assert_eq!(serde_json::to_string(&Case::Smoke {}).unwrap(), smoke);
    assert_eq!(serde_json::from_str::<Case>(smoke).unwrap(), Case::Smoke {});
    let cases = cases();
    assert_eq!(cases.len(), 16);
    let labels = cases
        .iter()
        .map(|case| case.label())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(labels.len(), 16);
    assert!(!cases.contains(&Case::Smoke {}));
    for case in cases {
        let encoded = serde_json::to_vec(&case).unwrap();
        assert_eq!(serde_json::from_slice::<Case>(&encoded).unwrap(), case);
        assert_eq!(case.root(), "private_store_policy7_matrix");
        assert!(case.feature().ends_with(case.integer().name()));
        assert_eq!(case.fixture(), "redundant_store_policy7_matrix.rs");
        let request = Request {
            case,
            mode: Mode::Extract,
            args_sha256: [1; 32],
            source: vec![],
        };
        let report = serde_json::to_vec(&Report {
            request: request.clone(),
            result: Ok(Outcome::Extracted {
                source: Source {
                    semantic: [2; 32],
                    roots: vec![],
                },
                llvm_sha256: [3; 32],
                llvm_bytes: 1,
            }),
        })
        .unwrap();
        assert!(decode(Some(0), Some(&report), &request).is_ok());
        assert!(
            decode(
                Some(0),
                Some(&report),
                &Request {
                    args_sha256: [9; 32],
                    ..request.clone()
                }
            )
            .is_err()
        );
        let mut missing_case = serde_json::to_value(&request).unwrap();
        missing_case.as_object_mut().unwrap().remove("case");
        assert!(serde_json::from_value::<Request>(missing_case).is_err());
        for other in [
            Case::Smoke {},
            Case::Matrix {
                integer: case.integer(),
                target: match case.target() {
                    Target::Gfx942 => Target::Gfx950,
                    Target::Gfx950 => Target::Gfx942,
                },
            },
            Case::Matrix {
                integer: if case.integer() == Integer::I8 {
                    Integer::U8
                } else {
                    Integer::I8
                },
                target: case.target(),
            },
        ] {
            assert_ne!(case, other);
            assert!(
                decode(
                    Some(0),
                    Some(&report),
                    &Request {
                        case: other,
                        ..request.clone()
                    }
                )
                .is_err()
            );
        }
    }
    for malformed in [
        r#"{"kind":"matrix","integer":"u128","target":"gfx942"}"#,
        r#"{"kind":"matrix","integer":"u32","target":"gfx900"}"#,
        r#"{"kind":"matrix","integer":"u32"}"#,
        r#"{"kind":"matrix","integer":"u32","target":"gfx942","extra":true}"#,
        r#"{"kind":"smoke","integer":"i8"}"#,
        r#"{"kind":"smoke","target":"gfx950"}"#,
        r#"{"kind":"smoke","kind":"matrix"}"#,
        r#"{"kind":"discovery"}"#,
    ] {
        assert!(
            serde_json::from_str::<Case>(malformed).is_err(),
            "{malformed}"
        );
    }
}

#[test]
fn policy7_matrix_fixture_wiring_preserves_smoke_and_exact_features() {
    let manifest = include_str!("../tests/fixtures/production-extraction-device/Cargo.toml");
    let lib = include_str!("../tests/fixtures/production-extraction-device/src/lib.rs");
    let fixture = include_str!(
        "../tests/fixtures/production-extraction-device/src/redundant_store_policy7_matrix.rs"
    );
    let check = |manifest: &str, lib: &str, fixture: &str| {
        lib.matches("mod redundant_store_policy7_matrix;").count() == 1
            && lib.matches("mod redundant_store_policy7;").count() == 1
            && Integer::ALL.into_iter().all(|integer| {
                let case = Case::Matrix {
                    integer,
                    target: Target::Gfx942,
                };
                let feature = format!("feature = \"{}\"", case.feature());
                manifest
                    .matches(&format!("{} = []", case.feature()))
                    .count()
                    == 1
                    && lib.matches(&feature).count() == 2
                    && lib.split_once("#[cfg(not(any(").is_some_and(|(_, rest)| {
                        rest.split_once(")))]")
                            .is_some_and(|(list, _)| list.matches(&feature).count() == 1)
                    })
                    && fixture
                        .matches(&format!(
                            "#[cfg({feature})]\ninstantiate!({});",
                            integer.name()
                        ))
                        .count()
                        == 1
            })
    };
    assert!(check(manifest, lib, fixture));
    assert!(!check(
        manifest,
        &lib.replace("mod redundant_store_policy7_matrix;", ""),
        fixture
    ));
    assert!(!check(
        manifest,
        &lib.replace("    feature = \"redundant-store-policy7-i8\",\n", ""),
        fixture
    ));
    assert!(!check(
        &manifest.replace("redundant-store-policy7-u64 = []", ""),
        lib,
        fixture
    ));
    assert!(!check(
        manifest,
        lib,
        &fixture.replace("instantiate!(i32);", "")
    ));
}
