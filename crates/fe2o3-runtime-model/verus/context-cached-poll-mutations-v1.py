"""Construct body-only cached-prefix mutations; no execution or acceptance."""
import hashlib

def need(value, message):
    if not value:
        raise ValueError(message)

CASES = [
    ("generation_check_inverted", "cached_submission_record_body_v1",
     "$submission.id.context_generation != $context.context_generation",
     "$submission.id.context_generation == $context.context_generation", ["submission_record"]),
    ("backend_identity_omitted", "cached_submission_record_body_v1",
     "record.backend_submission != $submission.backend_submission", "false", ["submission_record"]),
    ("stream_identity_omitted", "cached_submission_record_body_v1",
     "record.stream != $submission.stream", "false", ["submission_record"]),
    ("device_identity_omitted", "cached_submission_record_body_v1",
     "record.device != $submission.device", "false", ["submission_record"]),
    ("live_stream_device_omitted", "cached_live_submission_body_v1",
     "stream.device != record.device", "false", ["live_submission_record"]),
    ("stream_hold_call_omitted", "cached_poll_prefix_body_v1",
     "$context.require_stream_unheld_v1(record.stream)?;", "", ["poll_cached_status_v1"]),
    ("held_stream_accepted", "cached_unheld_stream_body_v1",
     "record.unpublished.is_some()", "false", ["unheld_stream_v1"]),
    ("cached_token_precedes_identity", "cached_poll_prefix_body_v1",
     "let record = $context.live_submission_record($submission)?;",
     "if let Some(status) = $submission.completion { return Ok(Some(status)); }\n            let record = $context.live_submission_record($submission)?;",
     ["poll_cached_status_v1"]),
    ("pending_marked_terminal", "cached_status_terminal_body_v1",
     "!matches!($status, Self::Pending)", "matches!($status, Self::Pending)", ["is_terminal"]),
    ("backend_failure_as_success", "cached_status_legacy_body_v1",
     "RuntimePollV1::Failed { code }", "RuntimePollV1::Succeeded", ["legacy_poll"]),
    ("cancelled_as_success", "cached_status_legacy_body_v1",
     "RuntimePollV1::Failed {\n                    code: RUNTIME_CANCELLED_CODE_V1,\n                }",
     "RuntimePollV1::Succeeded", ["legacy_poll"]),
    ("quiescent_as_success", "cached_status_legacy_body_v1",
     "RuntimePollV1::Failed {\n                    code: RUNTIME_QUIESCENT_WITHOUT_RESULT_CODE_V1,\n                }",
     "RuntimePollV1::Succeeded", ["legacy_poll"]),
    ("token_id_local_corrupted", "cached_observe_status_body_v1",
     "let observation = $status.legacy_poll();",
     "$submission.id.local = 0;\n            let observation = $status.legacy_poll();", ["observe_status"]),
    ("token_backend_identity_corrupted", "cached_observe_status_body_v1",
     "let observation = $status.legacy_poll();",
     "$submission.backend_submission = 0;\n            let observation = $status.legacy_poll();", ["observe_status"]),
    ("pending_completion_overwritten", "cached_observe_status_body_v1",
     "if $status.is_terminal() {", "if true {", ["observe_status"]),
    ("terminal_completion_omitted", "cached_observe_status_body_v1",
     "$submission.completion = Some(observation);", "$submission.completion = None;", ["observe_status"]),
]


def mutate(original, macro, before, after):
    marker = "macro_rules! " + macro + " {"
    need(original.count(marker) == 1, "one declared macro")
    start = original.index(marker)
    end = original.find("\nmacro_rules! ", start + len(marker))
    if end < 0:
        end = len(original)
    selected = original[start:end]
    need(before != after and selected.count(before) == 1, "one intended executable anchor")
    changed = selected.replace(before, after, 1)
    return original[:start] + changed + original[end:]


def mutations(original):
    need(hashlib.sha256(original.encode()).hexdigest() == "2b01dbf17b6a868d787781e31c90e5453a47487f4631a77efc3a625c23d00e76",
         "exact qualified body")
    return {name: dict(text=mutate(original, macro, before, after), macro=macro,
                      before=before, after=after, intended_declarations=declarations,
                      compiler_validated=False, solver_ran=False, accepted=False)
            for name, macro, before, after, declarations in CASES}
