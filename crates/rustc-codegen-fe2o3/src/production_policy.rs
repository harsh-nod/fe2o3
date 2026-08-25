//! Selector-free production backend policy.

use std::env;
use std::ffi::OsStr;

pub(crate) fn environment_rejection() -> Option<String> {
    rejection_from_values(
        env::var_os(crate::OBSOLETE_CODEGEN_PIPELINE_ENV).as_deref(),
        env::var_os(crate::QUALIFICATION_ORACLE_ENV).as_deref(),
    )
}

fn rejection_from_values(
    obsolete_pipeline: Option<&OsStr>,
    qualification_oracle: Option<&OsStr>,
) -> Option<String> {
    if let Some(value) = obsolete_pipeline {
        return Some(format!(
            "{} has been removed; production compilation has no selector and temporary test oracles use {}; found {value:?}",
            crate::OBSOLETE_CODEGEN_PIPELINE_ENV,
            crate::QUALIFICATION_ORACLE_ENV,
        ));
    }
    qualification_oracle.map(|value| {
        format!(
            "{} is unavailable in the production backend; temporary qualification oracles require backend feature `qualification-oracles-test-only`; found {value:?}",
            crate::QUALIFICATION_ORACLE_ENV,
        )
    })
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::rejection_from_values;

    #[test]
    fn unset_environment_selects_the_only_production_pipeline() {
        assert_eq!(rejection_from_values(None, None), None);
    }

    #[test]
    fn obsolete_pipeline_selector_is_rejected_first() {
        let rejection = rejection_from_values(
            Some(OsStr::new("production-v1")),
            Some(OsStr::new("kernel-ir-v1")),
        )
        .expect("obsolete selector must be rejected");
        assert!(rejection.contains("FE2O3_CODEGEN_PIPELINE has been removed"));
        assert!(rejection.contains("FE2O3_QUALIFICATION_ORACLE_V1"));
    }

    #[test]
    fn qualification_oracle_is_absent_from_the_production_backend() {
        let rejection = rejection_from_values(None, Some(OsStr::new("kernel-ir-v1")))
            .expect("qualification oracle must be rejected");
        assert!(rejection.contains("FE2O3_QUALIFICATION_ORACLE_V1 is unavailable"));
        assert!(rejection.contains("qualification-oracles-test-only"));
    }
}
