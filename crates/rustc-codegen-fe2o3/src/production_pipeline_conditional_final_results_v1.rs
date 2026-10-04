//! Test transport only; none of these records constructs source or proof custody.
use serde::{Deserialize, Serialize};

pub(super) const CHILD_SCHEMA: &str = "fe2o3-test-conditional-final-prefix-child-v1";
pub(super) const SCHEMA: &str = "fe2o3-test-conditional-final-prefix-results-v1";
pub(super) const MAIN_MODES: [&str; 4] = ["f", "fixed6", "work", "storage"];
pub(super) const LATE_MODES: [&str; 2] = ["late-agreement", "late-replay"];

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Identity {
    pub sha256: String,
    pub canonical_length: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Child {
    pub schema: String,
    pub target: String,
    pub mode: String,
    pub callbacks: u64,
    pub agreements: u64,
    pub replay_completed: u64,
    pub installed: u64,
    pub actual_f_identity: Option<Identity>,
    pub terminal: String,
    pub refusal: String,
    pub resource: Option<String>,
    pub fault_observed: bool,
    pub work: u64,
    pub floor_before: u64,
    pub floor_after: u64,
    pub account_preserved: bool,
    pub source_to_final_output_checked: bool,
    pub qualification_credit: bool,
    pub grants_artifact_or_launch_authority: bool,
    pub native_output_emitted: bool,
}

pub(super) fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub(super) fn decode_child(bytes: &[u8]) -> Result<Child, String> {
    if bytes.len() > 65_536 {
        return Err("conditional child JSON bound".into());
    }
    let child: Child = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let raw: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if serde_json::to_value(&child).map_err(|e| e.to_string())? != raw {
        return Err("conditional child requires every field including null options".into());
    }
    child.check(&child.target, &child.mode)?;
    Ok(child)
}

impl Child {
    pub fn check(&self, target: &str, mode: &str) -> Result<(), String> {
        if self.schema != CHILD_SCHEMA
            || self.target != target
            || self.mode != mode
            || !["gfx942", "gfx950"].contains(&target)
            || self.callbacks != 1
            || self.floor_before != 19
            || self.floor_after != self.floor_before
            || !self.account_preserved
            || self.refusal.is_empty()
            || self.refusal.len() > 16_384
            || self.qualification_credit
            || self.grants_artifact_or_launch_authority
            || self.native_output_emitted
        {
            return Err("conditional final child subject/account/authority fields".into());
        }
        let (agreement, replay, installed, terminal, resource, fault) = match mode {
            "f" => (1, 1, 1, "conditional-finalizer-required", None, false),
            "fixed6" => (0, 0, 0, "conditional-finalizer-required", None, false),
            "work" => (0, 0, 0, "resource-refusal", Some("work"), false),
            "storage" => (0, 0, 0, "resource-refusal", Some("storage"), false),
            "late-agreement" => (1, 0, 0, "injected-after-agreement", None, true),
            "late-replay" => (1, 1, 0, "injected-after-replay", None, true),
            _ => return Err("unknown conditional final mode".into()),
        };
        if (self.agreements, self.replay_completed, self.installed)
            != (agreement, replay, installed)
            || self.terminal != terminal
            || self.resource.as_deref() != resource
            || self.fault_observed != fault
            || self.source_to_final_output_checked != (agreement == 1)
            || self.actual_f_identity.is_some() != (agreement == 1)
            || (mode == "work" && self.work != 0)
            || (["f", "fixed6", "late-agreement", "late-replay"].contains(&mode) && self.work == 0)
            || (terminal == "conditional-finalizer-required"
                && !self.refusal.contains("FE2O3-COND-FINALIZER-001"))
        {
            return Err("conditional final child exact progress/refusal".into());
        }
        if let Some(identity) = &self.actual_f_identity {
            if !digest(&identity.sha256)
                || identity.sha256.bytes().all(|b| b == b'0')
                || identity.canonical_length == 0
            {
                return Err("actual final F identity".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Row {
    pub target: String,
    pub mode: String,
    pub captured_args_sha256: String,
    pub replay_args_sha256: String,
    pub child: Child,
    pub exit_code: i32,
    pub stdout_sha256: String,
    pub stderr_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Matrix {
    pub schema: String,
    pub suite: String,
    pub preparation_sha256: String,
    pub rows: Vec<Row>,
    pub source_unchanged: bool,
    pub inputs_unchanged: bool,
    pub tools_unchanged: bool,
    pub qualification_credit: bool,
    pub grants_artifact_or_launch_authority: bool,
    pub native_output_emitted: bool,
}

impl Matrix {
    pub fn check(&self) -> Result<(), String> {
        let modes = match self.suite.as_str() {
            "main" => &MAIN_MODES[..],
            "late" => &LATE_MODES[..],
            _ => return Err("conditional final suite".into()),
        };
        if self.schema != SCHEMA
            || !digest(&self.preparation_sha256)
            || self.rows.len() != 2 * modes.len()
            || !self.source_unchanged
            || !self.inputs_unchanged
            || !self.tools_unchanged
            || self.qualification_credit
            || self.grants_artifact_or_launch_authority
            || self.native_output_emitted
        {
            return Err("conditional final matrix completeness/authority".into());
        }
        for (target, rows) in ["gfx942", "gfx950"]
            .into_iter()
            .zip(self.rows.chunks_exact(modes.len()))
        {
            for (mode, row) in modes.iter().zip(rows) {
                if row.target != target
                    || row.mode != *mode
                    || row.exit_code != 0
                    || !digest(&row.captured_args_sha256)
                    || !digest(&row.replay_args_sha256)
                    || row.captured_args_sha256 == row.replay_args_sha256
                    || !digest(&row.stdout_sha256)
                    || !digest(&row.stderr_sha256)
                {
                    return Err("conditional final row subject/terminal/provenance".into());
                }
                row.child.check(target, mode)?;
            }
        }
        Ok(())
    }
}
