//! Selector-free production backend policy.

use std::env;
use std::ffi::OsStr;

const REMOVED_QUALIFICATION_ORACLE_ENV: &str = "FE2O3_QUALIFICATION_ORACLE_V1";

pub(crate) fn environment_rejection() -> Option<String> {
    rejection_from_values(
        env::var_os(crate::OBSOLETE_CODEGEN_PIPELINE_ENV).as_deref(),
        env::var_os(REMOVED_QUALIFICATION_ORACLE_ENV).as_deref(),
    )
}

fn rejection_from_values(
    obsolete_pipeline: Option<&OsStr>,
    qualification_oracle: Option<&OsStr>,
) -> Option<String> {
    if let Some(value) = obsolete_pipeline {
        return Some(format!(
            "{} and {} are unsupported and removed; production compilation has no alternate route; found {value:?}",
            crate::OBSOLETE_CODEGEN_PIPELINE_ENV,
            REMOVED_QUALIFICATION_ORACLE_ENV,
        ));
    }
    qualification_oracle.map(|value| {
        format!(
            "{} is unsupported and removed; production compilation has no alternate route; found {value:?}",
            REMOVED_QUALIFICATION_ORACLE_ENV,
        )
    })
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::{REMOVED_QUALIFICATION_ORACLE_ENV, rejection_from_values};

    #[test]
    fn unset_environment_accepts_the_only_production_compiler() {
        assert_eq!(rejection_from_values(None, None), None);
    }

    #[test]
    fn obsolete_pipeline_selector_is_rejected_first() {
        let rejection = rejection_from_values(
            Some(OsStr::new("production-v1")),
            Some(OsStr::new("kernel-ir-v1")),
        )
        .expect("obsolete selector must be rejected");
        assert!(rejection.contains("FE2O3_CODEGEN_PIPELINE"));
        assert!(rejection.contains("unsupported and removed"));
        assert!(rejection.contains("no alternate route"));
        assert!(rejection.contains("FE2O3_QUALIFICATION_ORACLE_V1"));
    }

    #[test]
    fn qualification_oracle_is_absent_from_the_production_backend() {
        let rejection = rejection_from_values(None, Some(OsStr::new("kernel-ir-v1")))
            .expect("qualification oracle must be rejected");
        assert!(rejection.contains(REMOVED_QUALIFICATION_ORACLE_ENV));
        assert!(rejection.contains("unsupported and removed"));
        assert!(rejection.contains("no alternate route"));
    }
}
