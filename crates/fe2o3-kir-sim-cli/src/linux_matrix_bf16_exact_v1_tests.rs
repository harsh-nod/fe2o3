use super::*;

#[test]
fn exact_matrix_domain_error_has_closed_positional_details_only() {
    for (role, name) in [
        (fe2o3_kir_sim::MatrixInputRoleV1::A, "a"),
        (fe2o3_kir_sim::MatrixInputRoleV1::B, "b"),
        (fe2o3_kir_sim::MatrixInputRoleV1::Accumulator, "accumulator"),
    ] {
        let error = SimulationExecutionErrorKindV1::UnsupportedMatrixInputDomain {
            role,
            lane: 63,
            component: 3,
        };
        assert_eq!(
            serde_json::to_value(execution_kind(&error)).unwrap(),
            "execution_unsupported_matrix_input_domain"
        );
        assert_eq!(
            serde_json::to_value(execution_detail(&error)).unwrap(),
            serde_json::json!({
                "kind": "unsupported_matrix_input_domain", "role": name, "lane": 63, "component": 3,
            })
        );
    }
}
