//! Closed diagnostic selector; never a normal compiler continuation.
use super::{ExtractionModeV1, OsString, PreparedExtractionV1};
use fe2o3_lower_mir_kernel::ProductionScopedTileObservationOrderV29 as Order;
use std::ffi::OsStr;

pub(super) const OUTPUT_ENV: &str = "FE2O3_EXTRACT_DIAGNOSTIC_KIR_PATH_V18";
pub(super) const ORDER_ENV: &str = "FE2O3_EXTRACT_DIAGNOSTIC_TILE_ORDER_V18";

pub(super) fn validate_options(
    output: Option<&OsStr>,
    order: Option<&OsStr>,
    other_outputs: impl IntoIterator<Item = bool>,
) -> Result<Option<Order>, String> {
    match (output, order) {
        (None, None) => Ok(None),
        (Some(path), Some(order)) => {
            if path.is_empty() {
                return Err(format!("{OUTPUT_ENV} must not be empty"));
            }
            if other_outputs.into_iter().any(|selected| selected) {
                return Err("diagnostic V18 is mutually exclusive with every other extraction output or continuation".into());
            }
            match order.to_str() {
                Some("blocked") => Ok(Some(Order::Blocked)),
                Some("striped") => Ok(Some(Order::Striped)),
                _ => Err(format!("{ORDER_ENV} must be exactly blocked or striped")),
            }
        }
        _ => Err(format!(
            "{OUTPUT_ENV} and {ORDER_ENV} must be supplied together"
        )),
    }
}

pub(super) fn select_mode(
    mut prepared: PreparedExtractionV1,
    output: Option<OsString>,
    order: Option<Order>,
) -> Result<PreparedExtractionV1, String> {
    match (output, order) {
        (None, None) => {}
        (Some(output), Some(order)) if !output.is_empty() => {
            if let PreparedExtractionV1::Selected(selected) = &mut prepared {
                if !matches!(selected.mode, ExtractionModeV1::KernelIr)
                    || selected.crate_binding_output.is_some()
                {
                    return Err("diagnostic V18 is mutually exclusive with other outputs and crate-binding sidecars".into());
                }
                selected.mode = ExtractionModeV1::DiagnosticKirV18(output, order);
            }
        }
        _ => return Err("diagnostic V18 requires a nonempty output and exact order".into()),
    }
    Ok(prepared)
}
