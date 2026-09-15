#[test]
fn defined_math_permutation_v21_canonical_document_roundtrip() {
    use crate::semantic_direct_call_expansion_v1::{
        SemanticCallExpansionLimitsV1, SemanticCallExpansionV1,
    };
    use crate::semantic_mir_v1::defined_math_v1::tests::permutations::{
        BIND_ORDERS, canonical_permute,
    };

    for epoch in [false, true] {
        for bind_order in BIND_ORDERS {
            for (getter_locals, getter_blocks, bridge_locals, bridge_blocks) in [
                ([0, 1], [1, 0], [1, 0], [0, 1]),
                ([1, 0], [0, 1], [0, 1], [1, 0]),
                ([1, 0], [1, 0], [1, 0], [1, 0]),
            ] {
                let mut request = defined_math_document_request(epoch);
                canonical_permute(&mut request.functions[1], &getter_locals, &getter_blocks);
                canonical_permute(&mut request.functions[2], &bridge_locals, &bridge_blocks);
                canonical_permute(&mut request.functions[3], &bind_order, &[0]);
                // A different canonical body needs a newly observed commitment.
                request.functions[1].defined_capability_contract = None;
                request.functions[3].defined_capability_contract = None;
                defined_math_attach(&mut request);
                let before = request.functions.to_vec();
                let admitted = request
                    .admit_exact_v21(SemanticMirLimitsV1::default())
                    .unwrap_or_else(|error| panic!(
                        "epoch={epoch} getter={getter_locals:?}/{getter_blocks:?} bridge={bridge_locals:?}/{bridge_blocks:?} bind={bind_order:?}: original admission: {error:?}"
                    ));
                assert_eq!(admitted.functions(), before);
                let decoded = AdmittedInertSemanticMirV1::decode_exact_v21_canonical(
                    admitted.canonical_encoding(),
                    SemanticMirLimitsV1::default(),
                )
                .unwrap_or_else(|error| panic!("epoch={epoch}: permuted V21 decode: {error:?}"));
                assert_eq!(decoded.functions(), before);
                assert_eq!(decoded.canonical_encoding(), admitted.canonical_encoding());
                let expansion = SemanticCallExpansionV1::try_new(
                    &decoded,
                    SemanticCallExpansionLimitsV1::default(),
                )
                .unwrap();
                let occurrences = expansion.defined_capability_bindings(&decoded).unwrap();
                assert_eq!(occurrences.len(), if epoch { 3 } else { 2 });
            }
        }
    }
}

#[test]
fn defined_math_permutation_v21_rejects_stale_canonical_commitments() {
    use crate::semantic_mir_v1::defined_math_v1::tests::permutations::canonical_permute;

    for function in [1, 2, 3] {
        let mut request = defined_math_document_request(false);
        if function == 3 {
            canonical_permute(&mut request.functions[function], &[1, 2, 0], &[0]);
        } else {
            canonical_permute(&mut request.functions[function], &[1, 0], &[1, 0]);
        }
        assert!(
            request
                .admit_exact_v21(SemanticMirLimitsV1::default())
                .is_err(),
            "function {function}: unchanged metadata must not authenticate a different canonical body"
        );
    }
}
