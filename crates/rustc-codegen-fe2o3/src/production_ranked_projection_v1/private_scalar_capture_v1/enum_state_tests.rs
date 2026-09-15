use super::*;

fn conditional(variant: usize, fields: Vec<Option<Value>>) -> Value {
    let mut alternatives = vec![None, None];
    alternatives[variant] = Some(Value::Fields(fields));
    Value::Variants {
        ty: SemanticTypeIdV1::from_index(6),
        possible: 1 << variant,
        fields: alternatives,
    }
}

#[test]
fn conditional_field_join_preserves_only_constructed_variant_data() {
    let some = conditional(
        1,
        vec![Some(Value::Fields(vec![
            Some(Value::Opaque),
            Some(Value::Opaque),
        ]))],
    );
    let none = conditional(0, vec![]);
    for reverse in [false, true] {
        let (left, right) = if reverse {
            (&none, &some)
        } else {
            (&some, &none)
        };
        let mut escaped = BTreeSet::new();
        let joined = left
            .join(right, &mut escaped, &mut Budget::new(MAX_WORK))
            .unwrap();
        let Value::Variants {
            possible, fields, ..
        } = joined
        else {
            panic!()
        };
        assert_eq!(possible, 3);
        assert_eq!(fields[0], Some(Value::Fields(vec![])));
        let Value::Variants {
            fields: original, ..
        } = &some
        else {
            panic!()
        };
        assert_eq!(fields[1], original[1]);
        assert!(escaped.is_empty());
    }
}

#[test]
fn conditional_field_join_does_not_initialize_missing_or_opaque_payloads() {
    let some = conditional(1, vec![Some(Value::Fields(vec![Some(Value::Opaque)]))]);
    let missing = conditional(1, vec![None]);
    let mut escaped = BTreeSet::new();
    let joined = some
        .join(&missing, &mut escaped, &mut Budget::new(MAX_WORK))
        .unwrap();
    let Value::Variants { fields, .. } = joined else {
        panic!()
    };
    assert_eq!(fields[1], Some(Value::Fields(vec![None])));
    assert_eq!(
        some.join(&Value::Opaque, &mut escaped, &mut Budget::new(MAX_WORK))
            .unwrap(),
        Value::Opaque
    );
    let mut foreign = some.clone();
    let Value::Variants { ty, .. } = &mut foreign else {
        panic!()
    };
    *ty = SemanticTypeIdV1::from_index(7);
    assert_eq!(
        some.join(&foreign, &mut escaped, &mut Budget::new(MAX_WORK))
            .unwrap(),
        Value::Opaque
    );
}

#[test]
fn conditional_reference_death_and_escape_poison_remain_transitive() {
    let reference = Value::Reference(Reference {
        target: 0,
        mutable: false,
        borrow: Site {
            block: 1,
            statement: 0,
        },
    });
    let mut initial = Flow::default();
    initial.values.insert(0, Value::Opaque);
    initial
        .values
        .insert(1, conditional(1, vec![Some(reference)]));
    for escape in [false, true] {
        let mut flow = initial.clone();
        if escape {
            flow.escape(0, &mut Budget::new(MAX_WORK)).unwrap();
        } else {
            flow.kill(0, &mut Budget::new(MAX_WORK)).unwrap();
        }
        assert!(!flow.values[&1].contains_references());
        assert!(flow.escaped.contains(&0));
    }
    let mut other = initial.clone();
    other.values.remove(&1);
    let joined = initial.join(&other, &mut Budget::new(MAX_WORK)).unwrap();
    assert!(!joined.values.contains_key(&1));
    assert!(joined.escaped.contains(&0));
}

#[test]
fn conditional_field_storage_and_work_keep_existing_ceilings() {
    let value = conditional(1, vec![Some(Value::Opaque)]);
    assert_eq!(value.nodes(), 4);
    let before = value.clone();
    assert!(
        value
            .join(
                &conditional(0, vec![]),
                &mut BTreeSet::new(),
                &mut Budget::new(0)
            )
            .is_err()
    );
    assert_eq!(value, before);
    let mut flow = Flow::default();
    flow.values.insert(1, value);
    let mut budget = Budget::new(MAX_WORK);
    let _reserved = budget.reserve(MAX_STORAGE).unwrap();
    assert!(flow.clone_retained(&mut budget).is_err());
}
