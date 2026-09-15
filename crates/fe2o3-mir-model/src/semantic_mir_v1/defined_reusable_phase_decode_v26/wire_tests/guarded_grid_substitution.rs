use super::*;

#[test]
fn phase_v26_payload_is_not_a_guarded_grid_record() {
    for recipe in recipes() {
        let mut bytes = encoded(record(recipe));
        assert_eq!(bytes[0], 9);
        bytes[0] = 10;
        let error = decode(&bytes, SemanticMirWireVersionV1::V26)
            .expect_err("original phase payload cannot become Grid custody by changing the tag");
        assert!(
            matches!(
                error,
                SemanticMirDecodeErrorV1::UnexpectedEnd { .. }
                    | SemanticMirDecodeErrorV1::Validation(SemanticMirErrorV1::InvalidFunctionAbi)
            ),
            "{recipe:?}: {error:?}"
        );
        assert!(matches!(
            decode(&bytes, SemanticMirWireVersionV1::V25),
            Err(SemanticMirDecodeErrorV1::InvalidTag {
                context: "defined capability contract",
                value: 10,
                ..
            })
        ));
    }
}
