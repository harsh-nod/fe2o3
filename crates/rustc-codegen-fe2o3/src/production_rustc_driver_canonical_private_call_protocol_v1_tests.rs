#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PrivateRequest {
    schema: u16,
    route: String,
    run_id: String,
    case: PrivateCase,
    target: Target,
    captured: [u8; 32],
    executed: [u8; 32],
    cwd: PathBuf,
    source: [FileStamp; 5],
}
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PrivateReport {
    request: PrivateRequest,
    callbacks: usize,
    result: Result<PrivateObservation, String>,
}
fn private_stamps() -> Result<[FileStamp; 5], String> {
    let base = workspace();
    [
        "Cargo.lock".into(),
        format!("{BASE}/Cargo.toml"),
        format!("{BASE}/src/lib.rs"),
        format!("{BASE}/src/private_call_history.rs"),
        format!("{BASE}/src/canonical_assertion_source.rs"),
    ]
    .into_iter()
    .map(|path: String| {
        let path = base.join(path).canonicalize().map_err(|e| e.to_string())?;
        let sha256 = digest(&std::fs::read(&path).map_err(|e| e.to_string())?);
        Ok(FileStamp { path, sha256 })
    })
    .collect::<Result<Vec<_>, String>>()?
    .try_into()
    .map_err(|_| "source roster".into())
}
fn private_executed(captured: &[String], case: PrivateCase) -> Result<Vec<String>, String> {
    require_canonical_overflow_checks_v1(captured).map_err(|e| e.to_string())?;
    require(
        !captured.iter().any(|arg| {
            arg.contains("fe2o3_private_call_")
                || arg.contains("fe2o3_canonical_assertion_")
                || arg.contains("fe2o3_canonical_scalar_")
                || arg.contains("mir-opt-level")
                || arg.contains("inline-mir")
        }),
        "preexisting case cfg or MIR override",
    )?;
    let mut args = captured.to_vec();
    for cfg in P_CFGS {
        args.push(format!("--check-cfg=cfg({cfg})"));
    }
    args.push(format!("--cfg={}", case.cfg()));
    if case.opt0() {
        args.push("-Zmir-opt-level=0".into());
    }
    Ok(args)
}
fn private_check_request(request: &PrivateRequest, invocation: &Invocation) -> Result<(), String> {
    require(
        request.schema == 1
            && request.route == P_ROUTE
            && !request.run_id.is_empty()
            && request.run_id.len() <= 256
            && request.source == private_stamps()?
            && request.cwd == env::current_dir().map_err(|e| e.to_string())?
            && request
                .source
                .iter()
                .map(|s| &s.path)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == 5
            && request.captured == args_hash(&invocation.captured)
            && request.executed == args_hash(&invocation.executed)
            && invocation.executed == private_executed(&invocation.captured, request.case)?,
        "exact source/cwd/route/argv request binding",
    )?;
    require(
        options(&invocation.captured, "-Ctarget-cpu") == [request.target.cpu()]
            && options(&invocation.captured, "--crate-name")
                == ["fe2o3_production_extraction_fixture"]
            && invocation
                .captured
                .iter()
                .filter(|arg| {
                    request.cwd.join(arg).canonicalize().ok()
                        == Some(request.source[2].path.clone())
                })
                .count()
                == 1,
        "captured target/crate/source entry",
    )
}
fn private_decode(
    status: Option<i32>,
    bytes: Option<&[u8]>,
    expected: &PrivateRequest,
) -> Result<PrivateObservation, String> {
    require(status == Some(0), "exact zero child exit required")?;
    let bytes = bytes.ok_or("missing fresh report")?;
    require(bytes.len() <= REPORT_CAP, "oversized report")?;
    let report: PrivateReport = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    require(
        report.request == *expected && report.callbacks == 1,
        "stale/foreign request or callback count",
    )?;
    let row = report.result?;
    private_validate(expected, &row)?;
    Ok(row)
}
fn private_audit_protocol(
    request: &PrivateRequest,
    invocation: &Invocation,
    report: &PrivateReport,
) {
    // Positive baseline comes only from this actual successful consuming child.
    private_check_request(request, invocation).unwrap();
    let bytes = serde_json::to_vec(report).unwrap();
    private_decode(Some(0), Some(&bytes), request).unwrap();
    for change in 0..11 {
        let mut changed = request.clone();
        match change {
            0..=4 => changed.source[change].sha256[0] ^= 1,
            5 => changed.route.push_str("-foreign"),
            6 => changed.schema += 1,
            7 => changed.run_id.clear(),
            8 => changed.cwd.push("foreign-cwd"),
            9 => changed.captured[0] ^= 1,
            _ => changed.executed[0] ^= 1,
        }
        assert!(
            private_check_request(&changed, invocation).is_err(),
            "request mutation {change}"
        );
    }
    let other = match request.target {
        Target::Gfx942 => Target::Gfx950,
        Target::Gfx950 => Target::Gfx942,
    };
    for change in 0..12 {
        let mut changed = serde_json::to_value(report).unwrap();
        match change {
            0 => {
                changed["request"]["run_id"] =
                    serde_json::json!(format!("{}-stale", request.run_id))
            }
            1 => changed["callbacks"] = serde_json::json!(0),
            2 => changed["callbacks"] = serde_json::json!(2),
            3 => changed["result"]["Ok"]["actual_target"] = serde_json::to_value(other).unwrap(),
            4 => changed["target_bound_stage"] = serde_json::json!(true),
            5 => changed["result"] = serde_json::json!({"Err": "genuine source refusal"}),
            6 => changed["result"]["Ok"]["checks"]["stages"] = serde_json::json!(0),
            7 => changed["result"]["Ok"]["checks"]["call_aliases"] = serde_json::json!(0),
            8 => changed["result"]["Ok"]["checks"]["pending"] = serde_json::json!(0),
            9 => changed["result"]["Ok"]["rounds"] = serde_json::json!([]),
            10 => changed["result"]["Ok"]["roots"] = serde_json::json!([]),
            _ => changed["result"]["Ok"]["final_storage"] = serde_json::json!(1),
        }
        assert!(
            private_decode(
                Some(0),
                Some(&serde_json::to_vec(&changed).unwrap()),
                request
            )
            .is_err(),
            "report mutation {change}"
        );
    }
    // Mutate only root membership or order in an actual accepted report.
    for change in 0..2 {
        let mut changed = serde_json::to_value(report).unwrap();
        let roots = changed["result"]["Ok"]["roots"].as_array_mut().unwrap();
        match change {
            0 => {
                roots.pop().unwrap();
            }
            _ => roots[0]["name"] = serde_json::json!("foreign_private_call_root"),
        }
        assert!(
            private_decode(
                Some(0),
                Some(&serde_json::to_vec(&changed).unwrap()),
                request
            )
            .is_err()
        );
    }
    if report.result.as_ref().unwrap().roots.len() > 1 {
        for change in 0..2 {
            let mut changed = serde_json::to_value(report).unwrap();
            let roots = changed["result"]["Ok"]["roots"].as_array_mut().unwrap();
            match change {
                0 => roots[1]["name"] = roots[0]["name"].clone(),
                _ => roots.swap(0, 1),
            }
            assert!(
                private_decode(
                    Some(0),
                    Some(&serde_json::to_vec(&changed).unwrap()),
                    request
                )
                .is_err()
            );
        }
    }
    private_decode(Some(0), Some(&bytes), request).unwrap();
    // Keep unrelated accepted predicates intact: these isolate the two wire
    // sums, which must reject rather than panic in debug or wrap in release.
    let mut overflow = serde_json::to_value(report).unwrap();
    overflow["result"]["Ok"]["checks"]["physical"] = serde_json::json!(0);
    overflow["result"]["Ok"]["checks"]["retained"] = serde_json::json!(usize::MAX);
    overflow["result"]["Ok"]["checks"]["removed"] = serde_json::json!(1);
    assert!(
        private_decode(
            Some(0),
            Some(&serde_json::to_vec(&overflow).unwrap()),
            request
        )
        .is_err()
    );
    if request.case.opt0() {
        let mut overflow = serde_json::to_value(report).unwrap();
        for graph in ["before", "after"] {
            overflow["result"]["Ok"][graph]["loads"] = serde_json::json!(usize::MAX);
            overflow["result"]["Ok"][graph]["stores"] = serde_json::json!(1);
        }
        overflow["result"]["Ok"]["checks"]["memory"][1] = serde_json::json!(0);
        assert!(
            private_decode(
                Some(0),
                Some(&serde_json::to_vec(&overflow).unwrap()),
                request
            )
            .is_err()
        );
    }
    private_decode(Some(0), Some(&bytes), request).unwrap();
    let mut changed = Invocation {
        captured: invocation.captured.clone(),
        executed: invocation.executed.clone(),
    };
    changed.executed.push("-Cdebuginfo=0".into());
    let mut changed_request = request.clone();
    changed_request.executed = args_hash(&changed.executed);
    assert!(private_check_request(&changed_request, &changed).is_err());
    for case in [
        PrivateCase::SharedPrivateOpt0,
        PrivateCase::CrossBlockPrivateOpt0,
        PrivateCase::NormalTypedCalls,
    ] {
        if case != request.case {
            let mut changed = request.clone();
            changed.case = case;
            assert!(private_check_request(&changed, invocation).is_err());
        }
    }
    for (option, value) in [
        ("--crate-name", "foreign_fixture"),
        ("-Ctarget-cpu", other.cpu()),
    ] {
        let mut captured = invocation.captured.clone();
        let n = captured
            .iter()
            .position(|a| a == option || a.starts_with(&format!("{option}=")))
            .unwrap();
        if captured[n] == option {
            captured[n + 1] = value.into();
        } else {
            captured[n] = format!("{option}={value}");
        }
        let changed = Invocation {
            executed: private_executed(&captured, request.case).unwrap(),
            captured,
        };
        let mut changed_request = request.clone();
        changed_request.captured = args_hash(&changed.captured);
        changed_request.executed = args_hash(&changed.executed);
        assert!(private_check_request(&changed_request, &changed).is_err());
    }
    for status in [None, Some(1), Some(101)] {
        assert!(private_decode(status, Some(&bytes), request).is_err());
    }
    assert!(private_decode(Some(0), None, request).is_err());
    assert!(private_decode(Some(0), Some(b"{"), request).is_err());
    let mut oversized = bytes.clone();
    oversized.resize(REPORT_CAP + 1, b' ');
    assert!(serde_json::from_slice::<PrivateReport>(&oversized).is_ok());
    assert!(private_decode(Some(0), Some(&oversized), request).is_err());
    private_decode(Some(0), Some(&bytes), request).unwrap();
}
#[test]
fn canonical_private_call_protocol_rejects_failure_and_invalid_argv() {
    // Negative wire sample only; never a manufactured successful source report.
    let invocation = Invocation {
        captured: vec![
            "rustc".into(),
            "-Coverflow-checks=on".into(),
            "-Ctarget-cpu=gfx942".into(),
            "--crate-name=fe2o3_production_extraction_fixture".into(),
            private_stamps().unwrap()[2].path.to_str().unwrap().into(),
        ],
        executed: Vec::new(),
    };
    let request = PrivateRequest {
        schema: 1,
        route: P_ROUTE.into(),
        run_id: "negative-wire-only".into(),
        case: PrivateCase::NormalTypedCalls,
        target: Target::Gfx942,
        captured: args_hash(&invocation.captured),
        executed: [0; 32],
        cwd: env::current_dir().unwrap(),
        source: private_stamps().unwrap(),
    };
    assert!(private_check_request(&request, &invocation).is_err());
    let report = PrivateReport {
        request: request.clone(),
        callbacks: 1,
        result: Err("producer refused".into()),
    };
    let bytes = serde_json::to_vec(&report).unwrap();
    for code in [None, Some(0), Some(1), Some(101)] {
        assert!(private_decode(code, Some(&bytes), &request).is_err());
    }
    assert!(private_decode(Some(0), None, &request).is_err());
}
#[test]
fn canonical_private_call_cases_have_explicit_cfg_and_mir_contracts() {
    assert_eq!(size_of::<PrivateChecks>(), 20 * size_of::<usize>());
    let captured = vec!["rustc".into(), "-Coverflow-checks=on".into()];
    for case in [
        PrivateCase::SharedPrivateOpt0,
        PrivateCase::CrossBlockPrivateOpt0,
        PrivateCase::NormalTypedCalls,
    ] {
        let args = private_executed(&captured, case).unwrap();
        assert_eq!(options(&args, "--cfg"), [case.cfg()]);
        assert_eq!(
            args.iter()
                .filter(|s| s.as_str() == "-Zmir-opt-level=0")
                .count(),
            usize::from(case.opt0())
        );
        for hostile in [
            "--cfg=fe2o3_private_call_shared",
            "-Zmir-opt-level=2",
            "-Zinline-mir=yes",
            "--cfg=fe2o3_canonical_assertion_retained",
        ] {
            let mut input = captured.clone();
            input.push(hostile.into());
            assert!(private_executed(&input, case).is_err());
        }
    }
}
