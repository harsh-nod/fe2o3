//! Unit tests exercise the same bounded codec consumed by actual callbacks.
use super::*;

fn example(order: Order) -> Recipe {
    Recipe::new(
        InstanceBinding {
            function: [1; 32],
            item: [2; 32],
            monomorphization: [3; 32],
            generic_types: [4; 32],
            const_arguments: [5; 32],
        },
        Origin {
            source: [6; 32],
            semantic: [7; 32],
            bound: [8; 32],
        },
        order,
    )
}

#[test]
fn recipe_two_preferences_roundtrip_without_authority() {
    let first = example(Order::SourceOrder);
    let second = example(Order::ReverseReady);
    assert_ne!(first.encode().unwrap(), second.encode().unwrap());
    for recipe in [first, second] {
        let bytes = recipe.encode().unwrap();
        assert!(bytes.len() <= BYTE_CAP);
        let decoded = Recipe::decode(&bytes).unwrap();
        assert_eq!(decoded, recipe);
        assert_eq!(decoded.encode().unwrap(), bytes);
        assert_eq!(decoded.bind(recipe.binding).unwrap(), recipe.preference);
    }
}

#[test]
fn recipe_refuses_duplicate_unknown_and_trailing_fields() {
    let original = String::from_utf8(example(Order::SourceOrder).encode().unwrap()).unwrap();
    let duplicate_binding = format!(
        "\"binding\":{{\"function\":{},",
        serde_json::to_string(&[1u8; 32]).unwrap()
    );
    let duplicate_origin = format!(
        "\"origin\":{{\"source\":{},",
        serde_json::to_string(&[6u8; 32]).unwrap()
    );
    let cases = [
        original.replacen("{", "{\"private_draft_version\":1,", 1),
        original.replacen("{", "{\"arbitrary_pass\":\"disable_verifier\",", 1),
        original.replacen("\"binding\":{", "\"binding\":{\"extra\":0,", 1),
        original.replacen("\"binding\":{", &duplicate_binding, 1),
        original.replacen("\"origin\":{", &duplicate_origin, 1),
        format!("{original} {{}}"),
    ];
    for bytes in cases {
        assert_eq!(
            Recipe::decode(bytes.as_bytes()).unwrap_err(),
            "local-order recipe malformed or unknown field"
        );
    }
}

#[test]
fn recipe_refuses_version_size_profile_and_unknown_enum() {
    let recipe = example(Order::SourceOrder);
    let original = String::from_utf8(recipe.encode().unwrap()).unwrap();
    let version = original.replace("\"private_draft_version\":1", "\"private_draft_version\":2");
    assert_eq!(
        Recipe::decode(version.as_bytes()).unwrap_err(),
        "local-order recipe private draft version unsupported"
    );
    for oversized in [Vec::new(), vec![b' '; BYTE_CAP + 1]] {
        assert_eq!(
            Recipe::decode(&oversized).unwrap_err(),
            "local-order recipe byte limit"
        );
    }
    for changed in [
        original.replace("\"required\":[64,1,1]", "\"required\":[128,1,1]"),
        original.replace("\"maximum\":[64,1,1]", "\"maximum\":[128,1,1]"),
        original.replace(
            "\"parameter_ordinals\":[1,2,3,4]",
            "\"parameter_ordinals\":[2,1,3,4]",
        ),
    ] {
        assert_ne!(changed, original);
        assert_eq!(
            Recipe::decode(changed.as_bytes()).unwrap_err(),
            "local-order recipe exact profile mismatch"
        );
    }
    for changed in [
        original.replace("Gfx942XnackOffWave64", "Gfx950XnackOffWave64"),
        original.replace("U32XorOrAndFourDistinctFormalParameters", "AnyOperations"),
        original.replace("source_order", "disable_checks"),
    ] {
        assert_ne!(changed, original);
        assert_eq!(
            Recipe::decode(changed.as_bytes()).unwrap_err(),
            "local-order recipe malformed or unknown field"
        );
    }
}

#[test]
fn recipe_requires_every_measured_instance_axis_not_source_digest_equality() {
    let recipe = example(Order::ReverseReady);
    for index in 0..5 {
        let mut current = recipe.binding;
        match index {
            0 => current.function[0] ^= 1,
            1 => current.item[0] ^= 1,
            2 => current.monomorphization[0] ^= 1,
            3 => current.generic_types[0] ^= 1,
            _ => current.const_arguments[0] ^= 1,
        }
        assert_eq!(recipe.bind(current).unwrap_err(), BINDING_CHANGED);
    }
    let changed_source = Recipe::new(
        recipe.binding,
        Origin {
            source: [9; 32],
            ..recipe.origin()
        },
        Order::ReverseReady,
    );
    assert_ne!(recipe.encode().unwrap(), changed_source.encode().unwrap());
    assert_eq!(
        recipe.bind(changed_source.binding).unwrap(),
        Order::ReverseReady
    );
    // This tests only inert binding comparison. Actual source/shape admission is
    // separately mandatory in the real callback ladder, never in this codec.
}
