use super::super::{ActiveCodegenProfileV1, SemanticLayoutTargetV1};
use std::mem::size_of;

fn plain() -> SemanticLayoutTargetV1 {
    SemanticLayoutTargetV1::new("amdgcn-amd-amdhsa", "e-p:64:64", 64).unwrap()
}

fn profiled(cpu: &str, features: &str) -> SemanticLayoutTargetV1 {
    SemanticLayoutTargetV1::new_with_codegen_profile(
        "amdgcn-amd-amdhsa",
        "e-p:64:64",
        64,
        cpu,
        "",
        features,
    )
    .unwrap()
}

// Independent owning-field formula, not a fold of the proposed visitor.
fn actual_heap(owner: &SemanticLayoutTargetV1) -> usize {
    let strings = owner.llvm_target.len() + owner.data_layout.len();
    match owner.active_codegen_profile.as_ref() {
        None => strings,
        Some(profile) => {
            strings
                + size_of::<ActiveCodegenProfileV1>()
                + profile.cpu.as_ref().map_or(0, String::capacity)
                + profile.features.capacity()
        }
    }
}

fn visits(owner: &SemanticLayoutTargetV1) -> Vec<(usize, usize)> {
    let mut rows = Vec::new();
    owner
        .visit_retained_heap_storage_v1(|count, width| {
            rows.push((count, width));
            Ok::<_, ()>(())
        })
        .unwrap();
    rows
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Arithmetic,
    Bytes,
    Items,
}

fn bounded_from(
    owner: &SemanticLayoutTargetV1,
    mut bytes: usize,
    mut items: usize,
    max_bytes: usize,
    max_items: usize,
) -> Result<(usize, usize), Refusal> {
    if bytes > max_bytes {
        return Err(Refusal::Bytes);
    }
    if items > max_items {
        return Err(Refusal::Items);
    }
    owner.visit_retained_heap_storage_v1(|count, width| {
        let added = count.checked_mul(width).ok_or(Refusal::Arithmetic)?;
        let next_bytes = bytes.checked_add(added).ok_or(Refusal::Arithmetic)?;
        let next_items = items.checked_add(1).ok_or(Refusal::Arithmetic)?;
        if next_bytes > max_bytes {
            return Err(Refusal::Bytes);
        }
        if next_items > max_items {
            return Err(Refusal::Items);
        }
        bytes = next_bytes;
        items = next_items;
        Ok(())
    })?;
    Ok((bytes, items))
}

#[test]
fn no_profile_counts_only_the_two_actual_boxed_strings() {
    let owner = plain();
    assert!(owner.active_codegen_profile.is_none());
    assert_eq!(
        visits(&owner),
        vec![(owner.llvm_target.len(), 1), (owner.data_layout.len(), 1)]
    );
    assert_eq!(
        actual_heap(&owner),
        owner.llvm_target.len() + owner.data_layout.len()
    );
}

#[test]
fn present_profile_with_unknown_cpu_keeps_its_box_and_empty_feature_visit() {
    let owner = profiled("native", "");
    let profile = owner.active_codegen_profile.as_ref().unwrap();
    assert!(profile.cpu.is_none());
    assert!(profile.features.is_empty());
    assert_eq!(
        visits(&owner),
        vec![
            (owner.llvm_target.len(), 1),
            (owner.data_layout.len(), 1),
            (1, size_of::<ActiveCodegenProfileV1>()),
            (profile.features.capacity(), 1),
        ]
    );
}

#[test]
fn present_cpu_and_features_use_actual_string_capacities() {
    let mut owner = profiled("gfx942", "-xnack,+wavefrontsize64");
    let before = owner.clone();
    let profile = owner.active_codegen_profile.as_mut().unwrap();
    profile.cpu.as_mut().unwrap().reserve_exact(257);
    profile.features.reserve_exact(509);
    let profile = owner.active_codegen_profile.as_ref().unwrap();
    let cpu = profile.cpu.as_ref().unwrap();
    assert!(cpu.capacity() > cpu.len());
    assert!(profile.features.capacity() > profile.features.len());
    let expected = vec![
        (owner.llvm_target.len(), 1),
        (owner.data_layout.len(), 1),
        (1, size_of::<ActiveCodegenProfileV1>()),
        (cpu.capacity(), 1),
        (profile.features.capacity(), 1),
    ];
    assert_eq!(visits(&owner), expected);
    assert_eq!(
        expected
            .iter()
            .map(|&(count, width)| count * width)
            .sum::<usize>(),
        actual_heap(&owner)
    );
    assert_eq!(
        owner, before,
        "capacity-only changes must preserve target values"
    );
}

#[test]
fn consumed_input_string_spare_capacity_is_not_boxed_string_payload() {
    let mut target = String::with_capacity(4096);
    target.push_str("amdgcn-amd-amdhsa");
    let mut layout = String::with_capacity(8192);
    layout.push_str("e-p:64:64");
    let owner = SemanticLayoutTargetV1::new(target, layout, 64).unwrap();
    assert_eq!(visits(&owner), vec![(17, 1), (9, 1)]);
    assert_eq!(actual_heap(&owner), 26);
}

#[test]
fn caller_counts_one_root_header_and_the_boxed_profile_header_once() {
    for owner in [
        plain(),
        profiled("native", ""),
        profiled("gfx942", "-xnack"),
    ] {
        let header = size_of::<SemanticLayoutTargetV1>();
        let expected = header + actual_heap(&owner);
        let count = visits(&owner).len() + 1;
        assert_eq!(
            bounded_from(&owner, header, 1, expected, count),
            Ok((expected, count))
        );
    }
}

#[test]
fn exact_byte_and_item_limits_refuse_one_short_without_a_partial_report() {
    let owner = profiled("gfx942", "-xnack,+wavefrontsize64");
    let header = size_of::<SemanticLayoutTargetV1>();
    let expected = header + actual_heap(&owner);
    assert_eq!(
        bounded_from(&owner, header, 1, expected - 1, 6),
        Err(Refusal::Bytes)
    );
    assert_eq!(
        bounded_from(&owner, header, 1, expected, 5),
        Err(Refusal::Items)
    );
    assert_eq!(
        bounded_from(&owner, header, 1, expected, 6),
        Ok((expected, 6))
    );
}

#[test]
fn every_callback_refusal_stops_before_any_later_payload_visit() {
    let owner = profiled("gfx942", "-xnack,+wavefrontsize64");
    for cutoff in 1..=5 {
        let mut calls = 0;
        let result = owner.visit_retained_heap_storage_v1(|_, _| {
            calls += 1;
            if calls == cutoff { Err(cutoff) } else { Ok(()) }
        });
        assert_eq!(result, Err(cutoff));
        assert_eq!(calls, cutoff);
    }
}

#[test]
fn enclosing_counter_overflow_is_propagated_not_saturated() {
    let owner = plain();
    assert_eq!(
        bounded_from(&owner, usize::MAX, 0, usize::MAX, usize::MAX),
        Err(Refusal::Arithmetic)
    );
    assert_eq!(
        bounded_from(&owner, 0, usize::MAX, usize::MAX, usize::MAX),
        Err(Refusal::Arithmetic)
    );
}

#[test]
fn observation_preserves_canonical_text_and_does_not_admit_bad_features() {
    let owner = profiled("gfx950", "-xnack,+wavefrontsize64");
    let mut before = String::new();
    owner.write_canonical(&mut before);
    let _ = visits(&owner);
    let mut after = String::new();
    owner.write_canonical(&mut after);
    assert_eq!(after, before);
    assert!(
        SemanticLayoutTargetV1::new_with_codegen_profile(
            "amdgcn-amd-amdhsa",
            "e-p:64:64",
            64,
            "gfx950",
            "+xnack,-xnack",
            "",
        )
        .is_err()
    );
}
