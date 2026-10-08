// Read-only metadata lookup. Journal state and physical completion are not checked.
macro_rules! completion_writer_lookup_body_v1 {
    ($syntax:ident, $submissions:ident, $versions:ident, $id:ident, $domain:ident) => {
        $syntax!({
            let record = $submissions.get(&$id);
            let expected = match record {
                Some(record) => record.journal_writer,
                None => None,
            };
            let absent = if expected.is_some() {
                Err(ContextVersionJournalErrorV1::InvalidReference)
            } else {
                Ok(None)
            };
            let Some(versions) = $versions else {
                return absent;
            };
            if versions.submission_readers.contains_key(&$id)
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
            let Some(root) = versions.submission_writers.get(&$id) else {
                return absent;
            };
            let writer = root.writer;
            if root.domain != $domain {
                return Err(ContextVersionJournalErrorV1::InvalidReference);
            }
            if record.is_some() && expected != Some(writer) {
                return Err(ContextVersionJournalErrorV1::InvalidReference);
            }
            if writer.key.context_generation != $id.context_generation
                || writer.key.local != $id.local
                || writer.key.kind != ContextWriterKindV1::Submission
            {
                return Err(ContextVersionJournalErrorV1::InvalidReference);
            }
            Ok(Some(writer))
        })
    };
}
