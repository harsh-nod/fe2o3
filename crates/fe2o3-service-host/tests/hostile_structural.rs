use fe2o3_host_api::*;
use fe2o3_service_host::*;
use fe2o3_service_model::{
    AbaStateV1, AllocationBindingInputV1, AllocationRoleV1, DeliveryPolicyV1, EvidenceStatusV1,
    FailureDispositionV1, FailureRecordV1, GenerationCounterV1, GenerationStateV1,
    IdentityDigestV1, LifecycleStateV1 as ModelLifecycleV1, PersistentPlanIdV1, PropertyClaimsV1,
    QueueSlotRecordV1, QueueSlotStateV1, SchedulerModelIdV1, ServiceExecutableIdV1,
    ServiceModelConfigV1, ServicePropertyV1, ServiceRunIdV1, ServiceRunInputV1, ServiceStateV1,
    SlotIdV1, TaskIdV1, TaskSchemaIdV1 as ModelTaskSchemaIdV1,
};

fn bytes(value: u64) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes[..8].copy_from_slice(&value.to_be_bytes());
    bytes[8..16].copy_from_slice(&value.wrapping_mul(17).to_be_bytes());
    bytes
}

fn host_digest(value: u64) -> HostDigestV1 {
    HostDigestV1::from_untrusted_bytes(bytes(value))
}

fn model_digest(value: u64) -> IdentityDigestV1 {
    IdentityDigestV1::from_untrusted_bytes(bytes(value))
}

macro_rules! host_id {
    ($type:ident, $value:expr) => {
        $type::from_untrusted_digest(host_digest($value))
    };
}

fn context(scope: FlowScopeIdV1, value: u64) -> OperationContextV1 {
    OperationContextV1::new(scope, host_id!(OperationIdV1, value), 1, None, vec![]).unwrap()
}

fn payload(value: u64) -> PayloadDescriptorV1 {
    PayloadDescriptorV1::new(
        host_id!(PayloadIdV1, value),
        host_id!(PayloadFormatIdV1, value + 1),
        64,
    )
    .unwrap()
}

fn accepted_load(scope: FlowScopeIdV1, runtime_context_seed: u64) -> LoadResultV1 {
    let compile_request = CompileRequestV1::new(
        host_id!(CompileRequestIdV1, 100),
        context(scope, 101),
        payload(102),
        host_id!(CompilerProfileIdV1, 103),
        host_id!(TargetProfileIdV1, 104),
        host_id!(CompileConfigurationIdV1, 105),
        128,
    )
    .unwrap();
    let candidate = payload(106);
    let compile_result = CompileResultV1::new(
        host_id!(CompileResultIdV1, 107),
        &compile_request,
        CompileOutcomeV1::Candidate(candidate),
        vec![],
    )
    .unwrap();
    let admit_request = AdmitRequestV1::new(
        host_id!(AdmitRequestIdV1, 108),
        context(scope, 109),
        &compile_result,
        candidate,
        host_id!(AdmissionPolicyIdV1, 110),
        vec![host_id!(ClaimIdV1, 111)],
    )
    .unwrap();
    let admit_result = AdmitResultV1::new(
        host_id!(AdmitResultIdV1, 112),
        &admit_request,
        AdmitOutcomeV1::Accepted {
            assessment_identity: host_id!(AdmissionAssessmentIdV1, 113),
        },
        vec![],
    )
    .unwrap();
    let load_request = LoadRequestV1::new(
        host_id!(LoadRequestIdV1, 114),
        context(scope, 115),
        &admit_result,
        candidate.identity(),
        host_id!(LoaderProfileIdV1, 116),
        host_id!(RuntimeContextIdV1, runtime_context_seed),
    )
    .unwrap();
    LoadResultV1::new(
        host_id!(LoadResultIdV1, 117),
        &load_request,
        LoadOutcomeV1::Loaded {
            loaded_object_identity: host_id!(LoadedObjectIdV1, 118),
            load_generation: 7,
        },
        vec![],
    )
    .unwrap()
}

struct Fixture {
    config: ServiceModelConfigV1,
    run: ServiceRunInputV1,
    claims: PropertyClaimsV1,
    plan: PersistentPlanIdV1,
    service_instance: ServiceInstanceIdV1,
    host_schema: fe2o3_host_api::TaskSchemaIdV1,
    load: LoadResultV1,
    scope: FlowScopeIdV1,
}

impl Fixture {
    fn new() -> Self {
        let scope = host_id!(FlowScopeIdV1, 1);
        let queue_identity = model_digest(20);
        let model_schema = ModelTaskSchemaIdV1::from_untrusted_digest(model_digest(21));
        let config = ServiceModelConfigV1 {
            run_id: ServiceRunIdV1::from_untrusted_digest(model_digest(22)),
            service_epoch: 9,
            queue_identity,
            task_schema_id: model_schema,
            scheduler_model_id: SchedulerModelIdV1::from_untrusted_digest(model_digest(23)),
            admitted_task_tags: vec![3, 7],
            queue_capacity: 1,
            generation_modulus: 16,
            maximum_live_generation_span: 4,
            delivery_policy: DeliveryPolicyV1::ExactlyOnce,
            failure_model_id: model_digest(24),
        };
        let run = ServiceRunInputV1 {
            executable_id: ServiceExecutableIdV1::from_untrusted_digest(model_digest(25)),
            physical_device_id: model_digest(26),
            runtime_context_id: model_digest(88),
            stream_or_queue_id: model_digest(27),
            service_epoch: config.service_epoch,
            allocations: vec![
                AllocationBindingInputV1 {
                    role: AllocationRoleV1::Queue,
                    ordinal: 0,
                    allocation_identity: queue_identity,
                    allocation_epoch: 13,
                },
                AllocationBindingInputV1 {
                    role: AllocationRoleV1::State,
                    ordinal: 0,
                    allocation_identity: model_digest(28),
                    allocation_epoch: 14,
                },
            ],
            launch_instance_id: model_digest(29),
        };
        let mut claims = PropertyClaimsV1::unsupported();
        claims.set(
            ServicePropertyV1::CancellationSafe,
            EvidenceStatusV1::Checked,
        );
        claims.set(
            ServicePropertyV1::QuiescenceSafe,
            EvidenceStatusV1::Validated,
        );
        claims.set(
            ServicePropertyV1::ServiceProgress,
            EvidenceStatusV1::Contracted,
        );
        Self {
            config,
            run,
            claims,
            plan: PersistentPlanIdV1::from_untrusted_digest(model_digest(30)),
            service_instance: host_id!(ServiceInstanceIdV1, 31),
            host_schema: fe2o3_host_api::TaskSchemaIdV1::from_untrusted_digest(host_digest(21)),
            load: accepted_load(scope, 88),
            scope,
        }
    }

    fn contract(&self) -> ServiceContractV1<'_> {
        ServiceContractV1::new(
            &self.config,
            &self.run,
            &self.claims,
            self.plan,
            self.service_instance,
            self.host_schema,
            &self.load,
        )
        .unwrap()
    }

    fn model_state(&self, lifecycle: ModelLifecycleV1) -> ServiceStateV1 {
        let admission_cutoff = match lifecycle {
            ModelLifecycleV1::Draining | ModelLifecycleV1::Stopping | ModelLifecycleV1::Stopped => {
                Some(0)
            }
            ModelLifecycleV1::Starting
            | ModelLifecycleV1::Running
            | ModelLifecycleV1::Failed(_) => None,
        };
        let failure = match lifecycle {
            ModelLifecycleV1::Failed(disposition) => Some(FailureRecordV1 {
                failure_model_id: self.config.failure_model_id,
                failure_event_id: model_digest(90),
                disposition,
            }),
            _ => None,
        };
        ServiceStateV1 {
            config: self.config.clone(),
            lifecycle,
            admission_cutoff,
            slots: vec![QueueSlotRecordV1 {
                slot_id: SlotIdV1(0),
                generation: GenerationStateV1::Current(GenerationCounterV1 {
                    logical: 0,
                    encoded: 0,
                }),
                aba: AbaStateV1::Protected {
                    oldest_live_generation: None,
                },
                state: QueueSlotStateV1::Empty { generation: 0 },
            }],
            tasks: vec![],
            leases: vec![],
            workers: vec![],
            dependencies: vec![],
            phase_regions: vec![],
            completion_records: vec![],
            failure,
        }
    }

    fn submitted_dispatch(&self, service_epoch: u64) -> (DispatchRequestV1, DispatchResultV1) {
        let request = DispatchRequestV1::new(
            host_id!(DispatchRequestIdV1, 200 + service_epoch),
            context(self.scope, 220 + service_epoch),
            &self.load,
            host_id!(EntryPointIdV1, 201),
            host_id!(DispatchContractIdV1, 202),
            host_id!(ArgumentSetIdV1, 203),
            DispatchKindV1::PersistentTask {
                service_instance_identity: self.service_instance,
                task_schema_identity: self.host_schema,
                task_tag: 3,
                service_epoch,
            },
            vec![],
            vec![],
        )
        .unwrap();
        let result = DispatchResultV1::new(
            host_id!(DispatchResultIdV1, 204 + service_epoch),
            &request,
            DispatchOutcomeV1::Submitted {
                submission_identity: host_id!(DispatchSubmissionIdV1, 205 + service_epoch),
                completion_signal_identity: host_id!(CompletionSignalIdV1, 206 + service_epoch),
            },
            vec![],
        )
        .unwrap();
        (request, result)
    }
}

#[test]
fn stale_ticket_rejects_changed_generation() {
    let fixture = Fixture::new();
    let contract = fixture.contract();
    let mut queue = [0u8; 1];
    let mut state_storage = [0u8; 1];
    let inputs = [0u8; 1];
    let mut outputs = [0u8; 1];
    let resources = ServiceResourcesV1::new(&mut queue, &mut state_storage, &inputs, &mut outputs);
    let starting = prepare(&contract, resources)
        .start(&fixture.model_state(ModelLifecycleV1::Starting))
        .unwrap();
    let running = starting
        .running(&fixture.model_state(ModelLifecycleV1::Running))
        .unwrap();
    let slot = QueueSlotBindingV1::for_slot(&contract, SlotIdV1(0), 0).unwrap();
    let (request, result) = fixture.submitted_dispatch(contract.key().service_epoch());
    let ticket = running
        .submit(TaskIdV1(1), slot, &request, &result)
        .unwrap();
    let stale_key = fixture.config.slot_key(SlotIdV1(0), 1);
    let stale = QueueSlotBindingV1::from_untrusted_parts(stale_key, slot.queue_epoch(), 1);
    assert_eq!(
        ticket.validate_current(stale),
        Err(ServiceHostErrorV1::StaleTicket)
    );
}

#[test]
fn wrong_service_epoch_rejects_submission() {
    let fixture = Fixture::new();
    let contract = fixture.contract();
    let mut queue = ();
    let mut state_storage = ();
    let inputs = ();
    let mut outputs = ();
    let running = prepare(
        &contract,
        ServiceResourcesV1::new(&mut queue, &mut state_storage, &inputs, &mut outputs),
    )
    .start(&fixture.model_state(ModelLifecycleV1::Starting))
    .unwrap()
    .running(&fixture.model_state(ModelLifecycleV1::Running))
    .unwrap();
    let slot = QueueSlotBindingV1::for_slot(&contract, SlotIdV1(0), 0).unwrap();
    let (request, result) = fixture.submitted_dispatch(contract.key().service_epoch() + 1);
    assert_eq!(
        running
            .submit(TaskIdV1(2), slot, &request, &result)
            .unwrap_err(),
        ServiceHostErrorV1::BindingMismatch {
            field: BindingFieldV1::ServiceEpoch
        }
    );
}

#[test]
fn skipped_lifecycle_transition_rejects() {
    let fixture = Fixture::new();
    let contract = fixture.contract();
    let cursor = LifecycleCursorV1::prepared(contract.key());
    assert_eq!(
        cursor
            .transition(contract.key(), LifecyclePhaseV1::Draining)
            .unwrap_err(),
        ServiceHostErrorV1::InvalidLifecycleTransition {
            from: LifecyclePhaseV1::Prepared,
            to: LifecyclePhaseV1::Draining,
        }
    );
}

#[test]
fn double_terminal_transition_rejects() {
    let fixture = Fixture::new();
    let contract = fixture.contract();
    let key = contract.key();
    let cursor = LifecycleCursorV1::prepared(key)
        .transition(key, LifecyclePhaseV1::Starting)
        .unwrap()
        .transition(key, LifecyclePhaseV1::Running)
        .unwrap()
        .transition(key, LifecyclePhaseV1::Draining)
        .unwrap()
        .transition(key, LifecyclePhaseV1::Stopping)
        .unwrap()
        .transition(key, LifecyclePhaseV1::Stopped)
        .unwrap();
    assert_eq!(
        cursor
            .transition(key, LifecyclePhaseV1::Stopped)
            .unwrap_err(),
        ServiceHostErrorV1::TerminalLifecycleTransition {
            phase: LifecyclePhaseV1::Stopped
        }
    );
}

#[test]
fn early_release_representation_rejects() {
    let fixture = Fixture::new();
    let contract = fixture.contract();
    let key = contract.key();
    let running = LifecycleCursorV1::prepared(key)
        .transition(key, LifecyclePhaseV1::Starting)
        .unwrap()
        .transition(key, LifecyclePhaseV1::Running)
        .unwrap();
    assert_eq!(
        running.storage_disposition(),
        StorageDispositionV1::Retained
    );
    assert_eq!(
        running.validate_release(&contract, &fixture.model_state(ModelLifecycleV1::Running)),
        Err(ServiceHostErrorV1::EarlyStorageRelease)
    );
}

#[test]
fn submit_wait_drain_stop_and_release_preserve_exact_records() {
    let fixture = Fixture::new();
    let contract = fixture.contract();
    assert_eq!(contract.cancellation_claim(), EvidenceStatusV1::Checked);
    assert_eq!(contract.quiescence_claim(), EvidenceStatusV1::Validated);
    assert_eq!(contract.progress_claim(), EvidenceStatusV1::Contracted);

    let mut queue = [0u8; 1];
    let mut state_storage = [0u8; 1];
    let inputs = [0u8; 1];
    let mut outputs = [0u8; 1];
    let running = prepare(
        &contract,
        ServiceResourcesV1::new(&mut queue, &mut state_storage, &inputs, &mut outputs),
    )
    .start(&fixture.model_state(ModelLifecycleV1::Starting))
    .unwrap()
    .running(&fixture.model_state(ModelLifecycleV1::Running))
    .unwrap();
    let slot = QueueSlotBindingV1::for_slot(&contract, SlotIdV1(0), 0).unwrap();
    let (dispatch_request, dispatch_result) =
        fixture.submitted_dispatch(contract.key().service_epoch());
    let ticket = running
        .submit(TaskIdV1(3), slot, &dispatch_request, &dispatch_result)
        .unwrap();
    let wait_request = WaitRequestV1::new(
        host_id!(WaitRequestIdV1, 300),
        context(fixture.scope, 301),
        WaitModeV1::All,
        vec![ticket.completion_signal_identity()],
        None,
    )
    .unwrap();
    let completion_record = host_id!(CompletionRecordIdV1, 302);
    let wait_result = WaitResultV1::new(
        host_id!(WaitResultIdV1, 303),
        &wait_request,
        WaitOutcomeV1::Satisfied(vec![CompletionObservationV1::new(
            ticket.completion_signal_identity(),
            completion_record,
            CompletionStatusV1::Succeeded,
        )]),
        vec![],
    )
    .unwrap();
    let completion = ticket.wait(slot, &wait_request, &wait_result).unwrap();
    assert_eq!(completion.completion_record_identity(), completion_record);
    assert_eq!(completion.status(), CompletionStatusV1::Succeeded);

    let stopped = running
        .drain(&fixture.model_state(ModelLifecycleV1::Draining))
        .unwrap()
        .stop(&fixture.model_state(ModelLifecycleV1::Stopping))
        .unwrap()
        .stopped(&fixture.model_state(ModelLifecycleV1::Stopped))
        .unwrap();
    assert_eq!(
        stopped.storage_disposition(),
        StorageDispositionV1::Releasable
    );
    let (queue, state_storage, _, outputs) = stopped.release().into_parts();
    queue[0] = 1;
    state_storage[0] = 2;
    outputs[0] = 3;
    assert_eq!((queue[0], state_storage[0], outputs[0]), (1, 2, 3));
}

#[test]
fn failure_may_access_cannot_release_until_quiesced() {
    let fixture = Fixture::new();
    let contract = fixture.contract();
    let mut queue = ();
    let mut state_storage = ();
    let inputs = ();
    let mut outputs = ();
    let starting = prepare(
        &contract,
        ServiceResourcesV1::new(&mut queue, &mut state_storage, &inputs, &mut outputs),
    )
    .start(&fixture.model_state(ModelLifecycleV1::Starting))
    .unwrap();
    let failed = starting
        .fail(&fixture.model_state(ModelLifecycleV1::Failed(
            FailureDispositionV1::DeviceMayStillAccess,
        )))
        .unwrap();
    let FailedServiceV1::MayStillAccess(failed) = failed else {
        panic!("wrong failure disposition")
    };
    assert_eq!(failed.storage_disposition(), StorageDispositionV1::Retained);
    let quiesced = failed
        .quiesced(&fixture.model_state(ModelLifecycleV1::Failed(
            FailureDispositionV1::DeviceQuiesced,
        )))
        .unwrap();
    assert_eq!(
        quiesced.storage_disposition(),
        StorageDispositionV1::Releasable
    );
    let _ = quiesced.release();
}
