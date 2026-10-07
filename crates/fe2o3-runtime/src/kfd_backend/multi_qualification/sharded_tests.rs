//! Exact indexed authority and scripted custody, not device arithmetic evidence.

use super::super::*;
use crate::kfd_backend::kfd_backend_sdma_seam::{
    ScriptedBufferKindV1, ScriptedExecutionOutcomeV1, ScriptedFailureModeV1,
    ScriptedRecycleOutcomeV1, ScriptedSdmaStepV1,
};
use crate::qualification_gfx942_sharded_vecadd_v1::*;
use crate::qualification_gfx942_vecadd_v1::{
    GFX942_VECADD_QUALIFICATION_ARGUMENTS_V1, gfx942_vecadd_qualification_hsaco_v1,
};
use crate::{RuntimeContextV1, RuntimePollV1};
use core::mem::ManuallyDrop;

#[test]
fn sharded_constructor_rejects_invalid_rosters_and_rounds_before_native_open() {
    let invalid = [
        Vec::new(),
        vec![1],
        (1..=9).collect(),
        vec![0, 2],
        vec![1, 0],
        vec![1, 1],
        vec![1, 2, 3, 4, 5, 6, 7, 0],
        vec![1, 2, 3, 4, 5, 6, 7, 1],
    ];
    for ids in invalid {
        for round in [0, 1, 2, usize::MAX] {
            let error =
                KfdMultiDeviceRuntimeBackendV1::open_gfx942_sharded_vecadd_peer_qualification_v1(
                    &ids, round,
                )
                .unwrap_err();
            assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::InvalidLaunch);
        }
    }
    for count in 2..=8 {
        let ids: Vec<_> = (1..=count).collect();
        for round in [2, usize::MAX] {
            let error =
                KfdMultiDeviceRuntimeBackendV1::open_gfx942_sharded_vecadd_peer_qualification_v1(
                    &ids, round,
                )
                .unwrap_err();
            // A native open would produce Native, not a finite-recipe rejection.
            assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::InvalidLaunch);
        }
    }
}

#[test]
fn indexed_qualification_preflight_preserves_recipe_order_and_failure_boundary() {
    let mut calls = Vec::new();
    let error = super::admit_indexed_qualification_devices_v1(&[9, 4, 9], 0, |n, i, r| {
        calls.push((n, i, r));
        Ok(KfdRuntimeLaunchGateV1::WorkerV3GeneratedOnly)
    })
    .unwrap_err();
    assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::InvalidLaunch);
    assert!(calls.is_empty());

    let ids = [30, 10, 20];
    let gates = super::admit_indexed_qualification_devices_v1(&ids, 1, |n, i, r| {
        calls.push((n, i, r));
        Ok(KfdRuntimeLaunchGateV1::WorkerV3GeneratedOnly)
    })
    .unwrap();
    assert_eq!(calls, [(3, 0, 1), (3, 1, 1), (3, 2, 1)]);
    assert_eq!(gates.iter().map(|(uid, _)| *uid).collect::<Vec<_>>(), ids);

    calls.clear();
    let error = super::admit_indexed_qualification_devices_v1(&ids, 0, |n, i, r| {
        calls.push((n, i, r));
        if i == 1 {
            Err(KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "indexed fixture rejection",
            ))
        } else {
            Ok(KfdRuntimeLaunchGateV1::WorkerV3GeneratedOnly)
        }
    })
    .unwrap_err();
    assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::InvalidLaunch);
    assert_eq!(calls, [(3, 0, 0), (3, 1, 0)]);
}

fn initial_bytes(recipe: Gfx942ShardedVecaddQualificationRecipeV1) -> [Vec<u8>; 3] {
    core::array::from_fn(|binding| {
        let mut bytes = Vec::with_capacity(recipe.padded_bytes());
        for index in 0..recipe.padded_bytes() / 4 {
            let value = match binding {
                0 if index < recipe.elements() => {
                    (recipe.global_offset() + index + 131_072 * recipe.round()) as f32
                }
                0 => -7.0,
                1 if index < recipe.elements() => (3 + recipe.round()) as f32,
                1 => -11.0,
                _ => -1.0,
            };
            bytes.extend_from_slice(&value.to_bits().to_le_bytes());
        }
        bytes
    })
}

struct ExactRequest {
    recipe: Gfx942ShardedVecaddQualificationRecipeV1,
    signature: [u8; 32],
    kernarg: [u8; 48],
    bytes: [Vec<u8>; 3],
    bindings: [BackendBindingV1; 3],
    abi: [KfdRuntimeAuthorityGlobalBufferV1<'static>; 3],
}

impl ExactRequest {
    fn new(admitted: &AdmittedGfx942ShardedVecaddQualificationV1) -> Self {
        let recipe = admitted.recipe();
        Self {
            recipe,
            signature: admitted.signature(),
            kernarg: admitted.explicit_kernarg(),
            bytes: initial_bytes(recipe),
            bindings: core::array::from_fn(|index| BackendBindingV1 {
                kernarg_byte_offset: GFX942_VECADD_QUALIFICATION_ARGUMENTS_V1[index].pointer_offset,
                region: BackendMemoryRegionV1 {
                    allocation: 10 + index as u64,
                    access: GFX942_VECADD_QUALIFICATION_ARGUMENTS_V1[index].access,
                    byte_offset: 0,
                    byte_len: recipe.padded_bytes() as u64,
                },
            }),
            abi: GFX942_VECADD_QUALIFICATION_ARGUMENTS_V1.map(|policy| {
                KfdRuntimeAuthorityGlobalBufferV1 {
                    explicit_argument_index: policy.explicit_argument_index,
                    name: policy.name,
                    kernarg_byte_offset: u64::from(policy.pointer_offset),
                    pointee_alignment: policy.reconciled_pointee_alignment,
                    access: if policy.access == RuntimeAccessV1::Read {
                        ArgumentAccess::ReadOnly
                    } else {
                        ArgumentAccess::WriteOnly
                    },
                }
            }),
        }
    }

    fn authorize(&self, gate: &KfdRuntimeLaunchGateV1) -> bool {
        let allocations =
            core::array::from_fn::<_, 3, _>(|index| KfdRuntimeAuthorityAllocationV1 {
                allocation: 10 + index as u64,
                kind: RuntimeMemoryKindV1::DeviceLocal,
                alignment: 4096,
                byte_offset: 0,
                bytes: &self.bytes[index],
                content_sha256: Some(Sha256::digest(&self.bytes[index]).into()),
            });
        let image = gfx942_vecadd_qualification_hsaco_v1();
        gate.authorize_launch_v1(KfdRuntimeAuthorityRequestV1 {
            module_image: image,
            module_sha256: Sha256::digest(image).into(),
            kernel_name: "vecadd",
            signature: self.signature,
            explicit_kernarg: &self.kernarg,
            complete_kernarg_template: &self.kernarg,
            bindings: &self.bindings,
            dispatch_abi: &self.abi,
            allocations: &allocations,
            geometry: self.recipe.geometry(),
            semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
        })
    }
}

#[test]
fn indexed_shard_gates_reject_sibling_data_and_consume_only_their_own_launch() {
    for count in [2, 3, 5, 8] {
        for round in 0..2 {
            let ids: Vec<_> = (0..count).map(|i| 100 + (count - i) as u64).collect();
            let gates = super::admit_indexed_qualification_devices_v1(
                &ids,
                round,
                super::admit_sharded_vecadd_v1,
            )
            .unwrap();
            let mut requests = Vec::new();
            let mut observations = Vec::new();
            for (index, (uid, gate)) in gates.iter().enumerate() {
                assert_eq!(*uid, ids[index]);
                let KfdRuntimeLaunchGateV1::ExactGfx942ShardedVecadd(admitted) = gate else {
                    panic!("each child must retain its indexed exact gate")
                };
                let recipe = admitted.recipe();
                assert_eq!(
                    (recipe.count(), recipe.index(), recipe.round()),
                    (count, index, round)
                );
                requests.push(ExactRequest::new(admitted));
                observations.push(admitted.observation_v1());
            }
            for (index, (_, gate)) in gates.iter().enumerate() {
                assert!(!requests[(index + 1) % count].authorize(gate));
                assert!(!observations[index].accepted_v1());
                assert!(requests[index].authorize(gate));
                assert!(!requests[index].authorize(gate));
                for (child, observation) in observations.iter().enumerate() {
                    assert_eq!(observation.accepted_v1(), child <= index);
                    assert_eq!(
                        observation.authorization_calls_v1(),
                        if child <= index { 3 } else { 0 }
                    );
                }
            }
        }
    }
}

fn release_steps(bytes: usize) -> [ScriptedSdmaStepV1; 8] {
    [
        ScriptedSdmaStepV1::Allocate {
            kind: ScriptedBufferKindV1::Host,
            byte_len: bytes,
        },
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len: bytes,
        },
        ScriptedSdmaStepV1::Submit {
            direction: Gfx942PersistentSdmaDirectionV1::HostToDevice,
            host_offset: 0,
            device_offset: 0,
            copy_bytes: bytes as u32,
            outcome: ScriptedFailureModeV1::Success,
        },
        ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ]
}

#[test]
fn sharded_context_joins_all_exact_computes_and_restores_original_padded_owners() {
    for count in [2, 3, 5, 8] {
        for round in 0..2 {
            let ids: Vec<_> = (0..count).map(|i| 100 + (count - i) as u64).collect();
            let gates = super::admit_indexed_qualification_devices_v1(
                &ids,
                round,
                super::admit_sharded_vecadd_v1,
            )
            .unwrap();
            let mut recipes = Vec::new();
            let mut observations = Vec::new();
            let children = gates
                .into_iter()
                .map(|(uid, gate)| {
                    let KfdRuntimeLaunchGateV1::ExactGfx942ShardedVecadd(admitted) = &gate else {
                        unreachable!()
                    };
                    recipes.push(admitted.recipe());
                    observations.push(admitted.observation_v1());
                    let mut child = KfdRuntimeBackendV1::mock();
                    child.description.backend_device = uid;
                    child.launch_gate = gate;
                    child
                })
                .collect();
            let backend = KfdMultiDeviceRuntimeBackendV1::from_backends(children).unwrap();
            let mut context = ManuallyDrop::new(
                RuntimeContextV1::open_with_version_journal_members_v1(
                    backend,
                    count * 3,
                    count * 2,
                    count * 8,
                )
                .unwrap(),
            );
            let mut allocations = Vec::new();
            let mut streams = Vec::new();
            let mut modules = Vec::new();
            let mut kernels = Vec::new();
            for (index, recipe) in recipes.iter().copied().enumerate() {
                let device = context.devices()[index].id();
                streams.push(context.create_stream(device).unwrap());
                let buffers = initial_bytes(recipe);
                let allocated = core::array::from_fn::<_, 3, _>(|binding| {
                    let allocation = context
                        .allocate(
                            device,
                            RuntimeMemoryKindV1::DeviceLocal,
                            recipe.padded_bytes() as u64,
                            4096,
                        )
                        .unwrap();
                    context
                        .write_allocation(allocation, 0, &buffers[binding])
                        .unwrap();
                    allocation
                });
                allocations.push(allocated);
                let module = context
                    .load_module(device, gfx942_vecadd_qualification_hsaco_v1())
                    .unwrap();
                kernels.push(
                    context
                        .resolve_kernel::<Gfx942ShardedVecaddQualificationArgumentsV1>(
                            module, "vecadd",
                        )
                        .unwrap(),
                );
                modules.push(module);
            }
            let mut original_ids = Vec::new();
            for (index, child) in context
                .backend_mut_for_test_v1()
                .children
                .iter_mut()
                .enumerate()
            {
                let bytes = recipes[index].padded_bytes();
                let driver = ScriptedSdmaDriverV1::new((0..3).flat_map(|_| release_steps(bytes)));
                let mut local_ids: Vec<_> = child.allocations.keys().copied().collect();
                local_ids.sort_unstable();
                assert_eq!(local_ids.len(), 3);
                let mut identities = Vec::new();
                for allocation in local_ids {
                    let record = child.allocations.get_mut(&allocation).unwrap();
                    let mut owner = driver.test_device_owner(bytes);
                    owner
                        .scripted_bytes_mut()
                        .unwrap()
                        .copy_from_slice(&record.bytes);
                    identities.push((allocation, owner.scripted_owner_id().unwrap()));
                    let authenticated_sha256 = Sha256::digest(&record.bytes).into();
                    let DirectionalSdmaDeviceOwnerV1::Scripted(device) = owner else {
                        unreachable!()
                    };
                    record.content_sha256 = Some(authenticated_sha256);
                    record.sdma_storage = KfdRuntimeSdmaStorageV1::H2dReady(Box::new(
                        PersistentComputeReadyStorageV1 {
                            owner: PersistentComputeReadyOwnerV1::Scripted {
                                device,
                                authenticated_sha256,
                            },
                            promotion: None,
                        },
                    ));
                    record.sdma_backed = true;
                    record.sdma_initialized = true;
                    record.sdma_shadow_dirty = false;
                }
                original_ids.push(identities);
                child.native_available = true;
                child.sdma_enabled = true;
                child.scripted_sdma = Some(driver);
                child.scripted_persistent_poll_pending_observations = 2;
            }
            if count == 2 {
                assert_eq!(
                    [recipes[0].padded_bytes(), recipes[1].padded_bytes()],
                    [135_168, 131_072]
                );
            }
            let mut submissions = Vec::new();
            for index in 0..count {
                let [left, right, output] = allocations[index];
                let arguments = Gfx942ShardedVecaddQualificationArgumentsV1::new(
                    recipes[index],
                    left,
                    right,
                    output,
                )
                .unwrap();
                submissions.push(
                    context
                        .launch(
                            streams[index],
                            &kernels[index],
                            &arguments,
                            recipes[index].geometry(),
                            &[],
                        )
                        .unwrap(),
                );
                for (child, observation) in observations.iter().enumerate() {
                    assert_eq!(observation.accepted_v1(), child <= index);
                    assert_eq!(
                        observation.authorization_calls_v1(),
                        u64::from(child <= index)
                    );
                }
            }
            for child in &context.backend().children {
                assert!(child.active.is_some());
                assert!(
                    child
                        .allocations
                        .ordinary_iter()
                        .all(|(_, allocation)| matches!(
                            allocation.sdma_storage,
                            KfdRuntimeSdmaStorageV1::ComputeInFlight { .. }
                        ))
                );
            }
            for (index, submission) in submissions.iter_mut().enumerate() {
                context.flush_stream(streams[index]).unwrap();
                assert_eq!(
                    context.wait(submission, Duration::from_secs(1)).unwrap(),
                    RuntimePollV1::Succeeded
                );
                for observation in &observations {
                    assert!(observation.accepted_v1());
                    assert_eq!(observation.authorization_calls_v1(), 1);
                }
                for pending in &context.backend().children[index + 1..] {
                    assert!(pending.active.is_some());
                    assert!(
                        pending
                            .allocations
                            .ordinary_iter()
                            .all(|(_, allocation)| matches!(
                                allocation.sdma_storage,
                                KfdRuntimeSdmaStorageV1::ComputeInFlight { .. }
                            ))
                    );
                }
                let child = &context.backend().children[index];
                assert!(child.active.is_none());
                assert!(child.pending_compute.is_empty());
                let performance = child.last_launch_performance_v1().unwrap();
                assert_eq!(
                    performance.data_path(),
                    KfdRuntimeLaunchDataPathV1::PersistentDeviceReused
                );
                assert_eq!(performance.user_data_materializations(), 0);
                let before = initial_bytes(recipes[index]);
                for (binding, (allocation, owner_id)) in original_ids[index].iter().enumerate() {
                    let record = &child.allocations[allocation];
                    let (restored_owner, restored_bytes) = match (&record.sdma_storage, binding) {
                        (KfdRuntimeSdmaStorageV1::H2dReady(ready), 0 | 1) => {
                            assert_eq!(
                                ready.owner.byte_len(),
                                recipes[index].padded_bytes() as u64
                            );
                            assert_eq!(ready.owner.physical_byte_len(), ready.owner.byte_len());
                            assert_eq!(
                                ready.owner.authenticated_sha256(),
                                <[u8; 32]>::from(Sha256::digest(&before[binding]))
                            );
                            assert!(ready.promotion.is_none());
                            (
                                ready.owner.scripted_owner_id(),
                                ready.owner.scripted_bytes().unwrap(),
                            )
                        }
                        (KfdRuntimeSdmaStorageV1::Device(owner), 2) => {
                            (owner.scripted_owner_id(), owner.scripted_bytes().unwrap())
                        }
                        _ => panic!(
                            "compute join must preserve ready reads and restore written storage"
                        ),
                    };
                    assert_eq!(restored_owner, Some(*owner_id));
                    assert_eq!(record.scripted_three_binding_replay, binding == 2);
                    assert!(record.persistent_storage_restore.is_none());
                    assert_eq!(record.bytes.len(), recipes[index].padded_bytes());
                    assert!(record.sdma_initialized);
                    // This seam models completion/custody only: it does not execute vecadd.
                    assert_eq!(restored_bytes, before[binding]);
                    if binding == 2 {
                        assert_eq!(record.content_sha256, None);
                        assert!(record.sdma_shadow_dirty);
                    } else {
                        assert_eq!(
                            record.content_sha256,
                            Some(Sha256::digest(&before[binding]).into())
                        );
                    }
                }
            }
            for submission in submissions.into_iter().rev() {
                context.release_submission(submission).unwrap();
            }
            for module in modules {
                context.unload_module(module).unwrap();
            }
            for allocated in allocations {
                for allocation in allocated {
                    context.release_allocation(allocation).unwrap();
                }
            }
            for stream in streams {
                context.destroy_stream(stream).unwrap();
            }
            let mut backend = ManuallyDrop::into_inner(context).shutdown().unwrap();
            backend.shutdown_native_v1().unwrap();
            assert!(backend.allocations.is_empty());
            assert!(backend.submissions.is_empty());
            for child in &backend.children {
                let driver = child.scripted_sdma.as_ref().unwrap();
                assert!(driver.is_exhausted(), "{driver:?}");
                assert_eq!(driver.live_owner_count(), 0);
                assert_eq!(driver.unexpected_drops(), 0);
                assert!(child.queue_retired);
            }
        }
    }
}
