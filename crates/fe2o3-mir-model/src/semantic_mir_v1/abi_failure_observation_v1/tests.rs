use super::*;

fn abi(arguments: u32) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([2; 32]),
        SemanticLayoutIdentityV1::from_sha256([3; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        arguments,
        (0..arguments).map(|_| SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            SemanticTypeIdV1::from_index(0), SemanticAbiPassModeV1::Ignore,
        ))).collect(),
        SemanticAbiValueV1::new(SemanticTypeIdV1::from_index(0), SemanticAbiPassModeV1::Ignore),
    ).unwrap().with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; arguments as usize]).unwrap()
}

fn observed(abi: Option<&SemanticFunctionAbiV1>) -> Text {
    render(&[], SemanticMirLocationV1::Function(SemanticFunctionIdV1::from_index(9)),
        "function", abi, &SemanticMirErrorV1::InvalidFunctionAbi,
        Some(OsStr::new("1")), Some(OsStr::new("abi"))).unwrap()
}

fn text(value: &Text) -> &str { std::str::from_utf8(&value.bytes[..value.len]).unwrap() }

#[test]
fn observation_requires_exact_flags_and_an_abi_failure() {
    for trace in [None, Some(OsStr::new("0")), Some(OsStr::new("true")), Some(OsStr::new("1"))] {
        for role in [None, Some(OsStr::new("policy")), Some(OsStr::new("abi"))] {
            for error in [SemanticMirErrorV1::InvalidFunctionAbi, SemanticMirErrorV1::InvalidTypeLayout] {
                assert_eq!(render(&[], SemanticMirLocationV1::Module, "callables", None, &error,
                    trace, role).is_some(),
                    trace == Some(OsStr::new("1")) && role == Some(OsStr::new("abi"))
                    && error == SemanticMirErrorV1::InvalidFunctionAbi);
            }
        }
    }
}

#[test]
fn output_keeps_the_actual_location_source_ownership_and_abi_values() {
    let abi = abi(2);
    let value = observed(Some(&abi));
    let text = text(&value);
    assert!(text.starts_with("ABI_FAILURE_BEGIN location=Function(SemanticFunctionIdV1(9)) step=function"));
    assert!(text.contains("source_count=2 argument_count=2"));
    assert!(text.contains("source_argument=1 type=SemanticTypeIdV1(0) ownership=Some(ByValue)"));
    assert!(text.contains("physical_argument=1 role=Source"));
    assert!(text.contains("value=return source=SemanticTypeIdV1(0)"));
    assert!(text.contains("mode=Ignore"));
    assert!(text.ends_with("ABI_FAILURE_END payload_truncated=false diagnostic_only=true\n"));
}

#[test]
fn argument_prefix_and_byte_budget_are_explicitly_bounded() {
    let abi = abi((MAX_ARGUMENTS + 1) as u32);
    let value = observed(Some(&abi));
    let text = text(&value);
    assert_eq!(text.matches("source_argument=").count(), MAX_ARGUMENTS);
    assert_eq!(text.matches("physical_argument=").count(), MAX_ARGUMENTS);
    assert!(text.contains("source_prefix_truncated=true"));
    assert!(text.contains("argument_prefix_truncated=true"));
    assert!(!text.contains("physical_argument=8"));
    assert!(value.len <= MAX_BYTES);

    let mut value = Text::default();
    value.write_str(&"a".repeat(MAX_BYTES - 96)).unwrap();
    assert!(value.write_str("x").is_err());
    value.end(false);
    assert!(text_of(&value).ends_with("ABI_FAILURE_END payload_truncated=true diagnostic_only=true\n"));
    assert!(value.len <= MAX_BYTES);
    fn text_of(value: &Text) -> &str { std::str::from_utf8(&value.bytes[..value.len]).unwrap() }
}

#[test]
fn missing_localization_is_not_presented_as_a_valid_abi() {
    let value = observed(None);
    assert!(text(&value).contains("no_localized_abi=true"));
    assert!(!text(&value).contains("source_argument="));
    assert!(text(&value).contains("diagnostic_only=true"));
}

#[test]
fn failure_observation_never_reexecutes_or_replaces_the_validator() {
    for error in [SemanticMirErrorV1::InvalidFunctionAbi, SemanticMirErrorV1::InvalidTypeLayout] {
        let mut calls = 0;
        let result: Result<(), _> = (|| { calls += 1; Err(error.clone()) })();
        let result = result.map_err(|error| record(&[], SemanticMirLocationV1::Module,
            "callables", None, error));
        assert_eq!(calls, 1);
        assert_eq!(result, Err(error));
    }
}
