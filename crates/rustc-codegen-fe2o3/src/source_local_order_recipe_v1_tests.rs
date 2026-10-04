//! Pure inert-record tests. These do not claim live source or L qualification.
use super::*;
use serde_json::{Value, json};

fn binding() -> InstanceBinding {
    InstanceBinding {
        function: [1; 32],
        item: [2; 32],
        monomorphization: [3; 32],
        generic_types: [4; 32],
        const_arguments: [5; 32],
    }
}
fn origin() -> Origin {
    Origin {
        source: [6; 32],
        semantic: [7; 32],
        bound: [8; 32],
    }
}
fn original() -> ProgramIdentity {
    ProgramIdentity {
        digest: [9; 32],
        canonical_length: 123,
    }
}
fn prefix() -> ProgramIdentity {
    ProgramIdentity {
        digest: [10; 32],
        canonical_length: 456,
    }
}
fn revision() -> SourceBinding {
    SourceBinding::ExactRevision {
        expected_source_sha256: [6; 32],
        expected_original: original(),
        expected_prefix: prefix(),
    }
}
fn recipe() -> Recipe {
    Recipe::new(
        binding(),
        origin(),
        Order::ReverseReady,
        Constraint {
            relation: Relation::OrBeforeXor,
            strength: Strength::Exact,
        },
        revision(),
    )
}
fn value() -> Value {
    serde_json::to_value(recipe()).unwrap()
}
fn decode(value: &Value) -> Result<Recipe, Error> {
    Recipe::decode(&serde_json::to_vec(value).unwrap())
}

#[test]
fn fixed_schema_roundtrips_every_supported_choice_without_authority() {
    for preference in [Order::SourceOrder, Order::ReverseReady] {
        for relation in [Relation::XorBeforeOr, Relation::OrBeforeXor] {
            for strength in [Strength::Exact, Strength::Advisory] {
                for source_binding in [revision(), SourceBinding::RebindCurrent {}] {
                    let recipe = Recipe::new(
                        binding(),
                        origin(),
                        preference,
                        Constraint { relation, strength },
                        source_binding,
                    );
                    let encoded = recipe.encode().unwrap();
                    assert!(!encoded.is_empty() && encoded.len() <= BYTE_CAP);
                    assert_eq!(Recipe::decode(&encoded).unwrap(), recipe);
                    assert_eq!(Recipe::decode(&encoded).unwrap().encode().unwrap(), encoded);
                    assert_eq!(recipe.binding(), binding());
                    assert_eq!(recipe.preference(), preference);
                    assert_eq!(recipe.constraint(), Constraint { relation, strength });
                    assert_eq!(recipe.source_binding(), source_binding);
                    assert_eq!(recipe.origin(), origin());
                    assert!(!recipe.grants_authority());
                }
            }
        }
    }
    let encoded = value();
    assert_eq!(encoded["schema"], SCHEMA);
    assert_eq!(encoded["composition"], COMPOSITION);
    assert_eq!(
        Order::SourceOrder.preference(),
        U32LocalOrderPreferenceV1::SourceOrder
    );
    assert_eq!(
        Order::ReverseReady.preference(),
        U32LocalOrderPreferenceV1::ReverseReady
    );
}

#[test]
fn each_instance_axis_is_a_required_refusal_predicate_in_both_source_modes() {
    for source_binding in [revision(), SourceBinding::RebindCurrent {}] {
        let mut recipe = recipe();
        recipe.source_binding = source_binding;
        assert_eq!(recipe.bind(binding(), [6; 32]), Ok(Order::ReverseReady));
        for axis in 0..5 {
            let mut changed = binding();
            match axis {
                0 => changed.function[0] ^= 1,
                1 => changed.item[0] ^= 1,
                2 => changed.monomorphization[0] ^= 1,
                3 => changed.generic_types[0] ^= 1,
                4 => changed.const_arguments[0] ^= 1,
                _ => unreachable!(),
            }
            assert_eq!(recipe.bind(changed, [6; 32]), Err(Error::InstanceChanged));
        }
    }
}

#[test]
fn exact_revision_checks_source_and_both_complete_program_predicates() {
    let recipe = recipe();
    assert_eq!(
        recipe.bind(binding(), [42; 32]),
        Err(Error::SourceRevisionChanged)
    );
    assert_eq!(
        recipe.check_program_predicates(original(), prefix()),
        Ok(())
    );
    let mut changed = original();
    changed.digest[0] ^= 1;
    assert_eq!(
        recipe.check_program_predicates(changed, prefix()),
        Err(Error::OriginalProgramChanged)
    );
    changed = original();
    changed.canonical_length += 1;
    assert_eq!(
        recipe.check_program_predicates(changed, prefix()),
        Err(Error::OriginalProgramChanged)
    );
    let mut changed = prefix();
    changed.digest[0] ^= 1;
    assert_eq!(
        recipe.check_program_predicates(original(), changed),
        Err(Error::PrefixProgramChanged)
    );
    changed = prefix();
    changed.canonical_length += 1;
    assert_eq!(
        recipe.check_program_predicates(original(), changed),
        Err(Error::PrefixProgramChanged)
    );
    // A matching file byte hash cannot hide a different current program.
    assert_eq!(recipe.bind(binding(), [6; 32]), Ok(Order::ReverseReady));
    assert_eq!(
        recipe.check_program_predicates(original(), changed),
        Err(Error::PrefixProgramChanged)
    );
}

#[test]
fn current_rebinding_skips_only_historical_revision_predicates() {
    let mut recipe = recipe();
    recipe.source_binding = SourceBinding::RebindCurrent {};
    assert_eq!(recipe.bind(binding(), [42; 32]), Ok(Order::ReverseReady));
    let other = ProgramIdentity {
        digest: [99; 32],
        canonical_length: u64::MAX,
    };
    assert_eq!(recipe.check_program_predicates(other, other), Ok(()));
    let mut changed = binding();
    changed.item[0] ^= 1;
    assert_eq!(recipe.bind(changed, [42; 32]), Err(Error::InstanceChanged));
    assert!(!recipe.grants_authority());
}

#[test]
fn historical_annotations_cannot_substitute_for_explicit_revision_predicates() {
    let mut recipe = recipe();
    recipe.origin = Origin {
        source: [99; 32],
        semantic: [99; 32],
        bound: [99; 32],
    };
    assert_eq!(recipe.bind(binding(), [6; 32]), Ok(Order::ReverseReady));
    assert_eq!(
        recipe.bind(binding(), [99; 32]),
        Err(Error::SourceRevisionChanged)
    );
    assert_eq!(
        recipe.check_program_predicates(original(), prefix()),
        Ok(())
    );
}

#[test]
fn exact_and_advisory_compare_actual_relation_without_changing_requested_schedule() {
    for preference in [Order::SourceOrder, Order::ReverseReady] {
        for requested in [Relation::XorBeforeOr, Relation::OrBeforeXor] {
            for actual in [Relation::XorBeforeOr, Relation::OrBeforeXor] {
                let mut recipe = recipe();
                recipe.preference = preference;
                recipe.constraint.relation = requested;
                recipe.constraint.strength = Strength::Exact;
                let exact = recipe.evaluate_constraint(actual);
                recipe.constraint.strength = Strength::Advisory;
                let advisory = recipe.evaluate_constraint(actual);
                if requested == actual {
                    assert_eq!(exact, Ok(ConstraintOutcome::Honored { relation: actual }));
                    assert_eq!(advisory, exact);
                } else {
                    assert_eq!(
                        exact,
                        Err(Error::ExactConstraintNotHonored { requested, actual })
                    );
                    assert_eq!(
                        advisory,
                        Ok(ConstraintOutcome::NotHonored { requested, actual })
                    );
                }
                assert_eq!(recipe.preference(), preference);
            }
        }
    }
}

#[test]
fn typed_advisory_report_explicitly_carries_requested_and_actual_canonical_order() {
    let result = ConstraintOutcome::NotHonored {
        requested: Relation::OrBeforeXor,
        actual: Relation::XorBeforeOr,
    };
    assert_eq!(
        serde_json::to_value(result).unwrap(),
        json!({
            "status": "not_honored", "requested": "or_before_xor", "actual": "xor_before_or",
        })
    );
}

#[test]
fn unsupported_version_composition_target_shape_preferences_and_strength_refuse() {
    for field in ["schema", "composition", "target", "shape", "preference"] {
        let mut changed = value();
        changed[field] = json!("unsupported");
        assert_eq!(
            decode(&changed),
            Err(Error::MalformedOrUnsupported),
            "{field}"
        );
    }
    for field in ["relation", "strength"] {
        let mut changed = value();
        changed["constraint"][field] = json!("unsupported");
        assert_eq!(
            decode(&changed),
            Err(Error::MalformedOrUnsupported),
            "{field}"
        );
    }
    let mut changed = value();
    changed["source_binding"]["mode"] = json!("best_effort");
    assert_eq!(decode(&changed), Err(Error::MalformedOrUnsupported));
}

#[test]
fn no_pass_selector_check_toggle_owner_receipt_or_old_private_wire_is_accepted() {
    for field in [
        "passes",
        "disable_checks",
        "owner",
        "receipt",
        "output",
        "instructions",
        "source_span",
        "policy",
        "private_draft_version",
    ] {
        let mut changed = value();
        changed[field] = json!(null);
        assert_eq!(
            decode(&changed),
            Err(Error::MalformedOrUnsupported),
            "{field}"
        );
    }
    let mut old = value();
    let object = old.as_object_mut().unwrap();
    object.remove("schema");
    object.remove("composition");
    object.insert("private_draft_version".into(), json!(1));
    assert_eq!(decode(&old), Err(Error::MalformedOrUnsupported));
}

#[test]
fn profile_axes_and_all_array_lengths_are_closed() {
    for field in ["required", "maximum"] {
        let mut changed = value();
        changed[field] = json!([128, 1, 1]);
        assert_eq!(decode(&changed), Err(Error::ProfileMismatch));
    }
    let mut changed = value();
    changed["parameter_ordinals"] = json!([1, 2, 4, 3]);
    assert_eq!(decode(&changed), Err(Error::ProfileMismatch));
    for (field, replacement) in [
        ("required", json!([64, 1])),
        ("maximum", json!([64, 1, 1, 1])),
        ("parameter_ordinals", json!([1, 2, 3])),
    ] {
        let mut changed = value();
        changed[field] = replacement;
        assert_eq!(decode(&changed), Err(Error::MalformedOrUnsupported));
    }
    let mut changed = value();
    changed["binding"]["function"] = json!([0]);
    assert_eq!(decode(&changed), Err(Error::MalformedOrUnsupported));
    let mut changed = value();
    changed["source_binding"]["expected_prefix"]["canonical_length"] = json!(-1);
    assert_eq!(decode(&changed), Err(Error::MalformedOrUnsupported));
}

#[test]
fn every_nested_record_rejects_unknown_fields_and_missing_fields() {
    for path in [
        vec![],
        vec!["binding"],
        vec!["origin"],
        vec!["constraint"],
        vec!["source_binding"],
        vec!["source_binding", "expected_original"],
        vec!["source_binding", "expected_prefix"],
    ] {
        let mut changed = value();
        let mut selected = &mut changed;
        for field in &path {
            selected = &mut selected[*field];
        }
        selected
            .as_object_mut()
            .unwrap()
            .insert("extra".into(), json!(false));
        assert_eq!(
            decode(&changed),
            Err(Error::MalformedOrUnsupported),
            "{path:?}"
        );
    }
    for field in [
        "schema",
        "composition",
        "target",
        "required",
        "maximum",
        "parameter_ordinals",
        "shape",
        "binding",
        "preference",
        "constraint",
        "source_binding",
        "origin",
    ] {
        let mut changed = value();
        changed.as_object_mut().unwrap().remove(field);
        assert_eq!(
            decode(&changed),
            Err(Error::MalformedOrUnsupported),
            "{field}"
        );
    }
    for (field, extra) in [
        ("expected_source_sha256", json!([0])),
        ("expected_original", json!(null)),
        ("expected_prefix", json!(null)),
        ("extra", json!(false)),
    ] {
        let mut changed = value();
        changed["source_binding"] = json!({"mode": "rebind_current"});
        changed["source_binding"][field] = extra;
        assert_eq!(
            decode(&changed),
            Err(Error::MalformedOrUnsupported),
            "{field}"
        );
    }
}

#[test]
fn duplicate_fields_at_all_record_levels_refuse() {
    let text = String::from_utf8(recipe().encode().unwrap()).unwrap();
    for (needle, valid_value) in [
        ("\"schema\":", json!(SCHEMA)),
        ("\"function\":", serde_json::to_value([1_u8; 32]).unwrap()),
        ("\"source\":", serde_json::to_value([6_u8; 32]).unwrap()),
        ("\"relation\":", json!("or_before_xor")),
        ("\"mode\":", json!("exact_revision")),
        ("\"canonical_length\":", json!(123)),
    ] {
        let duplicate = text.replacen(needle, &format!("{needle}{valid_value},{needle}"), 1);
        assert_ne!(duplicate, text);
        assert_eq!(
            Recipe::decode(duplicate.as_bytes()),
            Err(Error::MalformedOrUnsupported),
            "{needle}"
        );
    }
}

#[test]
fn encoding_bounds_malformed_utf8_trailing_content_and_oversized_input_refuse() {
    assert_eq!(Recipe::decode(&[]), Err(Error::ByteLimit));
    assert_eq!(Recipe::decode(&[b' '; BYTE_CAP + 1]), Err(Error::ByteLimit));
    assert_eq!(Recipe::decode(&[0xff]), Err(Error::MalformedOrUnsupported));
    let mut encoded = recipe().encode().unwrap();
    encoded.extend_from_slice(b"{}");
    assert_eq!(Recipe::decode(&encoded), Err(Error::MalformedOrUnsupported));
    let mut encoded = recipe().encode().unwrap();
    encoded.resize(BYTE_CAP, b' ');
    assert_eq!(Recipe::decode(&encoded).unwrap(), recipe());
    encoded.push(b' ');
    assert_eq!(Recipe::decode(&encoded), Err(Error::ByteLimit));
    let mut invalid = recipe();
    invalid.required[0] = 128;
    assert_eq!(invalid.encode(), Err(Error::ProfileMismatch));
}

#[test]
fn bounded_writer_refuses_before_mutating_or_growing_past_fixed_capacity() {
    let mut writer = BoundedWriter { bytes: Vec::new() };
    writer.bytes.try_reserve_exact(BYTE_CAP).unwrap();
    writer.write_all(&[b'x'; BYTE_CAP]).unwrap();
    let capacity = writer.bytes.capacity();
    assert!(writer.write_all(b"x").is_err());
    assert_eq!(writer.bytes.len(), BYTE_CAP);
    assert_eq!(writer.bytes.capacity(), capacity);
    assert!(writer.bytes.iter().all(|byte| *byte == b'x'));
}
