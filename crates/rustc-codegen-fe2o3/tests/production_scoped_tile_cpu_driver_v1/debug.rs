use super::*;
use std::fs::File;
use std::io::Write;

struct Session {
    job: process::Job,
    requests: File,
    revision: u64,
    count: u64,
    configuration: Option<String>,
}

impl Session {
    fn ask(&mut self, operation: &str, mut arguments: Value, unavailable: bool) -> Value {
        self.count += 1;
        let fields = arguments.as_object_mut().unwrap();
        fields.insert("schema".into(), json!("fe2o3-debug-request-v1"));
        fields.insert("request_id".into(), json!(self.count));
        fields.insert("expected_revision".into(), json!(self.revision));
        fields.insert("operation".into(), json!(operation));
        serde_json::to_writer(&mut self.requests, &arguments).unwrap();
        self.requests.write_all(b"\n").unwrap();
        self.job.send(&arguments).expect("bounded debugger request");
        let reply = self.job.reply().expect("bounded debugger response");
        assert_eq!(reply["schema"], "fe2o3-debug-response-v1");
        assert_eq!(reply["request_id"], self.count);
        assert_eq!(reply["operation"], operation);
        assert_eq!(
            reply["status"],
            if unavailable { "unavailable" } else { "ok" }
        );
        let session = &reply["session"];
        assert_eq!(session["backend"], "cpu_kir_simulator");
        assert_eq!(session["execution_kind"], "cpu_kir_simulation");
        assert_eq!(session["simulated"], true);
        assert_eq!(session["hardware_observed"], false);
        assert_eq!(session["performance_prediction"], false);
        let configuration = session["configuration_identity"].as_str().unwrap();
        assert_eq!(configuration.len(), 64);
        assert!(configuration != "0".repeat(64));
        assert!(
            configuration
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        );
        if let Some(expected) = &self.configuration {
            assert_eq!(configuration, expected);
        } else {
            self.configuration = Some(configuration.to_owned());
        }
        let revision = session["revision"].as_u64().unwrap();
        if unavailable {
            assert_eq!(revision, self.revision);
            assert_eq!(reply["unavailable"]["state_changed"], false);
        } else {
            assert!(revision >= self.revision);
            self.revision = revision;
        }
        reply
    }

    fn ok(&mut self, operation: &str, arguments: Value) -> Value {
        self.ask(operation, arguments, false)["result"].take()
    }

    fn memory(&mut self, allocation: &Value, length: usize) -> Value {
        let response = self.ok(
            "read_memory",
            json!({
                "allocation":allocation,"byte_offset":0,"byte_len":length
            }),
        );
        assert_eq!(response["result"], "memory");
        let memory = &response["memory"];
        assert_eq!(memory["allocation"], *allocation);
        assert_eq!(memory["byte_offset"], 0);
        assert_eq!(memory["returned_bytes"], length);
        assert_eq!(memory["availability"]["status"], "captured");
        assert_eq!(memory["availability"]["truncated"], false);
        memory["availability"].clone()
    }
}

pub fn run(scratch: &Scratch, binary: &Path, kir: &Path, request: &Path, order: &str) -> Value {
    let mut command = command(binary);
    command
        .args(["sim", "--diagnostic-kir-v18"])
        .arg(kir)
        .arg("--request")
        .arg(request)
        .args(["--protocol", "jsonl", "--wave-width", "64"]);
    let mut session = Session {
        job: process::Job::start_with_bound(
            &mut command,
            &scratch.0,
            &format!("{order}-debug"),
            std::time::Duration::from_secs(120),
            4 * 1024 * 1024,
        ),
        requests: process::fresh(&scratch.0.join(format!("{order}-debug.requests.jsonl"))),
        revision: 0,
        count: 0,
        configuration: None,
    };
    let capabilities = session.ok("discover_capabilities", json!({}));
    assert_eq!(capabilities["result"], "capabilities");
    assert!(!capabilities["capabilities"].as_array().unwrap().is_empty());
    let step = session.ok(
        "step",
        json!({"direction":"forward","granularity":"operation","count":1}),
    );
    assert!(step["events_advanced"].as_u64().unwrap() > 0);
    assert_eq!(step["snapshot"]["status"], "captured");
    assert_eq!(
        step["snapshot"]["snapshot"]["anchor"]["site"]["source"]["status"],
        "unavailable"
    );
    let lane = json!({"level":"lane","workgroup":[0,0,0],"wave":0,"lane":0});
    for scope in [
        json!({"level":"dispatch"}),
        json!({"level":"workgroup","workgroup":[0,0,0]}),
        json!({"level":"wave","workgroup":[0,0,0],"wave":0}),
        lane.clone(),
    ] {
        let reply = session.ok(
            "inspect_scope",
            json!({
                "scope":scope,"include_children":true,"page":{"limit":64}
            }),
        );
        assert_eq!(reply["result"], "scopes");
        assert!(!reply["scopes"].as_array().unwrap().is_empty());
    }
    let mut cursor = None;
    let mut pointers = Vec::<Value>::new();
    for _ in 0..4 {
        let mut page = json!({"limit":64});
        if let Some(value) = cursor.take() {
            page["cursor"] = value;
        }
        let reply = session.ok(
            "inspect_values",
            json!({
                "scope":lane,"selector":{"selector":"all"},"page":page
            }),
        );
        assert_eq!(reply["result"], "values");
        let values = reply["values"].as_array().unwrap();
        assert!(!values.is_empty());
        for entry in values {
            let value = &entry["availability"];
            if value["status"] == "captured"
                && value["value_type"]["kind"] == "pointer"
                && value["value_type"]["address_space"] == "global"
                && value["value"]["encoding"] == "allocation_relative_pointer"
            {
                let allocation = value["value"]["allocation"].clone();
                assert!(allocation["ordinal"].as_u64().is_some());
                assert!(allocation["generation"].as_u64().is_some());
                if !pointers.contains(&allocation) {
                    pointers.push(allocation);
                }
            }
        }
        cursor = reply.get("next_cursor").cloned();
        if cursor.is_none() {
            break;
        }
    }
    assert!(
        cursor.is_none(),
        "logical values exceeded bounded pagination"
    );
    assert_eq!(pointers.len(), 2, "two measured global allocations");
    let initial = cases::initial_output(cases::DEBUG_CASE);
    let length = (initial["bytes"].as_str().unwrap().len() - 2) / 2;
    let mut output = None;
    for pointer in pointers {
        let data = session.memory(&pointer, length);
        if data["bytes"] == initial["bytes"] && data["initialized"] == initial["initialized"] {
            assert!(
                output.replace(pointer).is_none(),
                "ambiguous output allocation"
            );
        }
    }
    let output = output.expect("output allocation measured from logical pointer values");
    let source = session.ask(
        "step",
        json!({
            "direction":"forward","granularity":"source","count":1
        }),
        true,
    );
    assert_eq!(source["unavailable"]["capability"], "source_sites");
    assert_eq!(
        source["unavailable"]["reason"],
        "requires_authenticated_map"
    );
    let registers = session.ask(
        "inspect_values",
        json!({
            "scope":lane,"selector":{"selector":"roots","roots":["register"]},"page":{"limit":64}
        }),
        true,
    );
    assert_eq!(registers["unavailable"]["capability"], "register_values");
    assert_eq!(registers["unavailable"]["reason"], "not_represented");
    let completed = session.ok("continue", json!({"max_events":1_000_000}));
    assert_eq!(
        completed["stop"],
        json!({"reason":"completed","outcome":"completed","exact":true})
    );
    let reversed = session.ok(
        "step",
        json!({"direction":"reverse","granularity":"operation","count":1}),
    );
    assert_eq!(reversed["snapshot"]["status"], "captured");
    let final_memory = session.memory(&output, length);
    let expected = cases::expected_output(cases::DEBUG_CASE, order);
    assert_eq!(final_memory["bytes"], expected["bytes"]);
    assert_eq!(final_memory["initialized"], expected["initialized"]);
    session.ok("terminate", json!({}));
    let summary = json!({"order":order,"commands":session.count,"complete":true,
        "configuration_identity":session.configuration,"measured_output_allocation":output});
    let capture = session.job.finish().expect("bounded debugger termination");
    process::success(&capture, "debugger");
    assert!(capture.stderr.is_empty(), "debugger success emitted stderr");
    summary
}
