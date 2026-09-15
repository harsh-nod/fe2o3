use super::*;
use serde_json::json;

const SOURCE: &str = "first(&mut storage); second(&mut storage);";
fn values() -> [Value; 3] {
    let start = SOURCE.rfind("&mut storage").unwrap();
    [
        json!({"$message_type":"diagnostic","level":"error","message":"borrow conflict",
            "code":{"code":"E0499"},"spans":[{"is_primary":true,"file_name":FILE,
                "byte_start":start,"byte_end":start+"&mut storage".len()}]}),
        json!({"$message_type":"diagnostic","level":"error","message":"aborting due to 1 previous error; 3 warnings emitted","code":null}),
        json!({"$message_type":SUMMARY,"test":"exact-test","cpu":"gfx950","configured":1,"after_analysis":0,"fatal":true}),
    ]
}
fn encode(values: &[Value]) -> Vec<u8> {
    values
        .iter()
        .map(|v| serde_json::to_string(v).unwrap())
        .collect::<Vec<_>>()
        .join("\n")
        .into_bytes()
}
fn checked(values: &[Value]) -> Result<(), &'static str> {
    check(&encode(values), "exact-test", "gfx950", SOURCE)
}

#[test]
fn phase49_duplicate_rust_json_requires_exact_error_and_no_callback() {
    assert_eq!(checked(&values()), Ok(()));
    for key in ["configured", "after_analysis"] {
        let mut v = values();
        v[2][key] = json!(2);
        assert_eq!(
            checked(&v),
            Err("duplicate Rust callback/fatal evidence mismatch")
        );
    }
    let mut v = values();
    v[2]["fatal"] = json!(false);
    assert_eq!(
        checked(&v),
        Err("duplicate Rust callback/fatal evidence mismatch")
    );
}
#[test]
fn phase49_duplicate_rust_json_does_not_accept_unrelated_errors_or_missing_failure() {
    let mut v = values();
    v[0]["code"]["code"] = json!("E0308");
    assert_eq!(
        checked(&v),
        Err("duplicate Rust rejection contains another coded error")
    );
    let mut v = values();
    v[0]["code"] = Value::Null;
    assert_eq!(
        checked(&v),
        Err("duplicate Rust rejection contains another uncoded error")
    );
    assert_eq!(
        checked(&values()[1..]),
        Err("duplicate Rust rejection omitted exact error or callback evidence")
    );
    assert_eq!(
        checked(&values()[..2]),
        Err("duplicate Rust rejection omitted exact error or callback evidence")
    );
}
#[test]
fn phase49_duplicate_rust_json_rejects_other_source_span_target_and_replay() {
    for key in ["byte_start", "byte_end"] {
        let mut v = values();
        v[0]["spans"][0][key] = json!(0);
        assert_eq!(
            checked(&v),
            Err("duplicate Rust error changed its exact storage borrow span")
        );
    }
    let mut v = values();
    v[0]["spans"][0]["file_name"] = json!("other.rs");
    assert_eq!(
        checked(&v),
        Err("duplicate Rust error changed its exact storage borrow span")
    );
    let mut v = values();
    let first = SOURCE.find("&mut storage").unwrap();
    v[0]["spans"][0]["byte_start"] = json!(first);
    v[0]["spans"][0]["byte_end"] = json!(first + "&mut storage".len());
    assert_eq!(
        checked(&v),
        Err("duplicate Rust error changed its exact storage borrow span")
    );
    for key in ["cpu", "test"] {
        let mut v = values();
        v[2][key] = json!("other");
        assert_eq!(
            checked(&v),
            Err("duplicate Rust callback/fatal evidence mismatch")
        );
    }
    let mut v = values().to_vec();
    v.push(v[2].clone());
    assert_eq!(
        checked(&v),
        Err("duplicate Rust callback/fatal evidence mismatch")
    );
}
#[test]
fn phase49_duplicate_rust_json_rejects_malformed_or_oversized_evidence() {
    assert_eq!(
        check(b"compiler panicked", "exact-test", "gfx950", SOURCE),
        Err("duplicate Rust diagnostic is not JSON")
    );
    assert_eq!(
        check(&vec![b'x'; MAX_OUTPUT + 1], "exact-test", "gfx950", SOURCE),
        Err("duplicate Rust diagnostic output limit")
    );
    assert_eq!(
        check(&vec![b'x'; MAX_LINE + 1], "exact-test", "gfx950", SOURCE),
        Err("duplicate Rust diagnostic line limit")
    );
    assert_eq!(
        check(&vec![b'\n'; MAX_LINES + 1], "exact-test", "gfx950", SOURCE),
        Err("duplicate Rust diagnostic line limit")
    );
}
