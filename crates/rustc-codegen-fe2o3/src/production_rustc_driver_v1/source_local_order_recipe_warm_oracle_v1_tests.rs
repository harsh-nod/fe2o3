//! Test-only full inert normal-output oracle; no borrowed compiler owner escapes.
use super::*;
use serde::Serialize;
use std::io::{self, Write};

pub(super) const NORMAL_LIMIT: usize = 512 * 1024;
pub(super) const ROW_LIMIT: usize = 2048;
pub(super) const SUMMARY_LIMIT: usize = 4096;
pub(super) const NORMAL_PREFIX: &[u8] = b"FE2O3_RECIPE_NORMAL_V1 ";
pub(super) const ROW_PREFIX: &[u8] = b"FE2O3_RECIPE_SERIES_V1 ";

pub(super) struct Bounded {
    pub(super) bytes: Vec<u8>,
    cap: usize,
}
impl Bounded {
    pub(super) fn new(cap: usize) -> Self {
        Self {
            bytes: Vec::new(),
            cap,
        }
    }
}
impl Write for Bounded {
    fn write(&mut self, value: &[u8]) -> io::Result<usize> {
        let next = self
            .bytes
            .len()
            .checked_add(value.len())
            .ok_or_else(|| io::Error::other("observer length overflow"))?;
        if next > self.cap {
            return Err(io::Error::other("observer byte envelope"));
        }
        self.bytes
            .try_reserve_exact(value.len())
            .map_err(|_| io::Error::other("observer allocation"))?;
        self.bytes.extend_from_slice(value);
        Ok(value.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(super) fn json_bytes(value: &impl Serialize, cap: usize) -> Result<Vec<u8>, String> {
    let mut output = Bounded::new(cap);
    serde_json::to_writer(&mut output, value).map_err(|error| error.to_string())?;
    output.write_all(b"\n").map_err(|error| error.to_string())?;
    Ok(output.bytes)
}
pub(super) fn relation(value: api::SourceLocalOrderRelationV1) -> &'static str {
    match value {
        api::SourceLocalOrderRelationV1::XorBeforeOr => "xor_before_or",
        api::SourceLocalOrderRelationV1::OrBeforeXor => "or_before_xor",
    }
}
pub(super) fn identity(value: &api::SourceLocalOrderIdentityObservationV1) -> serde_json::Value {
    serde_json::json!({"digest": value.digest(), "canonical_length": value.canonical_length()})
}
pub(super) fn normal(output: &Output) -> Result<Vec<u8>, String> {
    // Exhaustive field destructuring detects added evidence fields at compile
    // time; all current public output fields enter this deterministic oracle.
    let Output {
        llvm: _,
        created_recipe: _,
        evidence: _,
    } = output;
    let api::SourceLocalOrderRecipeEvidenceV1 {
        source_sha256,
        source_initializer,
        semantic_sha256,
        instance_axes,
        original,
        input,
        output: output_identity,
        requested_order,
        requested_relation,
        strength,
        source_binding_mode,
        actual_relation,
        constraint_outcome,
        region,
        output_result_order,
        prefix_execution_bytes,
        transition_sha256,
        transition_bytes,
        transition_rows,
        fresh_formal_counts,
        llvm_sha256,
        descriptor_sha256,
        recipe_sha256,
        canonical_work,
        canonical_peak_storage,
        created,
    } = output.evidence();
    let outcome = match constraint_outcome {
        api::SourceLocalOrderConstraintOutcomeV1::Honored { relation: value } => {
            serde_json::json!({"status":"honored","relation":relation(*value)})
        }
        api::SourceLocalOrderConstraintOutcomeV1::NotHonored { requested, actual } => {
            serde_json::json!({"status":"not_honored","requested":relation(*requested),"actual":relation(*actual)})
        }
    };
    let evidence = serde_json::json!({
        "source_sha256":source_sha256,"source_initializer":source_initializer,
        "semantic_sha256":semantic_sha256,"instance_axes":instance_axes,
        "original":identity(original),"input":identity(input),"output":identity(output_identity),
        "requested_order":match requested_order { api::SourceLocalOrderOrderV1::SourceOrder=>"source_order", api::SourceLocalOrderOrderV1::ReverseReady=>"reverse_ready" },
        "requested_relation":relation(*requested_relation),
        "strength":match strength { api::SourceLocalOrderStrengthV1::Exact=>"exact", api::SourceLocalOrderStrengthV1::Advisory=>"advisory" },
        "source_binding_mode":match source_binding_mode { api::SourceLocalOrderSourceBindingModeV1::ExactRevision=>"exact_revision", api::SourceLocalOrderSourceBindingModeV1::RebindCurrent=>"rebind_current" },
        "actual_relation":relation(*actual_relation),"constraint_outcome":outcome,
        "region":region,"output_result_order":output_result_order,
        "prefix_execution_bytes":prefix_execution_bytes.as_slice(),
        "transition_sha256":transition_sha256,"transition_bytes":transition_bytes,
        "transition_rows":transition_rows,"fresh_formal_counts":fresh_formal_counts,
        "llvm_sha256":llvm_sha256,"descriptor_sha256":descriptor_sha256,
        "recipe_sha256":recipe_sha256,"canonical_work":canonical_work,
        "canonical_peak_storage":canonical_peak_storage,"created":created,
        "descriptor_producer":output.evidence().descriptor_producer(),
        "composition":output.evidence().composition(),"grants_authority":output.evidence().grants_authority()
    });
    json_bytes(
        &serde_json::json!({
            "llvm":output.llvm_ir(),"created_recipe":output.created_recipe_bytes(),
            "evidence":evidence,"grants_artifact_or_launch_authority":output.grants_artifact_or_launch_authority()
        }),
        NORMAL_LIMIT,
    )
}
pub(super) fn emit_normal_to(out: &mut impl Write, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > NORMAL_LIMIT {
        return Err("normal output envelope".into());
    }
    out.write_all(b"\n")
        .and_then(|_| out.write_all(NORMAL_PREFIX))
        .and_then(|_| out.write_all(bytes))
        .and_then(|_| out.flush())
        .map_err(|error| error.to_string())
}
pub(super) fn emit_normal(bytes: &[u8]) -> Result<(), String> {
    let stdout = io::stdout();
    emit_normal_to(&mut stdout.lock(), bytes)
}
pub(super) fn emit(value: &impl Serialize, cap: usize) -> Result<(), String> {
    let bytes = json_bytes(value, cap)?;
    let stderr = io::stderr();
    let mut out = stderr.lock();
    out.write_all(ROW_PREFIX)
        .and_then(|_| out.write_all(&bytes))
        .and_then(|_| out.flush())
        .map_err(|error| error.to_string())
}
#[derive(Clone, Debug, Serialize)]
pub(super) struct Sample {
    pub(super) ordinal: usize,
    pub(super) calibration: bool,
    pub(super) elapsed_ns: u64,
    pub(super) normal_bytes: usize,
    pub(super) normal_sha256: String,
    pub(super) exact_external_oracle_equal: bool,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub(super) struct Stats {
    pub(super) p50_ns: u64,
    pub(super) p95_ns: u64,
    pub(super) max_ns: u64,
}
pub(super) fn stats(samples: &[Sample]) -> Result<Stats, String> {
    if samples.len() != 35 {
        return Err("expected exactly35 original samples".into());
    }
    let mut times = [0_u64; 30];
    for (index, sample) in samples.iter().enumerate() {
        if sample.ordinal != index + 1
            || sample.calibration != (index < 5)
            || !sample.exact_external_oracle_equal
        {
            return Err("sample order/calibration/oracle refusal".into());
        }
        if index >= 5 {
            times[index - 5] = sample.elapsed_ns;
        }
    }
    times.sort_unstable();
    Ok(Stats {
        p50_ns: times[14],
        p95_ns: times[28],
        max_ns: times[29],
    })
}
