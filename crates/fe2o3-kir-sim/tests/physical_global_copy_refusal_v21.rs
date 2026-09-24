//! Original wire profiles keep their refusals; V22 owns its distinct profile refusal.
use fe2o3_kir_sim::{
    SimulationCapabilityDispositionV1 as Disposition, SimulationKirWireVersionV1 as Wire,
    SimulationOperationSurfaceV1 as Surface, SimulationUnsupportedReasonCodeV1 as Reason,
    UnsupportedFeatureV1, semantic_capability_matrix_v1,
};

#[test]
fn global_copy_rows_refuse_every_existing_profile_without_reassigning_old_ids() {
    assert_eq!(Surface::PhysicalEntryDeclaration as u8, 42);
    assert_eq!(Surface::PhysicalEntryStep as u8, 43);
    assert_eq!(Surface::PhysicalGlobalCopyDeclaration as u8, 44);
    assert_eq!(Surface::PhysicalGlobalCopyStep as u8, 45);
    let matrix = semantic_capability_matrix_v1();
    let rows: Vec<_> = matrix
        .top_level_rows
        .iter()
        .filter(|row| row.kir_wire_version != Wire::V21 && row.kir_wire_version != Wire::V22)
        .filter(|row| {
            matches!(
                row.operation,
                Surface::PhysicalGlobalCopyDeclaration | Surface::PhysicalGlobalCopyStep
            )
        })
        .collect();
    assert_eq!(rows.len(), 2 * 4 * 9);
    for row in rows {
        assert_eq!(
            row.capability,
            Disposition::Unsupported {
                reason: Reason::PhysicalGlobalCopyProfile
            }
        );
    }
    assert_eq!(
        UnsupportedFeatureV1::PhysicalGlobalCopyProfile.reason_code(),
        Reason::PhysicalGlobalCopyProfile
    );
    assert_eq!(
        matrix
            .top_level_rows
            .iter()
            .filter(|row| row.kir_wire_version != Wire::V21
                && row.kir_wire_version != Wire::V22
                && (row.operation as u8) < 44)
            .count(),
        44 * 4 * 9
    );
}

#[test]
fn global_copy_rows_in_v22_use_the_exact_lds_profile_refusal() {
    let matrix = semantic_capability_matrix_v1();
    let rows: Vec<_> = matrix
        .top_level_rows
        .iter()
        .filter(|row| {
            row.kir_wire_version == Wire::V22
                && matches!(
                    row.operation,
                    Surface::PhysicalGlobalCopyDeclaration | Surface::PhysicalGlobalCopyStep
                )
        })
        .collect();
    assert_eq!(rows.len(), 2 * 4);
    for row in rows {
        assert_eq!(
            row.capability,
            Disposition::Unsupported {
                reason: Reason::PhysicalLdsExchangeProfile,
            }
        );
    }
}
