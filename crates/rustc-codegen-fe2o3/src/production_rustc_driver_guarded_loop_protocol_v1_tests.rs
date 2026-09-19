use super::*;

fn request() -> Envelope {
    Envelope {
        protocol: PROTOCOL,
        capture: Request {
            case: Case::Exact,
            target: "gfx942".into(),
            args_sha256: [1; 32],
            source: vec![Stamp {
                path: PathBuf::from("fixture"),
                sha256: [2; 32],
            }],
        },
    }
}
fn observation() -> Observed {
    Observed {
        semantic: [3; 32],
        n: [4; 32],
        n_len: 101,
        roots: vec![7],
        rows: vec![ObservedRoot {
            root: 7,
            body: 9,
            root_identity: [5; 32],
            body_identity: [6; 32],
            report_semantic: [3; 32],
            ranked_sha256: [7; 32],
            ranked_len: 103,
            certificates: 1,
            checked_additions: 1,
            consumed: Consumed::SourceAndN {
                function: 9,
                block: 3,
                statement: 2,
                certificate: 0,
                dynamic_u32_bound: true,
            },
        }],
        constructed_work: 17,
        replay_work: 11,
        stage_receipt: 1000,
        unrelated_floor: FLOOR,
        live_floor: FLOOR + 1000,
        undercut_refused_before_work: true,
    }
}
fn response() -> Response {
    Response {
        protocol: PROTOCOL,
        request: request(),
        result: Ok(observation()),
    }
}

#[test]
fn guarded_protocol_requires_exact_success_and_never_accepts_refusal_as_positive() {
    // Pure protocol controls, not source/guard/projection evidence.
    let request = request();
    let bytes = serde_json::to_vec(&response()).unwrap();
    decode_owned(Some(0), Some(&bytes), &request).unwrap();
    for status in [None, Some(1), Some(101), Some(9)] {
        assert!(decode_owned(status, Some(&bytes), &request).is_err());
    }
    for bytes in [None, Some(b"{}".as_slice()), Some(b"not-json".as_slice())] {
        assert!(decode_owned(Some(0), bytes, &request).is_err());
    }
    let refused = serde_json::to_vec(&Response {
        protocol: PROTOCOL,
        request: request.clone(),
        result: Err(fail(Stage::Ranked, "actual guard mapping refused")),
    })
    .unwrap();
    assert!(decode_owned(Some(0), Some(&refused), &request).is_err());
    for mode in 0..3 {
        let mut foreign = request.clone();
        match mode {
            0 => foreign.capture.target = "gfx950".into(),
            1 => foreign.capture.args_sha256 = [8; 32],
            _ => foreign.capture.source[0].sha256 = [9; 32],
        }
        assert!(decode_owned(Some(0), Some(&bytes), &foreign).is_err());
    }
}

#[test]
fn malformed_new_envelope_and_both_request_modes_never_select_historical_fallback() {
    let valid = serde_json::to_string(&request()).unwrap();
    parse_envelope(&valid, false).unwrap();
    assert!(parse_envelope(&valid, true).is_err());
    for raw in ["", "not-json", "{}", "null"] {
        for old_present in [false, true] {
            assert!(parse_envelope(raw, old_present).is_err());
        }
    }
    let value = serde_json::to_value(request()).unwrap();
    for mode in 0..8 {
        let mut changed = value.clone();
        match mode {
            0 => {
                changed.as_object_mut().unwrap().remove("protocol");
            }
            1 => changed["protocol"] = serde_json::json!("StrictGuardedOriginalN"),
            2 => changed["protocol"] = serde_json::json!({"Historical": {}}),
            3 => changed["protocol"] = serde_json::json!({"StrictGuardedOriginalN": {"extra": 1}}),
            4 => changed["extra"] = serde_json::json!(1),
            5 => changed["capture"]["extra"] = serde_json::json!(1),
            6 => changed["capture"]["source"][0]["extra"] = serde_json::json!(1),
            _ => changed["capture"]["case"] = serde_json::json!({"Exact": {"extra": 1}}),
        }
        assert!(
            parse_envelope(&changed.to_string(), false).is_err(),
            "mode {mode}"
        );
        assert!(
            parse_envelope(&changed.to_string(), true).is_err(),
            "mode {mode}"
        );
    }
}

#[test]
fn old_and_new_request_and_report_schemas_cannot_cross_decode() {
    let request = request();
    let new_request = serde_json::to_vec(&request).unwrap();
    assert!(serde_json::from_slice::<Request>(&new_request).is_err());
    assert!(parse_envelope(&serde_json::to_string(&request.capture).unwrap(), false).is_err());
    let new_report = serde_json::to_vec(&response()).unwrap();
    assert!(super::super::decode(Some(0), Some(&new_report), &request.capture).is_err());
    let old_report = serde_json::to_vec(&super::super::Report {
        request: request.capture.clone(),
        result: Err(fail(Stage::Guard, "old protocol diagnostic")),
    })
    .unwrap();
    assert!(serde_json::from_slice::<Response>(&old_report).is_err());
    assert!(decode_owned(Some(0), Some(&old_report), &request).is_err());
}

#[test]
fn complete_new_response_and_nested_outcomes_reject_unknown_fields_and_tags() {
    let value = serde_json::to_value(response()).unwrap();
    for mode in 0..10 {
        let mut changed = value.clone();
        match mode {
            0 => changed["extra"] = serde_json::json!(1),
            1 => {
                changed.as_object_mut().unwrap().remove("protocol");
            }
            2 => changed["protocol"] = serde_json::json!({"StrictGuardedOriginalN": {"extra": 1}}),
            3 => changed["request"]["extra"] = serde_json::json!(1),
            4 => changed["result"]["Ok"]["extra"] = serde_json::json!(1),
            5 => changed["result"]["Ok"]["rows"][0]["extra"] = serde_json::json!(1),
            6 => {
                changed["result"]["Ok"]["rows"][0]["consumed"]["SourceAndN"]["extra"] =
                    serde_json::json!(1)
            }
            7 => {
                changed["result"]["Ok"]["rows"][0]["consumed"] =
                    serde_json::json!({"NoU32Credit": {"extra": 1}})
            }
            8 => changed["result"]["Ok"]["rows"][0]["consumed"] = serde_json::json!("NoU32Credit"),
            _ => {
                changed["result"]["Ok"]["rows"][0]["consumed"] =
                    serde_json::json!({"Unsupported": {}})
            }
        }
        assert!(
            serde_json::from_value::<Response>(changed).is_err(),
            "mode {mode}"
        );
    }
    let mut failure = serde_json::to_value(Response {
        protocol: PROTOCOL,
        request: request(),
        result: Err(fail(Stage::Guard, "refused")),
    })
    .unwrap();
    failure["result"]["Err"]["extra"] = serde_json::json!(1);
    assert!(serde_json::from_value::<Response>(failure).is_err());
}

#[test]
fn observation_requires_full_owner_roster_source_progress_and_inherited_floor() {
    let request = request();
    let original = observation();
    validate_owned(&request, &original).unwrap();
    for mode in 0..16 {
        let mut changed = original.clone();
        match mode {
            0 => changed.semantic = [0; 32],
            1 => changed.n = [0; 32],
            2 => changed.n_len = 0,
            3 => changed.replay_work = 0,
            4 => changed.constructed_work = 0,
            5 => changed.live_floor -= 1,
            6 => changed.unrelated_floor = 0,
            7 => changed.undercut_refused_before_work = false,
            8 => changed.roots.push(7),
            9 => changed.rows[0].root = 8,
            10 => changed.rows[0].report_semantic = [8; 32],
            11 => changed.rows[0].ranked_len = 0,
            12 => changed.rows[0].certificates = 0,
            13 => changed.rows[0].consumed = Consumed::NoU32Credit {},
            14 => {
                changed.rows[0].consumed = Consumed::SourceAndN {
                    function: 10,
                    block: 3,
                    statement: 2,
                    certificate: 0,
                    dynamic_u32_bound: true,
                }
            }
            _ => {
                changed.rows[0].consumed = Consumed::SourceAndN {
                    function: 9,
                    block: 3,
                    statement: 2,
                    certificate: 0,
                    dynamic_u32_bound: false,
                }
            }
        }
        assert!(validate_owned(&request, &changed).is_err(), "mode {mode}");
    }
}

#[test]
fn root_order_is_actual_not_lexical_and_u64_never_receives_u32_credit() {
    let mut request = request();
    let mut observed = observation();
    request.capture.case = Case::U64;
    assert!(validate_owned(&request, &observed).is_err());
    observed.rows[0].certificates = 0;
    observed.rows[0].consumed = Consumed::NoU32Credit {};
    validate_owned(&request, &observed).unwrap();
    request.capture.case = Case::Exact;
    assert!(validate_owned(&request, &observed).is_err());
    request.capture.case = Case::Multi;
    let mut observed = observation();
    let mut second = observed.rows[0].clone();
    second.root = 8;
    observed.rows.insert(0, second);
    observed.roots.insert(0, 8);
    validate_owned(&request, &observed).unwrap();
    observed.rows.swap(0, 1);
    assert!(validate_owned(&request, &observed).is_err());
}
