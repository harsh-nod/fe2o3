//! Bounded diagnostic projection of the ordinary public recipe Attempt.
//! Never a recipe decoder, canonical owner, receipt import or authority format.
use rustc_codegen_fe2o3::{
    SourceLocalOrderConstraintOutcomeV1 as Constraint,
    SourceLocalOrderIdentityObservationV1 as Identity, SourceLocalOrderOrderV1 as Order,
    SourceLocalOrderRecipeAttemptV1 as Attempt, SourceLocalOrderRecipeEvidenceV1 as Evidence,
    SourceLocalOrderRecipeFailurePhaseV1 as Phase, SourceLocalOrderRelationV1 as Relation,
    SourceLocalOrderSourceBindingModeV1 as Binding, SourceLocalOrderStrengthV1 as Strength,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::io::{self, Write};

pub(super) const PREFIX: &[u8] = b"FE2O3_SOURCE_LOCAL_ORDER_REPORT_V1 ";
pub(super) const LIMIT: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Action {
    Create,
    Replay,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Format {
    Human,
    Json,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Publication {
    NotAttempted,
    Completed,
}

/// Additive exact aliases only; no flag parser, pass list or callback.
pub(super) fn command(value: Option<&str>) -> Option<(Action, Format)> {
    match value {
        Some("create") => Some((Action::Create, Format::Human)),
        Some("replay") => Some((Action::Replay, Format::Human)),
        Some("create-json") => Some((Action::Create, Format::Json)),
        Some("replay-json") => Some((Action::Replay, Format::Json)),
        _ => None,
    }
}
fn phase(value: Phase) -> &'static str {
    match value {
        Phase::Request => "request",
        Phase::Frontend => "frontend",
        Phase::Eligibility => "eligibility",
        Phase::RecipeBinding => "recipe_binding",
        Phase::Continuation => "continuation",
        Phase::Constraint => "constraint",
        Phase::Observation => "observation",
        Phase::SourceCurrentness => "source_currentness",
    }
}
fn order(value: Order) -> &'static str {
    match value {
        Order::SourceOrder => "source_order",
        Order::ReverseReady => "reverse_ready",
    }
}
fn relation(value: Relation) -> &'static str {
    match value {
        Relation::XorBeforeOr => "xor_before_or",
        Relation::OrBeforeXor => "or_before_xor",
    }
}
fn strength(value: Strength) -> &'static str {
    match value {
        Strength::Exact => "exact",
        Strength::Advisory => "advisory",
    }
}
fn binding(value: Binding) -> &'static str {
    match value {
        Binding::ExactRevision => "exact_revision",
        Binding::RebindCurrent => "rebind_current",
    }
}
fn constraint(value: Constraint) -> Value {
    match value {
        Constraint::Honored { relation: actual } => {
            json!({"status":"honored","relation":relation(actual)})
        }
        Constraint::NotHonored { requested, actual } => json!({
            "status":"not_honored","requested":relation(requested),"actual":relation(actual)
        }),
    }
}
fn identity(value: &Identity) -> Value {
    json!({"digest":value.digest(),"canonical_length":value.canonical_length()})
}
fn evidence(value: &Evidence) -> Value {
    // Only public typed getters. No cfg(test) serializer, graph import or inferred stage.
    json!({
        "source_sha256":value.source_sha256(),"source_initializer":value.source_initializer(),
        "semantic_sha256":value.semantic_sha256(),"instance_axes":value.instance_axes(),
        "original":identity(value.original()),"input":identity(value.input()),"output":identity(value.output()),
        "requested_order":order(value.requested_order()),"requested_relation":relation(value.requested_relation()),
        "strength":strength(value.strength()),"source_binding_mode":binding(value.source_binding_mode()),
        "actual_relation":relation(value.actual_relation()),"constraint_outcome":constraint(value.constraint_outcome()),
        "region":value.region(),"output_result_order":value.output_result_order(),
        "prefix_execution_bytes":value.prefix_execution_bytes().as_slice(),
        "transition_sha256":value.transition_sha256(),"transition_bytes":value.transition_bytes(),
        "transition_rows":value.transition_rows(),"fresh_formal_counts":value.fresh_formal_counts(),
        "llvm_sha256":value.llvm_sha256(),"descriptor_sha256":value.descriptor_sha256(),
        "recipe_sha256":value.recipe_sha256(),"canonical_work":value.canonical_work(),
        "canonical_peak_storage":value.canonical_peak_storage(),"created":value.created(),
        "descriptor_producer":value.descriptor_producer(),"composition":value.composition(),
        "grants_authority":value.grants_authority()
    })
}
struct Bounded {
    bytes: Vec<u8>,
    cap: usize,
}
impl Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let size = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("recipe report size overflow"))?;
        if size > self.cap {
            return Err(io::Error::other("recipe report byte cap"));
        }
        self.bytes
            .try_reserve_exact(bytes.len())
            .map_err(io::Error::other)?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn encode(value: &impl Serialize, cap: usize) -> io::Result<Vec<u8>> {
    let mut writer = Bounded {
        bytes: Vec::new(),
        cap,
    };
    serde_json::to_writer(&mut writer, value).map_err(io::Error::other)?;
    writer.write_all(b"\n")?;
    Ok(writer.bytes)
}
fn render(attempt: &Attempt, publication: Publication) -> io::Result<Vec<u8>> {
    let request = attempt.request();
    let result = match attempt.result() {
        Ok(output) => {
            if publication != Publication::Completed {
                return Err(io::Error::other(
                    "successful report requires completed example writes",
                ));
            }
            let value = output.evidence();
            json!({
                "status":"accepted","failure":null,"evidence":evidence(value),
                "llvm_returned":{"bytes":output.llvm_ir().len(),"sha256":value.llvm_sha256()},
                "recipe_returned":output.created_recipe_bytes().map(|bytes|json!({
                    "bytes":bytes.len(),"sha256":value.recipe_sha256()
                })),
                "grants_artifact_or_launch_authority":output.grants_artifact_or_launch_authority()
            })
        }
        Err(error) => {
            if publication != Publication::NotAttempted {
                return Err(io::Error::other(
                    "refused report cannot claim example writes",
                ));
            }
            json!({
                "status":"refused","failure":{"phase":phase(error.phase()),"diagnostic":error.diagnostic(),
                    "compiler_fatal":error.compiler_fatal()},
                "evidence":null,"llvm_returned":null,"recipe_returned":null,
                "grants_artifact_or_launch_authority":false
            })
        }
    };
    encode(
        &json!({
            "schema":"fe2o3-source-local-order-report-v1",
            "authority":"observation_only",
            "request":{"action":if request.is_create() {"create"} else {"replay"},
                "source":request.source_path(),
                "expected_current_source_sha256":request.expected_current_source_sha256()},
            "callback_count":attempt.callback_count(),"compiler_callback_count":attempt.compiler_callback_count(),
            "result":result,
            "publication":{"status":match publication {Publication::NotAttempted=>"not_attempted",Publication::Completed=>"completed"},
                "transactional":false,"output_file_currentness_authenticated":false},
            "timing":null,"complete_owner_memory":null,"execution_authenticated_by_report":false,
            "proof_authority":false,"artifact_authority":false,"launch_authority":false
        }),
        LIMIT,
    )
}
fn write_record(out: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    if bytes.len() > LIMIT || !bytes.ends_with(b"\n") {
        return Err(io::Error::other("bounded LF report required"));
    }
    out.write_all(b"\n")?;
    out.write_all(PREFIX)?;
    out.write_all(bytes)?;
    out.flush()
}
pub(super) fn emit(attempt: &Attempt, publication: Publication) -> io::Result<()> {
    // Complete serialization before any output. A failing stdout write may still
    // leave a partial prefix; command exit and complete framing are mandatory.
    let bytes = render(attempt, publication)?;
    write_record(&mut io::stdout().lock(), &bytes)
}
#[cfg(test)]
#[path = "tests.rs"]
mod tests;
