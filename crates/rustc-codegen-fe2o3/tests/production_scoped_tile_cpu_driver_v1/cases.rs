use super::*;
#[path = "../../../../examples/workgroup_sync_v1/src/mixed_tile_oracle.rs"]
mod oracle;
use oracle::{MixedTileConfigV1, MixedTileOrderV1, mixed_tile_oracle_v1};

pub const CANARY: u32 = 0xdad0_0057;
pub const PREFIX: usize = 3;
const SUFFIX: usize = 2;

#[derive(Clone, Copy, Debug)]
pub struct Case {
    pub input: usize,
    pub output: usize,
    pub base: u64,
    pub groups: u32,
}

pub const DEBUG_CASE: Case = Case {
    input: 65,
    output: 8,
    base: 0,
    groups: 1,
};

pub fn cases() -> Vec<Case> {
    let mut result = [0, 1, 2, 3, 63, 64, 65, 127, 128, 129, 191, 192, 193]
        .map(|input| Case {
            input,
            output: 64,
            base: 0,
            groups: 1,
        })
        .to_vec();
    result.extend(
        [
            (193, 128, 1),
            (193, 128, 64),
            (193, 128, 192),
            (193, 128, 193),
            (193, 128, u64::MAX - 1),
            (193, 128, u64::MAX),
            (193, 63, 0),
            (193, 1, 0),
            (193, 0, 0),
            (0, 128, 0),
            (65, 65, 0),
            (193, 128, 0),
        ]
        .map(|(input, output, base)| Case {
            input,
            output,
            base,
            groups: 2,
        }),
    );
    result.push(DEBUG_CASE);
    result
}

fn input_words(length: usize) -> Vec<u32> {
    (0..length)
        .map(|index| {
            [
                u32::MAX,
                u32::MAX - 17,
                0,
                0x8000_0001,
                11 + 17 * index as u32,
            ][index % 5]
        })
        .collect()
}

fn bits(values: &[bool]) -> String {
    let mut bytes = vec![0; values.len().div_ceil(8)];
    for (index, ready) in values.iter().enumerate() {
        if *ready {
            bytes[index / 8] |= 1 << (index % 8);
        }
    }
    format!("0x{}", hex(&bytes))
}

fn backing(values: &[u32], ready: &[bool], access: &str) -> Value {
    assert_eq!(values.len(), ready.len());
    let mut words = vec![CANARY; PREFIX];
    words.extend(values);
    words.extend([CANARY; SUFFIX]);
    let mut initialized = vec![true; PREFIX];
    initialized.extend(ready);
    initialized.extend([true; SUFFIX]);
    let bytes: Vec<u8> = words.into_iter().flat_map(u32::to_le_bytes).collect();
    let initialized: Vec<bool> = initialized
        .into_iter()
        .flat_map(|ready| [ready; 4])
        .collect();
    json!({"element":"u32","access":access,"alignment":4,
        "bytes":format!("0x{}",hex(&bytes)),"initialized":bits(&initialized)})
}

pub fn input(case: Case) -> Value {
    backing(
        &input_words(case.input),
        &vec![true; case.input],
        "read_only",
    )
}

pub fn initial_output(case: Case) -> Value {
    backing(
        &vec![CANARY; case.output],
        &vec![false; case.output],
        "read_write",
    )
}

pub fn expected_output(case: Case, order: &str) -> Value {
    let order = match order {
        "blocked" => MixedTileOrderV1::Blocked,
        "striped" => MixedTileOrderV1::Striped,
        _ => panic!("unknown test order"),
    };
    let mut output = vec![CANARY; case.output];
    let mut ready = vec![false; case.output];
    mixed_tile_oracle_v1(
        &input_words(case.input),
        MixedTileConfigV1 {
            base: case.base,
            workgroups: case.groups,
            order,
        },
        &mut output,
        &mut ready,
    )
    .unwrap();
    backing(&output, &ready, "read_write")
}

pub fn request(kernel: &str, case: Case) -> Value {
    let mut input = input(case);
    input["id"] = json!(7);
    let mut output = initial_output(case);
    output["id"] = json!(11);
    json!({
        "schema":"fe2o3-simulation-request-v1", "kernel":kernel,
        "grid":[case.groups*64,1,1], "workgroup":[64,1,1],
        "arguments":[
            {"kind":"buffer_view","backing":7,"element":"u32","access":"read_only",
             "alignment":4,"byte_offset":PREFIX*4,"elements":case.input},
            {"kind":"scalar","type":"u64","bits":format!("0x{:016x}",case.base)},
            {"kind":"buffer_view","backing":11,"element":"u32","access":"read_write",
             "alignment":4,"byte_offset":PREFIX*4,"elements":case.output}
        ], "shared_buffers":[input,output]
    })
}

pub fn check_result(result: &Value, request: &Value, case: Case, order: &str, kir: &Value) {
    assert_eq!(result["schema"], "fe2o3-simulation-result-v1");
    assert_eq!(result["status"], "ok");
    assert_eq!(result["authority"], "observation_only");
    assert_eq!(result["simulated"], true);
    for field in [
        "hardware_observed",
        "hardware_validation",
        "performance_prediction",
    ] {
        assert_eq!(result[field], false, "{field}");
    }
    assert_eq!(
        result["kir"],
        json!({"sha256":kir["identity_sha256"],"canonical_bytes":kir["canonical_bytes"]})
    );
    assert_eq!(result["arguments"], request["arguments"]);
    assert_eq!(result["counts"]["invocations_executed"], case.groups * 64);
    assert!(result["counts"]["steps_executed"].as_u64().unwrap() > 0);
    assert_eq!(result["target_profile"]["index_bits"], 64);
    assert_eq!(result["schedule"]["coverage"]["complete"], true);
    assert_eq!(
        result["shared_buffers"],
        json!([
            {"id":7,"buffer":input(case)}, {"id":11,"buffer":expected_output(case,order)}
        ]),
        "{order} {case:?}: bytes/initialization/canaries/input changed"
    );
}

#[test]
fn independent_oracle_keeps_orders_distinct_and_finite_width() {
    let blocked = expected_output(DEBUG_CASE, "blocked");
    let striped = expected_output(DEBUG_CASE, "striped");
    assert_ne!(blocked["bytes"], striped["bytes"]);
    for order in ["blocked", "striped"] {
        let case = Case {
            base: u64::MAX,
            ..DEBUG_CASE
        };
        assert_eq!(expected_output(case, order), initial_output(case));
    }
    assert_eq!(cases().len(), 26);
}
