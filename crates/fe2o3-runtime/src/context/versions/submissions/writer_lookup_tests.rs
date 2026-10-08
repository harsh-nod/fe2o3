use super::*;
use crate::context::versions::readers::RetainedSubmissionReadersV1;

struct Fixture {
    versions: ContextVersionsV1,
    submissions: HashMap<RuntimeSubmissionIdV1, SubmissionRecordV1>,
    id: RuntimeSubmissionIdV1,
    other: RuntimeSubmissionIdV1,
    writer: ContextWriterReferenceV1,
}

fn retained(writer: ContextWriterReferenceV1) -> RetainedSubmissionWriterV1 {
    RetainedSubmissionWriterV1 {
        writer,
        domain: SubmissionWriterDomainV1::Ordinary,
        allocations: Vec::new(),
        members: Vec::new(),
        queued: None,
        disposal_quiescent: false,
        disposal_group: None,
        disposal_started: false,
        disposed_count: 0,
        journal_disposed: false,
    }
}

fn record(writer: ContextWriterReferenceV1, local: u64) -> SubmissionRecordV1 {
    SubmissionRecordV1 {
        backend_submission: 900 + local,
        stream: RuntimeStreamIdV1 {
            context_generation: 7,
            local: 20 + local,
        },
        device: RuntimeDeviceIdV1 {
            context_generation: 7,
            local: 30 + local,
        },
        quiescent: false,
        status: RuntimeCompletionStatusV1::Pending,
        journal_writer: Some(writer),
        journal_read: None,
        journal_producer_read: None,
        scalar_peer_copy: false,
        directed_peer_copy: false,
        producer_launch: false,
        same_device_copy: false,
        segmented_peer_copy: false,
        segmented_destination: None,
        dependency_retains: 0,
    }
}

fn readers() -> RetainedSubmissionReadersV1 {
    RetainedSubmissionReadersV1 {
        domain: SubmissionWriterDomainV1::Ordinary,
        sources: Vec::new(),
        requests: Vec::new(),
        references: Vec::new(),
        marker: None,
    }
}

impl Fixture {
    fn new() -> Self {
        let mut versions = ContextVersionsV1::new(7, 4, 4, 4).unwrap();
        let id = RuntimeSubmissionIdV1 {
            context_generation: 7,
            local: 40,
        };
        let other = RuntimeSubmissionIdV1 {
            context_generation: 7,
            local: 41,
        };
        let mut submissions = HashMap::new();
        let mut first = None;
        for current in [id, other] {
            let writer = versions
                .journal
                .register_writer(ContextWriterKeyV1 {
                    context_generation: current.context_generation,
                    local: current.local,
                    kind: ContextWriterKindV1::Submission,
                })
                .unwrap();
            versions
                .submission_writers
                .insert(current, retained(writer));
            submissions.insert(current, record(writer, current.local));
            if current == id {
                first = Some(writer);
            }
        }
        Self {
            versions,
            submissions,
            id,
            other,
            writer: first.unwrap(),
        }
    }

    fn snapshot(&self) -> String {
        let mut roots: Vec<_> = self.versions.submission_writers.iter().collect();
        roots.sort_by_key(|(id, _)| (id.context_generation, id.local));
        let roots: Vec<_> = roots
            .into_iter()
            .map(|(id, root)| {
                format!(
                    "{id:?}:{:?}:{:?}:{:?}:{:?}:{:?}:{:?}:{:?}:{:?}:{:?}:{:?}:{:?}:{:?}",
                    root.writer,
                    root.domain,
                    root.allocations.as_ptr(),
                    root.members.as_ptr(),
                    root.queued
                        .as_ref()
                        .map(|values| (values.as_ptr(), values.len())),
                    root.disposal_quiescent,
                    root.disposal_group,
                    root.disposal_started,
                    root.disposed_count,
                    root.journal_disposed,
                    root.members,
                    root.queued
                )
            })
            .collect();
        format!(
            "{:?}|{:?}|{:?}|{:?}|{}|{}|{}|{}",
            self.submissions,
            self.versions.journal,
            self.versions.phases,
            roots,
            self.versions.submission_readers.len(),
            self.versions.producer_readers.len(),
            self.versions.disposal_groups.len(),
            self.versions.disposal_allocations.len()
        )
    }

    fn check(
        &self,
        domain: SubmissionWriterDomainV1,
        expected: Result<Option<ContextWriterReferenceV1>, ContextVersionJournalErrorV1>,
    ) {
        let before = self.snapshot();
        assert_eq!(
            preflight_settlement_writer_v1(
                &self.submissions,
                Some(&self.versions),
                self.id,
                domain,
            ),
            expected
        );
        assert_eq!(self.snapshot(), before);
    }
}

#[test]
fn writer_lookup_returns_original_registered_reference_without_settlement() {
    let fixture = Fixture::new();
    fixture.check(SubmissionWriterDomainV1::Ordinary, Ok(Some(fixture.writer)));
    fixture.check(SubmissionWriterDomainV1::Ordinary, Ok(Some(fixture.writer)));
    // Registration is genuine; this read-only result does not activate or settle it.
    assert_eq!(
        fixture.versions.journal.lookup_reserved(fixture.writer),
        Ok(fixture.writer.key)
    );
}

#[test]
fn writer_lookup_absence_preserves_existing_record_error_order() {
    let mut fixture = Fixture::new();
    assert_eq!(
        preflight_settlement_writer_v1(
            &fixture.submissions,
            None,
            fixture.id,
            SubmissionWriterDomainV1::Ordinary,
        ),
        Err(ContextVersionJournalErrorV1::InvalidReference)
    );
    fixture
        .submissions
        .get_mut(&fixture.id)
        .unwrap()
        .journal_writer = None;
    assert_eq!(
        preflight_settlement_writer_v1(
            &fixture.submissions,
            None,
            fixture.id,
            SubmissionWriterDomainV1::Ordinary,
        ),
        Ok(None)
    );
    fixture.versions.submission_writers.remove(&fixture.id);
    fixture.check(SubmissionWriterDomainV1::Ordinary, Ok(None));
    fixture
        .submissions
        .get_mut(&fixture.id)
        .unwrap()
        .journal_writer = Some(fixture.writer);
    fixture.check(
        SubmissionWriterDomainV1::Ordinary,
        Err(ContextVersionJournalErrorV1::InvalidReference),
    );
    fixture.submissions.remove(&fixture.id);
    fixture.check(SubmissionWriterDomainV1::Ordinary, Ok(None));
}

#[test]
fn writer_lookup_retained_root_without_public_record_keeps_original_behavior() {
    let mut fixture = Fixture::new();
    fixture.submissions.remove(&fixture.id);
    fixture.check(SubmissionWriterDomainV1::Ordinary, Ok(Some(fixture.writer)));
}

#[test]
fn writer_lookup_reader_conflict_precedes_missing_writer() {
    let mut fixture = Fixture::new();
    fixture.versions.submission_writers.remove(&fixture.id);
    fixture
        .versions
        .submission_readers
        .insert(fixture.id, readers());
    fixture.check(
        SubmissionWriterDomainV1::Ordinary,
        Err(ContextVersionJournalErrorV1::InvalidState),
    );
    fixture.versions.submission_readers.remove(&fixture.id);
    fixture
        .submissions
        .get_mut(&fixture.id)
        .unwrap()
        .journal_read = Some(SubmissionReaderMarkerV1 {
        first: fe2o3_runtime_model::ContextReadLeaseReferenceV1 {
            slot: 17,
            incarnation: 91,
            consumer: fixture.writer.key,
        },
        count: 1,
    });
    fixture.check(
        SubmissionWriterDomainV1::Ordinary,
        Err(ContextVersionJournalErrorV1::InvalidState),
    );
    // Missing versions is checked before either reader marker, as in the original.
    assert_eq!(
        preflight_settlement_writer_v1(
            &fixture.submissions,
            None,
            fixture.id,
            SubmissionWriterDomainV1::Ordinary,
        ),
        Err(ContextVersionJournalErrorV1::InvalidReference)
    );
}

#[test]
fn writer_lookup_rejects_each_key_axis_after_exact_reference_match() {
    for fault in 0..3 {
        let mut fixture = Fixture::new();
        let root = fixture
            .versions
            .submission_writers
            .get_mut(&fixture.id)
            .unwrap();
        match fault {
            0 => root.writer.key.context_generation += 1,
            1 => root.writer.key.local += 1,
            2 => root.writer.key.kind = ContextWriterKindV1::Synchronous,
            _ => unreachable!(),
        }
        fixture
            .submissions
            .get_mut(&fixture.id)
            .unwrap()
            .journal_writer = Some(root.writer);
        fixture.check(
            SubmissionWriterDomainV1::Ordinary,
            Err(ContextVersionJournalErrorV1::InvalidReference),
        );
    }
    let mut fixture = Fixture::new();
    fixture
        .versions
        .submission_writers
        .get_mut(&fixture.id)
        .unwrap()
        .writer
        .slot += 1;
    fixture.check(
        SubmissionWriterDomainV1::Ordinary,
        Err(ContextVersionJournalErrorV1::InvalidReference),
    );
}

#[test]
fn writer_lookup_binds_complete_generated_domain() {
    let mut fixture = Fixture::new();
    let stream = RuntimeStreamIdV1 {
        context_generation: 7,
        local: 81,
    };
    let domain = SubmissionWriterDomainV1::Generated {
        stream,
        hold: 83,
        shell_key: 89,
    };
    fixture
        .versions
        .submission_writers
        .get_mut(&fixture.id)
        .unwrap()
        .domain = domain;
    fixture.check(domain, Ok(Some(fixture.writer)));
    for wrong in [
        SubmissionWriterDomainV1::Ordinary,
        SubmissionWriterDomainV1::Generated {
            stream: RuntimeStreamIdV1 {
                context_generation: 8,
                ..stream
            },
            hold: 83,
            shell_key: 89,
        },
        SubmissionWriterDomainV1::Generated {
            stream: RuntimeStreamIdV1 {
                local: 82,
                ..stream
            },
            hold: 83,
            shell_key: 89,
        },
        SubmissionWriterDomainV1::Generated {
            stream,
            hold: 84,
            shell_key: 89,
        },
        SubmissionWriterDomainV1::Generated {
            stream,
            hold: 83,
            shell_key: 90,
        },
    ] {
        fixture.check(wrong, Err(ContextVersionJournalErrorV1::InvalidReference));
    }
}

#[test]
fn writer_lookup_ignores_non_target_readers_without_changing_other_record() {
    let mut fixture = Fixture::new();
    fixture
        .versions
        .submission_readers
        .insert(fixture.other, readers());
    fixture.check(SubmissionWriterDomainV1::Ordinary, Ok(Some(fixture.writer)));
    assert_eq!(fixture.submissions[&fixture.other].backend_submission, 941);
    assert!(
        fixture
            .versions
            .submission_readers
            .contains_key(&fixture.other)
    );
}
