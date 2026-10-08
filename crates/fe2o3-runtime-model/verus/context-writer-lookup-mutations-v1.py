"""Exact body-only lookup mutations. Construction is not logical acceptance."""
import hashlib

BODY_SHA = "fb21d538e2622078336188f9e65a9ae4ec81c6dfa9e320546620191585a4af77"
PROOF_SHA = "8ca891942dad1dcccf1611d944dbd1ca2dcba0ea07fdca2c4c55999819ad78bd"
MACRO = "completion_writer_lookup_body_v1"
INTENDED = ("preflight_settlement_writer_v1",)
READER_BLOCK = """            if versions.submission_readers.contains_key(&$id)
                || versions.producer_readers.contains_key(&$id)
                || match record {
                    Some(record) => record.journal_read.is_some(),
                    None => false,
                }
                || match record {
                    Some(record) => record.journal_producer_read.is_some(),
                    None => false,
                }
            {
                return Err(ContextVersionJournalErrorV1::InvalidState);
            }
"""
ROOT_BLOCK = """            let Some(root) = versions.submission_writers.get(&$id) else {
                return absent;
            };
"""
VERSIONS_BLOCK = """            let Some(versions) = $versions else {
                return absent;
            };
"""


def need(value, message):
    if not value:
        raise ValueError(message)


def domain_without(ignored):
    fields = ("stream", "hold", "shell_key")
    need(ignored in fields, "one actual Generated domain axis")
    left = ", ".join(field + ": " + ("_" if field == ignored else "left_" + field) for field in fields)
    right = ", ".join(field + ": " + ("_" if field == ignored else "right_" + field) for field in fields)
    checks = " || ".join("left_" + field + " != right_" + field for field in fields if field != ignored)
    return ("match (root.domain, $domain) {\n"
            "                (SubmissionWriterDomainV1::Generated { " + left + " },\n"
            "                 SubmissionWriterDomainV1::Generated { " + right + " }) => " + checks + ",\n"
            "                _ => root.domain != $domain,\n"
            "            }")


CASES = (
    ("expected_writer_erased", "Some(record) => record.journal_writer,", "Some(_record) => None,"),
    ("no_versions_accepts_reference", VERSIONS_BLOCK, VERSIONS_BLOCK.replace("return absent;", "return Ok(None);")),
    ("missing_writer_accepts_reference", ROOT_BLOCK, ROOT_BLOCK.replace("return absent;", "return Ok(None);")),
    ("empty_absence_rejected", "                Ok(None)\n", "                Err(ContextVersionJournalErrorV1::InvalidReference)\n"),
    ("reader_check_after_missing_writer", READER_BLOCK + ROOT_BLOCK, ROOT_BLOCK + READER_BLOCK),
    ("reader_error_is_reference", "return Err(ContextVersionJournalErrorV1::InvalidState);",
     "return Err(ContextVersionJournalErrorV1::InvalidReference);"),
    ("submission_reader_ignored", "versions.submission_readers.contains_key(&$id)", "false"),
    ("producer_reader_ignored", "versions.producer_readers.contains_key(&$id)", "false"),
    ("record_reader_ignored", "Some(record) => record.journal_read.is_some(),", "Some(_record) => false,"),
    ("record_producer_reader_ignored", "Some(record) => record.journal_producer_read.is_some(),", "Some(_record) => false,"),
    ("domain_check_omitted", "root.domain != $domain", "false"),
    ("generated_stream_ignored", "root.domain != $domain", domain_without("stream")),
    ("generated_hold_ignored", "root.domain != $domain", domain_without("hold")),
    ("generated_shell_ignored", "root.domain != $domain", domain_without("shell_key")),
    ("expected_reference_ignored", "record.is_some() && expected != Some(writer)", "false"),
    ("key_generation_ignored", "writer.key.context_generation != $id.context_generation", "false"),
    ("key_local_ignored", "writer.key.local != $id.local", "false"),
    ("key_kind_ignored", "writer.key.kind != ContextWriterKindV1::Submission", "false"),
    ("return_none_for_writer", "Ok(Some(writer))", "Ok(None)"),
    ("return_slot_zero", "Ok(Some(writer))",
     "Ok(Some(ContextWriterReferenceV1 { slot: 0, key: writer.key }))"),
)


def mutate(original, before, after):
    need(isinstance(original, str) and hashlib.sha256(original.encode()).hexdigest() == BODY_SHA,
         "exact qualified Source06 shared body")
    marker = "macro_rules! " + MACRO + " {"
    need(original.count(marker) == 1, "one actual macro declaration")
    start = original.index(marker)
    selected = original[start:]
    need(before != after and selected.count(before) == 1, "one exact executable anchor")
    return original[:start] + selected.replace(before, after, 1)


def mutations(original):
    need(len(CASES) == 20 and len({case[0] for case in CASES}) == 20, "twenty distinct named cases")
    result = {}
    hashes = set()
    for name, before, after in CASES:
        changed = mutate(original, before, after)
        digest = hashlib.sha256(changed.encode()).hexdigest()
        need(digest not in hashes, "distinct actual mutant bytes")
        hashes.add(digest)
        result[name] = dict(text=changed, bytes=len(changed.encode()), mode=0o644, sha256=digest,
                            macro=MACRO, before=before, after=after, body_before_sha256=BODY_SHA,
                            unchanged_proof_sha256=PROOF_SHA, intended_declarations=list(INTENDED),
                            compiler_validated=False, solver_ran=False, state="NOT_RUN", accepted=False)
    return result
