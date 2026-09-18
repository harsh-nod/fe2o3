use super::*;
use simulation::constant_shift::{Batch, Integer, ROOTS};

pub(super) struct Config {
    pub(super) batch: Batch,
    pub(super) dynamic: bool,
}

impl Config {
    pub(super) fn name(&self) -> String {
        if self.dynamic {
            "dynamic-shift-refusal".into()
        } else {
            self.batch.name().into()
        }
    }

    pub(super) fn roots(&self) -> &'static [&'static str] {
        if self.dynamic {
            &["dynamic_shift"]
        } else {
            &ROOTS
        }
    }

    pub(super) fn configure(&self, args: &mut Vec<String>) {
        args.push(format!(
            "--cfg=feature=\"constant-shift-{}\"",
            self.batch.integer.name()
        ));
        if self.dynamic {
            args.push("--cfg=feature=\"constant-shift-dynamic\"".into());
            // Isolate the still-closed dynamic shift grammar from the frontend
            // overflow Assert. This is not a positive or a policy override.
            args.push("-Coverflow-checks=off".into());
        } else if self.batch.retained {
            args.push("--cfg=feature=\"constant-shift-retained\"".into());
            args.push("-Zinline-mir=no".into());
        }
    }

    pub(super) fn check_refusal(&self, result: Result<Observation, SourceFailure>) {
        assert!(self.dynamic);
        let error = result.expect_err("dynamic count acquired checked-output admission");
        require_dynamic_source_refusal(&error).unwrap();
    }

    pub(super) fn check_root_roster(&self, roots: &[String]) -> Result<(), SourceFailure> {
        let expected = self.roots();
        if roots.len() != expected.len() {
            return Err(SourceFailure::new(
                SourceStage::Observation,
                "shift exact root count",
            ));
        }
        let mut unique = std::collections::BTreeSet::new();
        for root in roots {
            if !unique.insert(root.as_str()) || !expected.contains(&root.as_str()) {
                return Err(SourceFailure::new(
                    SourceStage::Observation,
                    "shift duplicate or foreign root identity",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn check(&self, observed: &Observation) {
        assert!(!self.dynamic);
        self.check_root_roster(&observed.roots).unwrap();
        assert_eq!(
            dispatch::check_private_helper_route(observed, false).unwrap(),
            dispatch::Route::DirectRawEmpty
        );
        let report = observed
            .constant_shift
            .as_ref()
            .expect("missing shift endpoint observations");
        assert_eq!(report.batch, self.batch);
        assert_eq!(report.output_digest, observed.output_digest);
        assert_eq!(report.roots.len(), ROOTS.len());
        assert_ne!(report.source_digest, [0; 32]);
        assert_ne!(report.original_digest, [0; 32]);
        for (row, expected) in report.roots.iter().zip(ROOTS) {
            assert_eq!(row.root, expected);
            assert_ne!(row.source_function, [0; 32]);
            assert_ne!(row.source_binding, [0; 32]);
            assert!(!row.original_entry.is_empty());
            assert!(!row.original_operation_owner.is_empty());
            assert!(!row.entry.is_empty());
            assert!(!row.operation_owner.is_empty());
            assert!(!row.native_symbol.is_empty());
        }
        let helpers = if self.batch.retained { ROOTS.len() } else { 0 };
        assert_eq!(
            (observed.internal_helpers, observed.helper_calls),
            (helpers, helpers)
        );
        eprintln!(
            "ordinary shift {}: 6 exact roots; retained-MIR test knob={}; original N={:02x?}; actual O={:02x?}",
            self.name(),
            self.batch.retained,
            report.original_digest,
            report.output_digest
        );
    }
}

fn require_dynamic_source_refusal(error: &SourceFailure) -> Result<(), String> {
    if error.stage != SourceStage::Policy4
        || !error.detail.contains("Unsupported { phase: \"source\", detail: \"total scalar/global recipe; no unchecked arithmetic\" }")
    {
        return Err(format!("wrong dynamic-count source refusal: {error:?}"));
    }
    Ok(())
}

fn decode_dynamic_refusal<T: serde::de::DeserializeOwned>(
    exit_code: Option<i32>,
    report: Option<&[u8]>,
) -> Result<SourceFailure, String> {
    if exit_code != Some(101) {
        return Err(format!(
            "expected refused libtest child exit 101, got {exit_code:?}"
        ));
    }
    let report = report.ok_or("expected refused child report is missing or unreadable")?;
    let result: Result<T, SourceFailure> = serde_json::from_slice(report)
        .map_err(|error| format!("invalid refused child report: {error}"))?;
    let error = match result {
        Err(error) => error,
        Ok(_) => return Err("refused child reported successful admission".into()),
    };
    require_dynamic_source_refusal(&error)?;
    Ok(error)
}

pub(super) fn expected_refusal_child(
    command: &mut Command,
    response: &Path,
) -> (std::process::Output, Result<Observation, SourceFailure>) {
    assert!(
        !response.exists(),
        "refusal report must be fresh: {response:?}"
    );
    let child = command
        .output()
        .expect("execute expected source-refusal child");
    let report = std::fs::read(response);
    let error = decode_dynamic_refusal::<Observation>(child.status.code(), report.as_deref().ok())
        .unwrap_or_else(|error| {
            panic!(
                "{error}\n{command:?}\n{}\n{}",
                String::from_utf8_lossy(&child.stdout),
                String::from_utf8_lossy(&child.stderr)
            )
        });
    (child, Err(error))
}

#[test]
fn literal_dynamic_refusal_requires_exact_child_status_report_and_source_boundary() {
    let report = |stage, detail: &str| {
        serde_json::to_vec(&Err::<(), _>(SourceFailure::new(stage, detail))).unwrap()
    };
    let detail = "Unsupported { phase: \"source\", detail: \"total scalar/global recipe; no unchecked arithmetic\" }";
    let exact = report(SourceStage::Policy4, detail);
    let admitted = serde_json::to_vec(&Ok::<(), SourceFailure>(())).unwrap();
    let observed = decode_dynamic_refusal::<()>(Some(101), Some(&exact)).unwrap();
    assert_eq!(observed.stage, SourceStage::Policy4);
    assert_eq!(observed.detail, detail);
    for status in [None, Some(0), Some(1), Some(134), Some(137)] {
        assert!(decode_dynamic_refusal::<()>(status, Some(&exact)).is_err());
    }
    assert!(decode_dynamic_refusal::<()>(Some(101), None).is_err());
    for invalid in [&b"not-json"[..], &b"{}"[..], admitted.as_slice()] {
        assert!(decode_dynamic_refusal::<()>(Some(101), Some(invalid)).is_err());
    }
    for stage in [
        SourceStage::Rustc,
        SourceStage::SourceCollection,
        SourceStage::Simulation,
    ] {
        assert!(decode_dynamic_refusal::<()>(Some(101), Some(&report(stage, detail))).is_err());
    }
    assert!(
        decode_dynamic_refusal::<()>(
            Some(101),
            Some(&report(SourceStage::Policy4, "a different source gate"))
        )
        .is_err()
    );
    let config = Config {
        batch: Batch {
            integer: Integer::U32,
            retained: false,
        },
        dynamic: true,
    };
    config.check_refusal(Err(observed));
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct RootObservation {
    root: String,
    source_function: [u8; 32],
    source_binding: [u8; 32],
    original_entry: String,
    original_operation_owner: String,
    entry: String,
    operation_owner: String,
    native_symbol: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct ShiftObservation {
    batch: Batch,
    source_digest: [u8; 32],
    original_digest: [u8; 32],
    output_digest: [u8; 32],
    roots: Vec<RootObservation>,
}

pub(super) fn check_actual(
    stage: &dispatch::Stage,
    requested: Option<simulation::Case>,
) -> Result<Option<ShiftObservation>, SourceFailure> {
    let Some(simulation::Case::ConstantShift(batch)) = requested else {
        return Ok(None);
    };
    let dispatch::Stage::Direct(direct) = stage else {
        return Err(SourceFailure::new(
            SourceStage::Observation,
            "scalar shifts require original RawEmpty custody",
        ));
    };
    let source = direct.output().source_semantic_kir();
    let semantic = stage.semantic();
    assert!(std::ptr::eq(semantic, source.semantic().semantic()));
    let source_digest = *semantic.semantic_sha256().as_bytes();
    assert_eq!(
        <[u8; 32]>::from(Sha256::digest(semantic.canonical_encoding())),
        source_digest
    );
    assert_eq!(semantic.roots().len(), ROOTS.len());
    let semantic_roots = semantic
        .roots()
        .iter()
        .map(|root| {
            let function = &semantic.functions()[root.index() as usize];
            let entry = function.kernel_entry().unwrap();
            (
                std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
                (
                    *function.identity().as_bytes(),
                    *entry.kernel_binding_identity().as_bytes(),
                ),
            )
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    assert_eq!(
        semantic_roots
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>(),
        ROOTS.into_iter().collect::<std::collections::BTreeSet<_>>()
    );
    let output = stage.output().module();
    simulation::constant_shift::check_preserved_root_order(source.module(), output)?;
    let original_owners = simulation::constant_shift::check_graph(source.module(), batch)?;
    let owners = simulation::constant_shift::check_graph(output, batch)?;
    let roots = ROOTS
        .into_iter()
        .map(|root| {
            let kernel = simulation::constant_shift::kernel_for_root(output, root)?;
            let original = simulation::constant_shift::kernel_for_root(source.module(), root)?;
            let owner = owners[root].clone();
            Ok(RootObservation {
                root: root.into(),
                source_function: semantic_roots[root].0,
                source_binding: semantic_roots[root].1,
                original_entry: original.entry.as_str().into(),
                original_operation_owner: original_owners[root].clone(),
                entry: kernel.entry.as_str().into(),
                native_symbol: if batch.retained {
                    owner.clone()
                } else {
                    kernel.id.as_str().into()
                },
                operation_owner: owner,
            })
        })
        .collect::<Result<Vec<_>, SourceFailure>>()?;
    Ok(Some(ShiftObservation {
        batch,
        source_digest,
        original_digest: *source.canonical_kernel_ir_identity().digest(),
        output_digest: *stage.output().canonical().identity().digest(),
        roots,
    }))
}

pub(super) fn native_body<'a>(llvm: &'a str, symbol: &str) -> Result<&'a str, SourceFailure> {
    let marker = format!("@{symbol}(");
    let matches = llvm
        .match_indices("define ")
        .filter(|(start, _)| {
            llvm[*start..]
                .lines()
                .next()
                .is_some_and(|line| line.contains(&marker))
        })
        .collect::<Vec<_>>();
    let [(start, _)] = matches.as_slice() else {
        return Err(SourceFailure::new(
            SourceStage::NativeHandoff,
            "shift exact actual-O native function missing or duplicate",
        ));
    };
    let body = &llvm[*start..];
    let end = body.find("\n}").ok_or_else(|| {
        SourceFailure::new(
            SourceStage::NativeHandoff,
            "shift native function not closed",
        )
    })?;
    Ok(&body[..end])
}

fn check_native_body(batch: Batch, ordinal: usize, body: &str) -> Result<(), SourceFailure> {
    // Numeric RHS identity is checked in actual N/O and by the production
    // native handoff. This text observation checks opcode/type/flags per owner.
    let (right, count) = simulation::constant_shift::root_case(batch.integer, ordinal);
    let opcode = if !right {
        "shl"
    } else if batch.integer.signed() {
        "ashr"
    } else {
        "lshr"
    };
    let expected = format!("{opcode} i{} ", batch.integer.width());
    let instructions = body
        .lines()
        .filter_map(|line| line.split_once(" = ").map(|(_, rhs)| rhs))
        .filter(|rhs| {
            ["shl ", "ashr ", "lshr "]
                .iter()
                .any(|opcode| rhs.starts_with(opcode))
        })
        .collect::<Vec<_>>();
    if instructions.len() > 1
        || (count != 0 && instructions.len() != 1)
        || instructions.iter().any(|rhs| !rhs.starts_with(&expected))
    {
        return Err(SourceFailure::new(
            SourceStage::NativeHandoff,
            "shift native sign, width, instruction roster or no-wrap flags differ",
        ));
    }
    Ok(())
}

pub(super) fn check_native(
    report: Option<&ShiftObservation>,
    llvm: &str,
) -> Result<(), SourceFailure> {
    let Some(report) = report else {
        return Ok(());
    };
    for (ordinal, row) in report.roots.iter().enumerate() {
        let body = native_body(llvm, &row.native_symbol)?;
        check_native_body(report.batch, ordinal, body)?;
        if report.batch.retained {
            let entry = native_body(llvm, &row.root)?;
            if !entry.contains(&format!("@{}(", row.native_symbol)) {
                return Err(SourceFailure::new(
                    SourceStage::NativeHandoff,
                    "shift native root lost its exact actual-O helper call",
                ));
            }
        }
    }
    Ok(())
}

const PROFILES: [fe2o3_amd_target::ProductionAmdTargetProfileV1; 2] = [
    fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx942,
    fe2o3_amd_target::ProductionAmdTargetProfileV1::Gfx950,
];

fn cases() -> Vec<OrdinarySourceCase> {
    let mut cases = Vec::new();
    for integer in Integer::ALL {
        for retained in [false, true] {
            cases.push(OrdinarySourceCase::ConstantShift(Config {
                batch: Batch { integer, retained },
                dynamic: false,
            }));
        }
    }
    cases.push(OrdinarySourceCase::ConstantShift(Config {
        batch: Batch {
            integer: Integer::U32,
            retained: false,
        },
        dynamic: true,
    }));
    cases
}

#[test]
fn shift_source_matrix_has_32_positive_configurations_192_roots_and_two_dynamic_controls() {
    let mut positive = std::collections::BTreeSet::new();
    let mut negative = 0;
    for profile in PROFILES {
        for case in cases() {
            let OrdinarySourceCase::ConstantShift(config) = case else {
                unreachable!()
            };
            if config.dynamic {
                negative += 1;
            } else {
                assert!(positive.insert((profile.cpu(), config.name())));
            }
        }
    }
    assert_eq!(
        (positive.len(), positive.len() * ROOTS.len(), negative),
        (32, 192, 2)
    );
}

#[test]
fn shift_source_root_roster_accepts_identity_order_and_rejects_duplicates_missing_and_foreign() {
    let config = Config {
        batch: Batch {
            integer: Integer::I8,
            retained: false,
        },
        dynamic: false,
    };
    let mut roots = ROOTS.into_iter().map(str::to_owned).collect::<Vec<_>>();
    roots.reverse();
    config.check_root_roster(&roots).unwrap();
    let mut missing = roots.clone();
    missing.pop();
    assert!(config.check_root_roster(&missing).is_err());
    let mut duplicate = roots.clone();
    duplicate[1] = duplicate[0].clone();
    assert!(config.check_root_roster(&duplicate).is_err());
    roots[0] = "foreign_root".into();
    assert!(config.check_root_roster(&roots).is_err());
}

#[test]
fn shift_native_oracle_rejects_wrong_sign_width_flags_and_foreign_function() {
    for integer in Integer::ALL {
        let batch = Batch {
            integer,
            retained: false,
        };
        let right = if integer.signed() { "ashr" } else { "lshr" };
        let exact = format!("%x = {right} i{} %value, 1", integer.width());
        check_native_body(batch, 3, &exact).unwrap();
        for bad in [
            exact.replace(right, "shl"),
            exact.replace(right, if integer.signed() { "lshr" } else { "ashr" }),
            exact.replace(&format!("i{} ", integer.width()), "i128 "),
            exact.replace(right, &format!("{right} exact")),
            String::new(),
        ] {
            assert!(check_native_body(batch, 3, &bad).is_err());
        }
        check_native_body(batch, 2, "ret void").unwrap();
    }
    assert!(native_body("define void @other() {\n}\n", "expected").is_err());
    assert!(native_body("define void @x() {\n}\ndefine void @x() {\n}\n", "x").is_err());
}

#[test]
#[ignore = "requires pinned rust-src, AMD dependencies, constant-shift admission and ordinary-source compilation"]
fn ordinary_rust_constant_shifts_reach_actual_o_and_cpu_oracle_both_profiles() {
    let cases = cases();
    for profile in PROFILES {
        ordinary_rust_checked_output_cases_for_profile(&cases, profile);
    }
}
