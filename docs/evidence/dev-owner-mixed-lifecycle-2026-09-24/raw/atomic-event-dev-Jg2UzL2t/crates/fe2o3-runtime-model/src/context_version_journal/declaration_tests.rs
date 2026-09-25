use super::*;

#[test]
fn allocation_key_derives_preserve_lexicographic_order_at_boundaries() {
    let values = [0, 1, u64::MAX - 1, u64::MAX];
    for context_generation in values {
        for local in values {
            let left = ContextAllocationKeyV1 {
                context_generation,
                local,
            };
            for other_context in values {
                for other_local in values {
                    let right = ContextAllocationKeyV1 {
                        context_generation: other_context,
                        local: other_local,
                    };
                    let expected = (context_generation, local).cmp(&(other_context, other_local));
                    assert_eq!(left.cmp(&right), expected);
                    assert_eq!(left.partial_cmp(&right), Some(expected));
                    assert_eq!(left == right, expected.is_eq());
                    assert_eq!(left < right, expected.is_lt());
                }
            }
        }
    }
}

#[test]
fn writer_reference_equality_includes_every_identity_coordinate() {
    let mut references = Vec::new();
    for slot in [0, usize::MAX] {
        for context_generation in [0, u64::MAX] {
            for local in [0, u64::MAX] {
                for kind in [
                    ContextWriterKindV1::Synchronous,
                    ContextWriterKindV1::Submission,
                ] {
                    references.push(ContextWriterReferenceV1 {
                        slot,
                        key: ContextWriterKeyV1 {
                            context_generation,
                            local,
                            kind,
                        },
                    });
                }
            }
        }
    }
    for (i, left) in references.iter().enumerate() {
        for (j, right) in references.iter().enumerate() {
            assert_eq!(left == right, i == j);
        }
    }
}

#[test]
fn proof_and_runtime_include_one_unmodified_declaration_source() {
    let runtime = include_str!("../context_version_journal.rs");
    let proof = include_str!("../../verus/context_journal_representation_v1.rs");
    let declarations = include_str!("declarations.rs");
    assert!(runtime.contains("($($items:tt)*) => { $($items)* };"));
    assert!(runtime.contains("include!(\"context_version_journal/declarations.rs\");"));
    assert!(proof.contains("use vstd::prelude::verus as context_journal_declarations_v1;"));
    assert!(proof.contains("include!(\"../src/context_version_journal/declarations.rs\");"));
    assert!(declarations.contains(
        "#[must_use = \"discarding a reference does not release its retained writer slot\"]"
    ));
    assert!(declarations.contains("#[cfg(test)]\n    indexed_accesses: Cell<usize>,"));
    assert!(!runtime.contains("pub struct ContextVersionJournalV1 {"));
    assert!(!proof.contains("pub struct ContextVersionJournalV1 {"));
}
