// Read-only retained-root selection. This does not validate journal contents.
macro_rules! producer_input_first_reference_body {
    ($syntax:ident, $root:ident) => {
        $syntax!({
            let (consumer, reference) = match $root.inputs.first()?.request {
                ProducerReadRequestV1::Active(_) => {
                    let first = *$root.references.first()?;
                    (first.consumer, ProducerReadReferenceV1::Active(first))
                }
                ProducerReadRequestV1::Queued(_) => {
                    let first = *$root.queued_references.first()?;
                    (first.consumer, ProducerReadReferenceV1::Queued(first))
                }
            };
            Some(FirstProducerReadV1 {
                consumer,
                reference,
            })
        })
    };
}

macro_rules! producer_input_preflight_body {
    ($syntax:ident, $context:ident, $id:ident) => {
        $syntax!({
            let record = $context.submissions.get(&$id);
            let expected = match record {
                Some(record) => record.journal_producer_read,
                None => None,
            };
            let absent = if expected.is_some() {
                Err(ContextVersionJournalErrorV1::InvalidReference)
            } else {
                Ok(None)
            };
            let Some(versions) = &$context.versions else {
                return absent;
            };
            let Some(root) = versions.producer_readers.get(&$id) else {
                return absent;
            };
            let marker = root
                .marker
                .ok_or(ContextVersionJournalErrorV1::InvalidReference)?;
            let launch = root.domain == ProducerReadDomainV1::Launch;
            let consumer = ContextWriterKeyV1 {
                context_generation: $id.context_generation,
                local: $id.local,
                kind: ContextWriterKindV1::Submission,
            };
            if marker.count == 0
                || marker.count != root.inputs.len()
                || Some(marker.count)
                    != root
                        .references
                        .len()
                        .checked_add(root.queued_references.len())
                || root.references.len() != root.requests.len()
                || root.queued_references.len() != root.queued_requests.len()
                || Some(marker.first) != root.first_reference()
                || marker.active
                    != match root.references.first() {
                        Some(first) => Some((*first, root.references.len())),
                        None => None,
                    }
                || marker.queued
                    != match root.queued_references.first() {
                        Some(first) => Some((*first, root.queued_references.len())),
                        None => None,
                    }
                || marker.first.consumer != consumer
                || !launch && !root.queued_requests.is_empty()
                || !launch && versions.submission_readers.contains_key(&$id)
                || match record {
                    Some(record) => {
                        record.producer_launch != launch
                            || !launch
                                && (!record.directed_peer_copy || record.journal_read.is_some())
                            || expected != Some(marker)
                            || record.journal_writer
                                != match versions.submission_writers.get(&$id) {
                                    Some(root) => Some(root.writer),
                                    None => None,
                                }
                    }
                    None => false,
                }
                || match versions.submission_writers.get(&$id) {
                    Some(root) => root.domain != SubmissionWriterDomainV1::Ordinary,
                    None => false,
                }
                || !launch && !versions.submission_writers.contains_key(&$id)
            {
                return Err(ContextVersionJournalErrorV1::InvalidReference);
            }
            Ok(Some(ProducerInputRootV1 {
                versions,
                root,
                consumer,
                launch,
            }))
        })
    };
}
