//! JSON shape controls only; no compiler, source capture or proof execution.
use super::*;
use serde_json::{Value, json};

fn success() -> (Request, Value) {
    let value = json!({
        "request": {
            "case": "Exact", "target": "gfx942", "args_sha256": ([1u8; 32]),
            "source": [{"path": "fixture.rs", "sha256": ([9u8; 32])}]
        },
        "result": {"Ok": {
            "semantic": ([2u8; 32]), "n": ([3u8; 32]), "roots": [7],
            "query_work": 1, "guard_work": 1, "retained_floor": 2,
            "rows": [{
                "root": 7, "body": 9, "root_identity": ([4u8; 32]),
                "body_identity": ([5u8; 32]), "report_semantic": ([2u8; 32]),
                "ordinal": 0, "certificates": 1, "checked_additions": 1,
                "dynamic_u32_bound": true,
                "outcome": {"Joined": {
                    "header": [0, 1], "parameter": "p", "initial": "i", "update": "u",
                    "step": "s", "overflow": "o", "initial_edge": "e", "backedge": "b",
                    "source_initialization": "i", "source_update": "u", "source_guard": "g"
                }},
                "guard": {"Joined": {
                    "header": [0, 1], "bound": [0, 0], "condition": [0, 1, 0, 0],
                    "body": [0, 2], "exit": [0, 3],
                    "then_edge": [0, 1, 0], "else_edge": [0, 1, 1]
                }}
            }]
        }}
    });
    let request = serde_json::from_value(value["request"].clone()).unwrap();
    (request, value)
}

fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(value).unwrap()
}

fn reject_shape(value: &Value, request: &Request) {
    let bytes = bytes(value);
    assert!(serde_json::from_slice::<Report>(&bytes).is_err());
    assert!(decode(Some(0), Some(&bytes), request).is_err());
}

#[test]
fn report_and_all_nested_success_objects_reject_unknown_fields() {
    let (request, value) = success();
    decode(Some(0), Some(&bytes(&value)), &request).unwrap();
    for path in [
        "",
        "/request",
        "/request/source/0",
        "/result/Ok",
        "/result/Ok/rows/0",
        "/result/Ok/rows/0/outcome/Joined",
        "/result/Ok/rows/0/guard/Joined",
    ] {
        let mut changed = value.clone();
        changed
            .pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unexpected".into(), json!({"extra": true}));
        reject_shape(&changed, &request);
    }
    let typed: Report = serde_json::from_slice(&bytes(&value)).unwrap();
    assert_eq!(serde_json::to_value(typed).unwrap(), value);
}

#[test]
fn outcomes_and_request_tags_reject_unknown_variants_and_payloads() {
    let (request, value) = success();
    for path in ["/result/Ok/rows/0/outcome", "/result/Ok/rows/0/guard"] {
        let mut changed = value.clone();
        let object = changed.pointer_mut(path).unwrap().as_object_mut().unwrap();
        let payload = object.remove("Joined").unwrap();
        object.insert("UnexpectedOutcome".into(), payload);
        reject_shape(&changed, &request);
        let mut changed = value.clone();
        changed
            .pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("UnexpectedOutcome".into(), Value::Null);
        reject_shape(&changed, &request);
    }
    for tag in [
        json!("UnexpectedCase"),
        json!({"Exact": {"unexpected": true}}),
    ] {
        let mut changed = value.clone();
        changed["request"]["case"] = tag;
        reject_shape(&changed, &request);
    }
    let mut wide = value;
    wide["request"]["case"] = "U64".into();
    let row = &mut wide["result"]["Ok"]["rows"][0];
    row["outcome"] = "NoCertificate".into();
    row["guard"] = "NoCertificate".into();
    row["ordinal"] = Value::Null;
    row["certificates"] = 0.into();
    row["dynamic_u32_bound"] = false.into();
    let wide_request: Request = serde_json::from_value(wide["request"].clone()).unwrap();
    decode(Some(0), Some(&bytes(&wide)), &wide_request).unwrap();
    for field in ["outcome", "guard"] {
        let mut changed = wide.clone();
        changed["result"]["Ok"]["rows"][0][field] = json!({"NoCertificate": {"unexpected": true}});
        reject_shape(&changed, &wide_request);
        let mut changed = wide.clone();
        changed["result"]["Ok"]["rows"][0][field] = json!({"Unavailable": {"unexpected": true}});
        reject_shape(&changed, &wide_request);
    }
}

#[test]
fn failure_fields_and_stage_tags_are_closed_but_refusals_stay_failures() {
    let (request, mut value) = success();
    value["result"] = json!({"Err": {"stage": "Guard", "detail": "refused"}});
    assert!(serde_json::from_slice::<Report>(&bytes(&value)).is_ok());
    assert!(decode(Some(0), Some(&bytes(&value)), &request).is_err());
    let mut changed = value.clone();
    changed["result"]["Err"]["unexpected"] = true.into();
    reject_shape(&changed, &request);
    for tag in [
        json!("UnexpectedStage"),
        json!({"Guard": {"unexpected": true}}),
    ] {
        let mut changed = value.clone();
        changed["result"]["Err"]["stage"] = tag;
        reject_shape(&changed, &request);
    }
    let mut changed = value.clone();
    changed["result"] = json!({"UnexpectedResult": {"stage": "Guard", "detail": "refused"}});
    reject_shape(&changed, &request);
    for status in [None, Some(1), Some(101)] {
        assert!(decode(status, Some(&bytes(&value)), &request).is_err());
    }
}
