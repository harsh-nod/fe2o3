//! Explicit normal composition source selection. The flag selects the live
//! importer, never turns inert JSON/LLVM/canonical bytes into source custody.
use super::*;
use crate::production_pipeline::{CollectedRustStage, ProductionCompilation};
use fe2o3_lower_mir_kernel::ProductionOrderedCompositionCheckedKirOwnerV1;

pub(super) const SELECTION_ENV: &str = "FE2O3_EXTRACT_ORDERED_COMPOSITION_V1";
fn parse_selection(value: Option<&std::ffi::OsStr>) -> Result<bool, String> {
    match value {
        None => Ok(false),
        Some(v) if v == "1" => Ok(true),
        Some(_) => Err("ordered composition source selection requires exact value 1".into()),
    }
}
const INCOMPATIBLE_OUTPUTS: [&str; 9] = [
    "FE2O3_EXTRACT_DIAGNOSTIC_KIR_PATH_V16",
    "FE2O3_EXTRACT_DIAGNOSTIC_KIR_PATH_V17",
    "FE2O3_EXTRACT_DIAGNOSTIC_KIR_PATH_V19",
    "FE2O3_EXTRACT_DIAGNOSTIC_PHYSICAL_ENTRY_DIRECTORY_V20",
    "FE2O3_EXTRACT_DIAGNOSTIC_PHYSICAL_GLOBAL_COPY_DIRECTORY_V21",
    "FE2O3_EXTRACT_DIAGNOSTIC_PHYSICAL_LDS_EXCHANGE_DIRECTORY_V22",
    "FE2O3_EXTRACT_DIAGNOSTIC_ORDERED_COMPOSITION_DIRECTORY_V1",
    "FE2O3_DIAGNOSTIC_ORDERED_ORIGIN_PATH_V1",
    "FE2O3_EXTRACT_ORDERED_COMPOSITION_PROMOTION_REQUEST_V1",
];
fn disjoint_selection(
    selected: bool,
    present: impl IntoIterator<Item = bool>,
) -> Result<bool, String> {
    if selected && present.into_iter().any(|v| v) {
        return Err("normal ordered composition source selection conflicts with diagnostic or promotion output".into());
    }
    Ok(selected)
}
pub(super) fn selected() -> Result<bool, String> {
    disjoint_selection(
        parse_selection(env::var_os(SELECTION_ENV).as_deref())?,
        INCOMPATIBLE_OUTPUTS
            .iter()
            .map(|key| env::var_os(key).is_some()),
    )
}
pub(super) fn refuse_semantic_v3() -> Result<(), String> {
    if selected()? {
        Err("ordered composition has no admitted semantic V3/protected handoff route".into())
    } else {
        Ok(())
    }
}
fn observation(
    checked: &ProductionOrderedCompositionCheckedKirOwnerV1,
    kind: &str,
) -> serde_json::Value {
    let conditions = checked
        .launch_envelope_requirements()
        .iter()
        .map(|r| {
            serde_json::json!({
                "parameter":r.parameter_index(), "minimum_byte_len":r.minimum_byte_len(),
                "requires_initialized_read":r.requires_initialized_read(),
                "requires_write_permission":r.requires_write_permission(),
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "schema":"fe2o3-ordered-composition-normal-v1", "output":kind,
        "canonical_sha256":lower_hex_v1(checked.executable().identity().digest()),
        "source_semantic_sha256":lower_hex_v1(checked.semantic_ssa().source_semantic().semantic_sha256().as_bytes()),
        "definition_count":checked.composition().definitions().len(),
        "helper_count":checked.composition().helpers().len(),
        "call_count":checked.composition().calls().len(),
        "occurrence_count":checked.composition().occurrences().len(),
        "ranked_dependency_count":checked.ranked_dependencies().len(),
        "formal_access_count":checked.formal_obligations().accesses().len(),
        "original_formal_bounds":format!("{:?}",checked.formal_obligations().bounds_requirements()),
        "original_formal_alias_requirements":format!("{:?}",checked.formal_obligations().runtime_alias_requirements()),
        "additional_launch_envelope_requirements":conditions,
        "runtime_conditions_discharged":false, "source_custody_exported":false,
        "functional_equivalence_claim":false, "host_launch_authority":false,
        "protected_authority":false, "hardware_execution":false,
    })
}
pub(super) fn extract_llvm<'tcx>(
    transaction: ProductionCompilation<'tcx, CollectedRustStage<'tcx>>,
    output: &Path,
    expected_target: Option<&str>,
) -> Result<(), String> {
    let target = transaction.lower_ordered_composition_target_v1()?;
    validate_compiler_handoff_target(target.target_name(), expected_target)?;
    let report = observation(target.checked(), "llvm");
    publish_new_inert_output(
        output,
        target.llvm_ir().as_bytes(),
        4 * 1024 * 1024,
        "ordered composition checked LLVM",
    )?;
    eprintln!("{report}");
    Ok(())
}
pub(super) fn extract_handoff<'tcx>(
    transaction: ProductionCompilation<'tcx, CollectedRustStage<'tcx>>,
    output: &Path,
    expected_target: Option<&str>,
) -> Result<(), String> {
    let target = transaction.lower_ordered_composition_target_v1()?;
    validate_compiler_handoff_target(target.target_name(), expected_target)?;
    let prepared =
        crate::production_worker_handoff::prepare_ordered_composition_worker_handoff_v1(target)
            .map_err(|e| e.to_string())?;
    let report = observation(prepared.checked(), "inert_handoff");
    let (handoff, _descriptor) = prepared
        .into_inert_for_extraction()
        .map_err(|e| e.to_string())?;
    publish_new_inert_output(
        output,
        handoff.canonical_bytes(),
        fe2o3_compiler_ffi::MAX_COMPILER_MODULE_HANDOFF_BYTES_V2,
        "ordered composition inert handoff",
    )?;
    eprintln!("{report}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_is_explicit_and_closed() {
        assert!(!parse_selection(None).unwrap());
        assert!(parse_selection(Some(std::ffi::OsStr::new("1"))).unwrap());
        for value in ["", "0", "true", "V17", "composition", "1 ", " 1", "01"] {
            assert!(parse_selection(Some(std::ffi::OsStr::new(value))).is_err());
        }
    }
    #[test]
    fn selection_refuses_each_diagnostic_and_promotion_collision() {
        for i in 0..INCOMPATIBLE_OUTPUTS.len() {
            assert!(
                disjoint_selection(true, (0..INCOMPATIBLE_OUTPUTS.len()).map(|j| i == j)).is_err()
            );
        }
        assert!(disjoint_selection(true, [false; 9]).unwrap());
        assert!(!disjoint_selection(false, [true; 9]).unwrap());
    }
    #[cfg(unix)]
    #[test]
    fn selection_refuses_non_utf8_without_fallback() {
        use std::os::unix::ffi::OsStrExt;
        assert!(parse_selection(Some(std::ffi::OsStr::from_bytes(&[0xff]))).is_err());
    }
}
