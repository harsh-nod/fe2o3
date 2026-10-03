use super::*;
use fe2o3_runtime_model::{
    ContextAllocationReferenceV1, ContextAllocationWriteV1, ContextReadLeaseReferenceV1,
    ContextWriterReferenceV1,
};

// Real Context maps and retained value types, without backend or journal effects.
struct Owner {
    submissions: HashMap<RuntimeSubmissionIdV1, SubmissionRecordV1>,
    versions: Option<ContextVersionsV1>,
}

impl Owner {
    fn select(
        &self,
        id: RuntimeSubmissionIdV1,
    ) -> Result<Option<ProducerInputRootV1<'_>>, ContextVersionJournalErrorV1> {
        producer_input_preflight_body!(completion_journal_rust_syntax, self, id)
    }

    fn root(&mut self) -> &mut RetainedProducerReadV1 {
        self.versions
            .as_mut()
            .unwrap()
            .producer_readers
            .get_mut(&id())
            .unwrap()
    }

    fn snapshot(&self) -> String {
        let versions = self.versions.as_ref().unwrap();
        let root = &versions.producer_readers[&id()];
        format!(
            "{:?}",
            (
                self.submissions.get(&id()),
                (root.marker, &root.references, &root.queued_references),
                (&root.requests, &root.queued_requests),
                (
                    root.inputs.as_ptr(),
                    root.inputs.len(),
                    root.inputs.capacity()
                ),
                (
                    versions.producer_readers.len(),
                    versions.producer_readers.capacity()
                ),
                (
                    versions.submission_writers.len(),
                    versions.submission_readers.len()
                ),
                (
                    versions.journal.retained_read_count(),
                    versions.retained_writers()
                ),
            )
        )
    }
}

fn id() -> RuntimeSubmissionIdV1 {
    RuntimeSubmissionIdV1::new(17, 9)
}
fn consumer() -> ContextWriterKeyV1 {
    ContextWriterKeyV1 {
        context_generation: 17,
        local: 9,
        kind: ContextWriterKindV1::Submission,
    }
}
fn writer() -> ContextWriterReferenceV1 {
    ContextWriterReferenceV1 {
        slot: 2,
        key: consumer(),
    }
}

fn fixture(queued: &[bool], launch: bool, record_present: bool) -> Owner {
    let allocation = ContextAllocationReferenceV1 {
        slot: 0,
        key: ContextAllocationKeyV1 {
            context_generation: 17,
            local: 3,
        },
    };
    let member = ContextAllocationWriteV1 {
        allocation,
        device: ContextJournalDeviceKeyV1 {
            context_generation: 17,
            local: 1,
        },
        byte_extent: 64,
    };
    let source = ContextReadSourceV1 {
        region: RuntimeMemoryRegionV1 {
            allocation: RuntimeAllocationIdV1::new(17, 3),
            byte_offset: 0,
            byte_len: 64,
            access: RuntimeAccessV1::Read,
        },
        record: AllocationRecordV1 {
            backend_allocation: 30,
            device: RuntimeDeviceIdV1::new(17, 1),
            kind: RuntimeMemoryKindV1::DeviceLocal,
            byte_len: 64,
            journal: Some(allocation),
        },
    };
    let dependency = ScalarPeerDependencyV1 {
        ordinal: 0,
        event: RuntimeEventIdV1::new(17, 7),
        backend_event: 70,
        submission: RuntimeSubmissionIdV1::new(17, 5),
        backend_submission: 50,
        stream: RuntimeStreamIdV1::new(17, 2),
        device: source.record.device,
    };
    let mut root = RetainedProducerReadV1 {
        domain: if launch {
            ProducerReadDomainV1::Launch
        } else {
            ProducerReadDomainV1::DirectedPeer
        },
        inputs: Vec::new(),
        requests: Vec::new(),
        references: Vec::new(),
        queued_requests: Vec::new(),
        queued_references: Vec::new(),
        marker: None,
    };
    for is_queued in queued {
        let request = if *is_queued {
            let request = ContextQueuedProducerReadV1 {
                allocation: member,
                byte_offset: 0,
                byte_len: 64,
                producer: writer(),
            };
            root.queued_requests.push(request);
            root.queued_references
                .push(ContextQueuedProducerReadReferenceV1 {
                    slot: root.queued_references.len(),
                    incarnation: 20 + root.queued_references.len() as u64,
                    consumer: consumer(),
                });
            ProducerReadRequestV1::Queued(request)
        } else {
            let request = ContextProducerReadV1 {
                read: ContextAllocationReadV1 {
                    allocation,
                    device: member.device,
                    byte_extent: 64,
                    byte_offset: 0,
                    byte_len: 64,
                    attempt_epoch: 1,
                    content_lineage: 1,
                },
                producer: writer(),
            };
            root.requests.push(request);
            root.references.push(ContextProducerReadReferenceV1 {
                slot: root.references.len(),
                incarnation: 10 + root.references.len() as u64,
                consumer: consumer(),
            });
            ProducerReadRequestV1::Active(request)
        };
        root.inputs.push(ProducerInputV1 {
            source,
            dependency,
            request,
        });
    }
    root.marker = Some(root.complete_marker());
    let marker = root.marker;
    let mut versions = ContextVersionsV1::new(17, 4, 4, 8).unwrap();
    versions.producer_readers.insert(id(), root);
    versions.submission_writers.insert(
        id(),
        submissions::RetainedSubmissionWriterV1 {
            writer: writer(),
            domain: SubmissionWriterDomainV1::Ordinary,
            allocations: Vec::new(),
            members: Vec::new(),
            queued: None,
            disposal_quiescent: false,
            disposal_group: None,
            disposal_started: false,
            disposed_count: 0,
            journal_disposed: false,
        },
    );
    let mut submissions = HashMap::new();
    if record_present {
        submissions.insert(
            id(),
            SubmissionRecordV1 {
                backend_submission: 90,
                stream: RuntimeStreamIdV1::new(17, 2),
                device: source.record.device,
                quiescent: false,
                status: RuntimeCompletionStatusV1::Pending,
                journal_writer: Some(writer()),
                journal_read: None,
                journal_producer_read: marker,
                scalar_peer_copy: !launch,
                directed_peer_copy: !launch,
                producer_launch: launch,
                same_device_copy: false,
                segmented_peer_copy: false,
                segmented_destination: None,
                dependency_retains: 1,
            },
        );
    }
    Owner {
        submissions,
        versions: Some(versions),
    }
}

fn rejected(owner: &Owner) {
    let before = owner.snapshot();
    assert!(matches!(
        owner.select(id()),
        Err(ContextVersionJournalErrorV1::InvalidReference)
    ));
    assert_eq!(owner.snapshot(), before);
}

#[test]
fn producer_input_preflight_absent_and_construction_roots_are_distinct() {
    let empty = Owner {
        submissions: HashMap::new(),
        versions: None,
    };
    assert!(empty.select(id()).unwrap().is_none());
    for record_present in [false, true] {
        let mut owner = fixture(&[false], true, record_present);
        let foreign = RuntimeSubmissionIdV1::new(18, id().local);
        assert!(owner.select(foreign).unwrap().is_none());
        assert!(
            owner
                .select(RuntimeSubmissionIdV1::new(17, 10))
                .unwrap()
                .is_none()
        );
        assert!(owner.select(id()).unwrap().is_some());
        owner.versions = None;
        if record_present {
            assert!(matches!(
                owner.select(id()),
                Err(ContextVersionJournalErrorV1::InvalidReference)
            ));
        } else {
            assert!(owner.select(id()).unwrap().is_none());
        }
        let mut owner = fixture(&[false], true, record_present);
        owner
            .versions
            .as_mut()
            .unwrap()
            .producer_readers
            .remove(&id());
        assert_eq!(owner.select(id()).is_err(), record_present);
    }
}

#[test]
fn producer_input_preflight_preserves_exact_borrowed_roots_without_status_authority() {
    for queued in [
        &[false][..],
        &[true],
        &[false, true],
        &[true, false],
        &[false, true, false],
    ] {
        for record_present in [false, true] {
            let mut owner = fixture(queued, true, record_present);
            if let Some(record) = owner.submissions.get_mut(&id()) {
                record.status =
                    RuntimeCompletionStatusV1::Failed(RuntimeCompletionFailureV1::BackendCode(-7));
                record.quiescent = true;
            }
            let before = owner.snapshot();
            let selected = owner.select(id()).unwrap().unwrap();
            let versions = owner.versions.as_ref().unwrap();
            assert!(std::ptr::eq(selected.versions, versions));
            assert!(std::ptr::eq(
                selected.root,
                &versions.producer_readers[&id()]
            ));
            assert_eq!(selected.consumer, consumer());
            assert!(selected.launch);
            assert_eq!(owner.snapshot(), before);
        }
    }
    let owner = fixture(&[false], false, true);
    assert!(!owner.select(id()).unwrap().unwrap().launch);
    let owner = fixture(&[false], false, false);
    assert!(!owner.select(id()).unwrap().unwrap().launch);
    let mut owner = fixture(&[false], true, false);
    owner.versions.as_mut().unwrap().submission_writers.clear();
    assert!(owner.select(id()).unwrap().is_some());
}

#[test]
fn producer_input_preflight_rejects_each_marker_and_family_mismatch_without_mutation() {
    for case in 0..15 {
        let mut owner = fixture(&[false, true], true, true);
        let root = owner.root();
        match case {
            0 => root.marker = None,
            1 => root.marker.as_mut().unwrap().count = 0,
            2 => root.marker.as_mut().unwrap().count += 1,
            3 => {
                root.inputs.pop();
            }
            4 => {
                root.references.pop();
            }
            5 => {
                root.queued_references.pop();
            }
            6 => {
                root.requests.pop();
            }
            7 => {
                root.queued_requests.pop();
            }
            8 => root.marker.as_mut().unwrap().active = None,
            9 => root.marker.as_mut().unwrap().queued = None,
            10 => {
                root.marker
                    .as_mut()
                    .unwrap()
                    .first
                    .consumer
                    .context_generation += 1
            }
            11 => {
                root.marker.as_mut().unwrap().first.consumer.kind = ContextWriterKindV1::Synchronous
            }
            12 => root.references[0].incarnation += 1,
            13 => root.queued_references[0].incarnation += 1,
            14 => root.inputs.swap(0, 1),
            _ => unreachable!(),
        }
        rejected(&owner);
    }
}

#[test]
fn producer_input_preflight_rejects_record_and_writer_domain_substitution() {
    for case in 0..8 {
        let mut owner = fixture(&[false], false, true);
        match case {
            0 => {
                owner
                    .submissions
                    .get_mut(&id())
                    .unwrap()
                    .journal_producer_read = None
            }
            1 => owner.submissions.get_mut(&id()).unwrap().journal_writer = None,
            2 => owner.submissions.get_mut(&id()).unwrap().producer_launch = true,
            3 => owner.submissions.get_mut(&id()).unwrap().directed_peer_copy = false,
            4 => {
                owner.versions.as_mut().unwrap().submission_writers.clear();
            }
            5 => {
                owner
                    .versions
                    .as_mut()
                    .unwrap()
                    .submission_writers
                    .get_mut(&id())
                    .unwrap()
                    .domain = SubmissionWriterDomainV1::Generated {
                    stream: RuntimeStreamIdV1::new(17, 2),
                    hold: 1,
                    shell_key: 2,
                }
            }
            6 => {
                owner.submissions.get_mut(&id()).unwrap().journal_read =
                    Some(SubmissionReaderMarkerV1 {
                        first: ContextReadLeaseReferenceV1 {
                            slot: 0,
                            incarnation: 1,
                            consumer: consumer(),
                        },
                        count: 1,
                    })
            }
            7 => {
                owner.versions.as_mut().unwrap().submission_readers.insert(
                    id(),
                    readers::RetainedSubmissionReadersV1 {
                        domain: SubmissionWriterDomainV1::Ordinary,
                        sources: Vec::new(),
                        requests: Vec::new(),
                        references: Vec::new(),
                        marker: None,
                    },
                );
            }
            _ => unreachable!(),
        }
        rejected(&owner);
    }
    rejected(&fixture(&[true], false, false));
}
