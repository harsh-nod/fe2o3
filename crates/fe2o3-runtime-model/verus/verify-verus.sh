#!/bin/sh
set -eu

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/../../.." && pwd)
lifecycle_proof="$script_dir/runtime_lifecycle_v1.rs"
identity_proof="$script_dir/device_identity_generation_v1.rs"
projection_proof="$script_dir/device_projection_refinement_v1.rs"
memory_proof="$script_dir/memory_lifecycle_v1.rs"
queue_proof="$script_dir/queue_lifecycle_v1.rs"
load_plan_proof="$script_dir/load_plan_v1.rs"
materialization_proof="$script_dir/materialization_v1.rs"
aql_proof="$script_dir/aql_publication_v1.rs"
r7_async_resources_proof="$script_dir/r7_async_resources_v1.rs"
r8_execution_contracts_proof="$script_dir/r8_execution_contracts_v1.rs"
r9_native_evidence_proof="$script_dir/r9_native_evidence_v1.rs"
r10_closed_execution_proof="$script_dir/r10_closed_execution_v1.rs"
r11_runtime_semantics_proof="$script_dir/r11_runtime_semantics_v1.rs"
r12_native_concurrency_proof="$script_dir/r12_native_concurrency_v1.rs"
r13_logical_scheduler_proof="$script_dir/r13_logical_scheduler_v1.rs"
r14_async_observer_proof="$script_dir/r14_async_observer_v1.rs"
r16_worker_semantic_boundary_proof="$script_dir/r16_worker_semantic_boundary_v1.rs"
negative_lifecycle="$script_dir/negative/runtime_lifecycle_v1_release_while_published.rs"
negative_vm="$script_dir/negative/device_identity_generation_v1_vm_substitution.rs"
negative_stale="$script_dir/negative/device_identity_generation_v1_stale_reuse.rs"
negative_render="$script_dir/negative/device_identity_generation_v1_render_substitution.rs"
negative_projection_schema="$script_dir/negative/device_projection_refinement_v1_schema_drop.rs"
negative_projection_history="$script_dir/negative/device_projection_refinement_v1_history_link.rs"
negative_projection_identity="$script_dir/negative/device_projection_refinement_v1_identity_mix.rs"
negative_projection_currentness="$script_dir/negative/device_projection_refinement_v1_currentness_drop.rs"
negative_memory_free="$script_dir/negative/memory_lifecycle_v1_free_while_partial.rs"
negative_memory_unmap="$script_dir/negative/memory_lifecycle_v1_unmap_prefix.rs"
negative_memory_failed_full="$script_dir/negative/memory_lifecycle_v1_failed_full_release.rs"
negative_queue_resource_substitution="$script_dir/negative/queue_lifecycle_v1_resource_substitution.rs"
negative_queue_destroy_ambiguity="$script_dir/negative/queue_lifecycle_v1_destroy_ambiguity.rs"
negative_queue_destroy_source_restore="$script_dir/negative/queue_lifecycle_v1_destroy_source_restore.rs"
negative_queue_history_prefix="$script_dir/negative/queue_lifecycle_v1_history_prefix.rs"
negative_queue_sentinel_returned="$script_dir/negative/queue_lifecycle_v1_sentinel_returned.rs"
negative_queue_publication_owner="$script_dir/negative/queue_lifecycle_v1_publication_owner.rs"
negative_queue_ambiguous_id_reuse="$script_dir/negative/queue_lifecycle_v1_ambiguous_id_reuse.rs"
negative_queue_mapping_generation="$script_dir/negative/queue_lifecycle_v1_mapping_generation.rs"
negative_queue_illegal_ambiguity="$script_dir/negative/queue_lifecycle_v1_illegal_ambiguity.rs"
negative_queue_generic_create_ambiguity="$script_dir/negative/queue_lifecycle_v1_generic_create_ambiguity.rs"
negative_queue_cancel_retention="$script_dir/negative/queue_lifecycle_v1_cancel_retention.rs"
negative_queue_pending_create_overlap="$script_dir/negative/queue_lifecycle_v1_pending_create_overlap.rs"
negative_load_page_overlap="$script_dir/negative/load_plan_v1_page_overlap.rs"
negative_load_descriptor_delta="$script_dir/negative/load_plan_v1_descriptor_delta.rs"
negative_materialization_source="$script_dir/negative/materialization_v1_source_substitution.rs"
negative_materialization_zero="$script_dir/negative/materialization_v1_zero_omission.rs"
negative_aql_vendor_body="$script_dir/negative/aql_publication_v1_vendor_body.rs"
negative_aql_setup_substitution="$script_dir/negative/aql_publication_v1_setup_substitution.rs"
negative_aql_replay="$script_dir/negative/aql_reservation_v1_replay.rs"
negative_aql_read_regression="$script_dir/negative/aql_reservation_v1_read_regression.rs"
negative_aql_full_overwrite="$script_dir/negative/aql_reservation_v1_full_overwrite.rs"
negative_r7_generation_reuse="$script_dir/negative/r7_async_resources_v1_generation_reuse.rs"
negative_r7_cross_device="$script_dir/negative/r7_async_resources_v1_cross_device.rs"
negative_r8_eager_publication="$script_dir/negative/r8_execution_contracts_v1_eager_publication.rs"
negative_r8_conflicting_overlap="$script_dir/negative/r8_execution_contracts_v1_conflicting_overlap.rs"
negative_r8_dependency_polarity="$script_dir/negative/r8_execution_contracts_v1_dependency_polarity.rs"
negative_r8_binding_substitution="$script_dir/negative/r8_execution_contracts_v1_binding_substitution.rs"
negative_r8_generation_substitution="$script_dir/negative/r8_execution_contracts_v1_generation_substitution.rs"
negative_r8_epoch_substitution="$script_dir/negative/r8_execution_contracts_v1_epoch_substitution.rs"
negative_r8_atomic_alignment="$script_dir/negative/r8_execution_contracts_v1_atomic_alignment.rs"
negative_r8_atomic_coherence="$script_dir/negative/r8_execution_contracts_v1_atomic_coherence.rs"
negative_r8_atomic_return="$script_dir/negative/r8_execution_contracts_v1_atomic_return.rs"
negative_r8_early_collective="$script_dir/negative/r8_execution_contracts_v1_early_collective.rs"
negative_r8_duplicate_collective="$script_dir/negative/r8_execution_contracts_v1_duplicate_collective.rs"
negative_r9_duplicate_gpu="$script_dir/negative/r9_native_evidence_v1_duplicate_gpu.rs"
negative_r9_nonzero_begin_prefix="$script_dir/negative/r9_native_evidence_v1_nonzero_begin_prefix.rs"
negative_r9_map_prefix_substitution="$script_dir/negative/r9_native_evidence_v1_map_prefix_substitution.rs"
negative_r9_unmap_prefix_addition="$script_dir/negative/r9_native_evidence_v1_unmap_prefix_addition.rs"
negative_r9_early_compensation_release="$script_dir/negative/r9_native_evidence_v1_early_compensation_release.rs"
negative_r9_incomplete_compensation="$script_dir/negative/r9_native_evidence_v1_incomplete_compensation.rs"
negative_r9_reversed_route="$script_dir/negative/r9_native_evidence_v1_reversed_route.rs"
negative_r9_stale_topology="$script_dir/negative/r9_native_evidence_v1_stale_topology.rs"
negative_r9_reset_fence_drop="$script_dir/negative/r9_native_evidence_v1_reset_fence_drop.rs"
negative_r9_artifact_substitution="$script_dir/negative/r9_native_evidence_v1_artifact_substitution.rs"
negative_r9_receipt_substitution="$script_dir/negative/r9_native_evidence_v1_receipt_substitution.rs"
negative_r9_stale_dispatch="$script_dir/negative/r9_native_evidence_v1_stale_dispatch.rs"
negative_r9_incomplete_dependency="$script_dir/negative/r9_native_evidence_v1_incomplete_dependency.rs"
negative_r9_copy_inactive_mapping="$script_dir/negative/r9_native_evidence_v1_copy_inactive_mapping.rs"
negative_r9_uncertain_copy_release="$script_dir/negative/r9_native_evidence_v1_uncertain_copy_release.rs"
negative_r10_dependency_bypass="$script_dir/negative/r10_closed_execution_v1_dependency_bypass.rs"
negative_r10_partial_batch="$script_dir/negative/r10_closed_execution_v1_partial_batch.rs"
negative_r10_pool_generation="$script_dir/negative/r10_closed_execution_v1_pool_generation.rs"
negative_r10_peer_owner="$script_dir/negative/r10_closed_execution_v1_peer_owner.rs"
negative_r10_cancel_release="$script_dir/negative/r10_closed_execution_v1_cancel_release.rs"
negative_r10_quarantine_release="$script_dir/negative/r10_closed_execution_v1_quarantine_release.rs"
negative_r10_atomic_scope="$script_dir/negative/r10_closed_execution_v1_atomic_scope.rs"
negative_r10_atomic_fence="$script_dir/negative/r10_closed_execution_v1_atomic_fence.rs"
negative_r10_atomic_return="$script_dir/negative/r10_closed_execution_v1_atomic_return.rs"
negative_r10_wave_early="$script_dir/negative/r10_closed_execution_v1_wave_early.rs"
negative_r10_scan_prefix="$script_dir/negative/r10_closed_execution_v1_scan_prefix.rs"
negative_r11_atomic_capability="$script_dir/negative/r11_runtime_semantics_v1_atomic_capability.rs"
negative_r11_callback_redischarge="$script_dir/negative/r11_runtime_semantics_v1_callback_redischarge.rs"
negative_r11_compare_exchange_failure_order="$script_dir/negative/r11_runtime_semantics_v1_compare_exchange_failure_order.rs"
negative_r11_collective_membership="$script_dir/negative/r11_runtime_semantics_v1_collective_membership.rs"
negative_r11_collective_partial_tail="$script_dir/negative/r11_runtime_semantics_v1_collective_partial_tail.rs"
negative_r11_event_substitution="$script_dir/negative/r11_runtime_semantics_v1_event_substitution.rs"
negative_r11_mapping_early_release="$script_dir/negative/r11_runtime_semantics_v1_mapping_early_release.rs"
negative_r11_mapping_uncertain="$script_dir/negative/r11_runtime_semantics_v1_mapping_uncertain.rs"
negative_r12_capability_count="$script_dir/negative/r12_native_concurrency_v1_capability_count.rs"
negative_r12_cross_queue_terminal="$script_dir/negative/r12_native_concurrency_v1_cross_queue_terminal.rs"
negative_r12_currentness_quarantine="$script_dir/negative/r12_native_concurrency_v1_currentness_quarantine.rs"
negative_r12_dependent_release="$script_dir/negative/r12_native_concurrency_v1_dependent_release.rs"
negative_r12_dependency_bypass="$script_dir/negative/r12_native_concurrency_v1_dependency_bypass.rs"
negative_r12_indeterminate_drain="$script_dir/negative/r12_native_concurrency_v1_indeterminate_drain.rs"
negative_r12_published_cancel="$script_dir/negative/r12_native_concurrency_v1_published_cancel.rs"
negative_r12_published_release="$script_dir/negative/r12_native_concurrency_v1_published_release.rs"
negative_r12_queue_recreation="$script_dir/negative/r12_native_concurrency_v1_queue_recreation.rs"
negative_r12_queue_occurrence="$script_dir/negative/r12_native_concurrency_v1_queue_occurrence.rs"
negative_r12_slot_recycle="$script_dir/negative/r12_native_concurrency_v1_slot_recycle.rs"
negative_r12_slot_generation="$script_dir/negative/r12_native_concurrency_v1_slot_generation.rs"
negative_r12_stale_drain="$script_dir/negative/r12_native_concurrency_v1_stale_drain.rs"
negative_r13_currentness_quarantine="$script_dir/negative/r13_logical_scheduler_v1_currentness_quarantine.rs"
negative_r13_dependency_bound="$script_dir/negative/r13_logical_scheduler_v1_dependency_bound.rs"
negative_r13_dependency_bypass="$script_dir/negative/r13_logical_scheduler_v1_dependency_bypass.rs"
negative_r13_dependent_release="$script_dir/negative/r13_logical_scheduler_v1_dependent_release.rs"
negative_r13_fifo_bypass="$script_dir/negative/r13_logical_scheduler_v1_fifo_bypass.rs"
negative_r13_foreign_owner="$script_dir/negative/r13_logical_scheduler_v1_foreign_owner.rs"
negative_r13_foreign_terminal="$script_dir/negative/r13_logical_scheduler_v1_foreign_terminal.rs"
negative_r13_lane_collision="$script_dir/negative/r13_logical_scheduler_v1_lane_collision.rs"
negative_r13_non_tail_cancel="$script_dir/negative/r13_logical_scheduler_v1_non_tail_cancel.rs"
negative_r13_resource_overlap="$script_dir/negative/r13_logical_scheduler_v1_resource_overlap.rs"
negative_r13_third_lane="$script_dir/negative/r13_logical_scheduler_v1_third_lane.rs"
negative_r14_abandon_release="$script_dir/negative/r14_async_observer_v1_abandon_release.rs"
negative_r14_capacity_bound="$script_dir/negative/r14_async_observer_v1_capacity_bound.rs"
negative_r14_duplicate_registration="$script_dir/negative/r14_async_observer_v1_duplicate_registration.rs"
negative_r14_error_substitution="$script_dir/negative/r14_async_observer_v1_error_substitution.rs"
negative_r14_key_order="$script_dir/negative/r14_async_observer_v1_key_order.rs"
negative_r14_pending_removal="$script_dir/negative/r14_async_observer_v1_pending_removal.rs"
negative_r14_status_substitution="$script_dir/negative/r14_async_observer_v1_status_substitution.rs"
negative_r14_stop_cancel="$script_dir/negative/r14_async_observer_v1_stop_cancel.rs"
negative_r16_contract_substitution="$script_dir/negative/r16_worker_semantic_boundary_v1_contract_substitution.rs"
negative_r16_dependency_bound="$script_dir/negative/r16_worker_semantic_boundary_v1_dependency_bound.rs"
negative_r16_handshake_downgrade="$script_dir/negative/r16_worker_semantic_boundary_v1_handshake_downgrade.rs"
negative_r16_pre_custody="$script_dir/negative/r16_worker_semantic_boundary_v1_pre_custody.rs"
negative_r16_reachability="$script_dir/negative/r16_worker_semantic_boundary_v1_reachability.rs"
negative_r16_response_custody="$script_dir/negative/r16_worker_semantic_boundary_v1_response_custody.rs"
negative_r16_sidecar_scope="$script_dir/negative/r16_worker_semantic_boundary_v1_sidecar_scope.rs"
negative_r16_sidecar_substitution="$script_dir/negative/r16_worker_semantic_boundary_v1_sidecar_substitution.rs"
negative_r16_terminal_reopen="$script_dir/negative/r16_worker_semantic_boundary_v1_terminal_reopen.rs"
negative_r16_variant_mismatch="$script_dir/negative/r16_worker_semantic_boundary_v1_variant_mismatch.rs"
pin_dir="$script_dir/pins"
closure_manifest="$pin_dir/VERUS_CLOSURE_MANIFEST"
closure_checker="$repo_root/examples/row_softmax_v1/verify-verus-closure.sh"
source_checker="$repo_root/examples/wave64_collectives_v1/check-proof-source.py"
verus_bin=${VERUS:-verus}

if [ "$#" -ne 0 ]; then
    printf 'usage: %s\n' "$0" >&2
    exit 2
fi

read_pin() {
    value=$(sed -n '1p' "$1")
    case "$value" in
        *[!0-9a-f]*|'') printf 'FAIL: invalid SHA-256 pin in %s\n' "$1" >&2; exit 1 ;;
    esac
    if [ "${#value}" -ne 64 ]; then
        printf 'FAIL: SHA-256 pin in %s must contain 64 hex digits\n' "$1" >&2
        exit 1
    fi
    printf '%s\n' "$value"
}

expected_lifecycle=$(read_pin "$pin_dir/MODEL_SHA256")
expected_identity=$(read_pin "$pin_dir/DEVICE_IDENTITY_MODEL_SHA256")
expected_projection=$(read_pin "$pin_dir/DEVICE_PROJECTION_REFINEMENT_SHA256")
expected_memory=$(read_pin "$pin_dir/MEMORY_LIFECYCLE_SHA256")
expected_queue=$(read_pin "$pin_dir/QUEUE_LIFECYCLE_SHA256")
expected_load_plan=$(read_pin "$pin_dir/LOAD_PLAN_SHA256")
expected_materialization=$(read_pin "$pin_dir/MATERIALIZATION_SHA256")
expected_negative_lifecycle=$(read_pin "$pin_dir/NEGATIVE_SHA256")
expected_aql=$(read_pin "$pin_dir/AQL_PUBLICATION_SHA256")
expected_r7_async_resources=$(read_pin "$pin_dir/R7_ASYNC_RESOURCES_SHA256")
expected_r8_execution_contracts=$(read_pin "$pin_dir/R8_EXECUTION_CONTRACTS_SHA256")
expected_r9_native_evidence=$(read_pin "$pin_dir/R9_NATIVE_EVIDENCE_SHA256")
expected_r10_closed_execution=$(read_pin "$pin_dir/R10_CLOSED_EXECUTION_SHA256")
expected_r11_runtime_semantics=$(read_pin "$pin_dir/R11_RUNTIME_SEMANTICS_SHA256")
expected_r12_native_concurrency=$(read_pin "$pin_dir/R12_NATIVE_CONCURRENCY_SHA256")
expected_r13_logical_scheduler=$(read_pin "$pin_dir/R13_LOGICAL_SCHEDULER_SHA256")
expected_r14_async_observer=$(read_pin "$pin_dir/R14_ASYNC_OBSERVER_SHA256")
expected_r16_worker_semantic_boundary=$(read_pin "$pin_dir/R16_WORKER_SEMANTIC_BOUNDARY_SHA256")
expected_negative_vm=$(read_pin "$pin_dir/NEGATIVE_VM_SUBSTITUTION_SHA256")
expected_negative_stale=$(read_pin "$pin_dir/NEGATIVE_STALE_REUSE_SHA256")
expected_negative_render=$(read_pin "$pin_dir/NEGATIVE_RENDER_SUBSTITUTION_SHA256")
expected_negative_projection_schema=$(read_pin "$pin_dir/NEGATIVE_PROJECTION_SCHEMA_SHA256")
expected_negative_projection_history=$(read_pin "$pin_dir/NEGATIVE_PROJECTION_HISTORY_SHA256")
expected_negative_projection_identity=$(read_pin "$pin_dir/NEGATIVE_PROJECTION_IDENTITY_SHA256")
expected_negative_projection_currentness=$(read_pin "$pin_dir/NEGATIVE_PROJECTION_CURRENTNESS_SHA256")
expected_negative_memory_free=$(read_pin "$pin_dir/NEGATIVE_MEMORY_FREE_SHA256")
expected_negative_memory_unmap=$(read_pin "$pin_dir/NEGATIVE_MEMORY_UNMAP_SHA256")
expected_negative_memory_failed_full=$(read_pin "$pin_dir/NEGATIVE_MEMORY_FAILED_FULL_SHA256")
expected_negative_queue_resource_substitution=$(read_pin "$pin_dir/NEGATIVE_QUEUE_RESOURCE_SUBSTITUTION_SHA256")
expected_negative_queue_destroy_ambiguity=$(read_pin "$pin_dir/NEGATIVE_QUEUE_DESTROY_AMBIGUITY_SHA256")
expected_negative_queue_destroy_source_restore=$(read_pin "$pin_dir/NEGATIVE_QUEUE_DESTROY_SOURCE_RESTORE_SHA256")
expected_negative_queue_history_prefix=$(read_pin "$pin_dir/NEGATIVE_QUEUE_HISTORY_PREFIX_SHA256")
expected_negative_queue_sentinel_returned=$(read_pin "$pin_dir/NEGATIVE_QUEUE_SENTINEL_RETURNED_SHA256")
expected_negative_queue_publication_owner=$(read_pin "$pin_dir/NEGATIVE_QUEUE_PUBLICATION_OWNER_SHA256")
expected_negative_queue_ambiguous_id_reuse=$(read_pin "$pin_dir/NEGATIVE_QUEUE_AMBIGUOUS_ID_REUSE_SHA256")
expected_negative_queue_mapping_generation=$(read_pin "$pin_dir/NEGATIVE_QUEUE_MAPPING_GENERATION_SHA256")
expected_negative_queue_illegal_ambiguity=$(read_pin "$pin_dir/NEGATIVE_QUEUE_ILLEGAL_AMBIGUITY_SHA256")
expected_negative_queue_generic_create_ambiguity=$(read_pin "$pin_dir/NEGATIVE_QUEUE_GENERIC_CREATE_AMBIGUITY_SHA256")
expected_negative_queue_cancel_retention=$(read_pin "$pin_dir/NEGATIVE_QUEUE_CANCEL_RETENTION_SHA256")
expected_negative_queue_pending_create_overlap=$(read_pin "$pin_dir/NEGATIVE_QUEUE_PENDING_CREATE_OVERLAP_SHA256")
expected_negative_load_page_overlap=$(read_pin "$pin_dir/NEGATIVE_LOAD_PAGE_OVERLAP_SHA256")
expected_negative_load_descriptor_delta=$(read_pin "$pin_dir/NEGATIVE_LOAD_DESCRIPTOR_DELTA_SHA256")
expected_negative_materialization_source=$(read_pin "$pin_dir/NEGATIVE_MATERIALIZATION_SOURCE_SHA256")
expected_negative_materialization_zero=$(read_pin "$pin_dir/NEGATIVE_MATERIALIZATION_ZERO_SHA256")
expected_verus=$(read_pin "$pin_dir/VERUS_SHA256")
expected_negative_aql_vendor_body=$(read_pin "$pin_dir/NEGATIVE_AQL_VENDOR_BODY_SHA256")
expected_negative_aql_setup_substitution=$(read_pin "$pin_dir/NEGATIVE_AQL_SETUP_SUBSTITUTION_SHA256")
expected_negative_aql_replay=$(read_pin "$pin_dir/NEGATIVE_AQL_REPLAY_SHA256")
expected_negative_aql_read_regression=$(read_pin "$pin_dir/NEGATIVE_AQL_READ_REGRESSION_SHA256")
expected_negative_aql_full_overwrite=$(read_pin "$pin_dir/NEGATIVE_AQL_FULL_OVERWRITE_SHA256")
expected_negative_r7_generation_reuse=$(read_pin "$pin_dir/NEGATIVE_R7_GENERATION_REUSE_SHA256")
expected_negative_r7_cross_device=$(read_pin "$pin_dir/NEGATIVE_R7_CROSS_DEVICE_SHA256")
expected_negative_r8_eager_publication=$(read_pin "$pin_dir/NEGATIVE_R8_EAGER_PUBLICATION_SHA256")
expected_negative_r8_conflicting_overlap=$(read_pin "$pin_dir/NEGATIVE_R8_CONFLICTING_OVERLAP_SHA256")
expected_negative_r8_dependency_polarity=$(read_pin "$pin_dir/NEGATIVE_R8_DEPENDENCY_POLARITY_SHA256")
expected_negative_r8_binding_substitution=$(read_pin "$pin_dir/NEGATIVE_R8_BINDING_SUBSTITUTION_SHA256")
expected_negative_r8_generation_substitution=$(read_pin "$pin_dir/NEGATIVE_R8_GENERATION_SUBSTITUTION_SHA256")
expected_negative_r8_epoch_substitution=$(read_pin "$pin_dir/NEGATIVE_R8_EPOCH_SUBSTITUTION_SHA256")
expected_negative_r8_atomic_alignment=$(read_pin "$pin_dir/NEGATIVE_R8_ATOMIC_ALIGNMENT_SHA256")
expected_negative_r8_atomic_coherence=$(read_pin "$pin_dir/NEGATIVE_R8_ATOMIC_COHERENCE_SHA256")
expected_negative_r8_atomic_return=$(read_pin "$pin_dir/NEGATIVE_R8_ATOMIC_RETURN_SHA256")
expected_negative_r8_early_collective=$(read_pin "$pin_dir/NEGATIVE_R8_EARLY_COLLECTIVE_SHA256")
expected_negative_r8_duplicate_collective=$(read_pin "$pin_dir/NEGATIVE_R8_DUPLICATE_COLLECTIVE_SHA256")
expected_negative_r9_duplicate_gpu=$(read_pin "$pin_dir/NEGATIVE_R9_DUPLICATE_GPU_SHA256")
expected_negative_r9_nonzero_begin_prefix=$(read_pin "$pin_dir/NEGATIVE_R9_NONZERO_BEGIN_PREFIX_SHA256")
expected_negative_r9_map_prefix_substitution=$(read_pin "$pin_dir/NEGATIVE_R9_MAP_PREFIX_SUBSTITUTION_SHA256")
expected_negative_r9_unmap_prefix_addition=$(read_pin "$pin_dir/NEGATIVE_R9_UNMAP_PREFIX_ADDITION_SHA256")
expected_negative_r9_early_compensation_release=$(read_pin "$pin_dir/NEGATIVE_R9_EARLY_COMPENSATION_RELEASE_SHA256")
expected_negative_r9_incomplete_compensation=$(read_pin "$pin_dir/NEGATIVE_R9_INCOMPLETE_COMPENSATION_SHA256")
expected_negative_r9_reversed_route=$(read_pin "$pin_dir/NEGATIVE_R9_REVERSED_ROUTE_SHA256")
expected_negative_r9_stale_topology=$(read_pin "$pin_dir/NEGATIVE_R9_STALE_TOPOLOGY_SHA256")
expected_negative_r9_reset_fence_drop=$(read_pin "$pin_dir/NEGATIVE_R9_RESET_FENCE_DROP_SHA256")
expected_negative_r9_artifact_substitution=$(read_pin "$pin_dir/NEGATIVE_R9_ARTIFACT_SUBSTITUTION_SHA256")
expected_negative_r9_receipt_substitution=$(read_pin "$pin_dir/NEGATIVE_R9_RECEIPT_SUBSTITUTION_SHA256")
expected_negative_r9_stale_dispatch=$(read_pin "$pin_dir/NEGATIVE_R9_STALE_DISPATCH_SHA256")
expected_negative_r9_incomplete_dependency=$(read_pin "$pin_dir/NEGATIVE_R9_INCOMPLETE_DEPENDENCY_SHA256")
expected_negative_r9_copy_inactive_mapping=$(read_pin "$pin_dir/NEGATIVE_R9_COPY_INACTIVE_MAPPING_SHA256")
expected_negative_r9_uncertain_copy_release=$(read_pin "$pin_dir/NEGATIVE_R9_UNCERTAIN_COPY_RELEASE_SHA256")
expected_negative_r10_dependency_bypass=$(read_pin "$pin_dir/NEGATIVE_R10_DEPENDENCY_BYPASS_SHA256")
expected_negative_r10_partial_batch=$(read_pin "$pin_dir/NEGATIVE_R10_PARTIAL_BATCH_SHA256")
expected_negative_r10_pool_generation=$(read_pin "$pin_dir/NEGATIVE_R10_POOL_GENERATION_SHA256")
expected_negative_r10_peer_owner=$(read_pin "$pin_dir/NEGATIVE_R10_PEER_OWNER_SHA256")
expected_negative_r10_cancel_release=$(read_pin "$pin_dir/NEGATIVE_R10_CANCEL_RELEASE_SHA256")
expected_negative_r10_quarantine_release=$(read_pin "$pin_dir/NEGATIVE_R10_QUARANTINE_RELEASE_SHA256")
expected_negative_r10_atomic_scope=$(read_pin "$pin_dir/NEGATIVE_R10_ATOMIC_SCOPE_SHA256")
expected_negative_r10_atomic_fence=$(read_pin "$pin_dir/NEGATIVE_R10_ATOMIC_FENCE_SHA256")
expected_negative_r10_atomic_return=$(read_pin "$pin_dir/NEGATIVE_R10_ATOMIC_RETURN_SHA256")
expected_negative_r10_wave_early=$(read_pin "$pin_dir/NEGATIVE_R10_WAVE_EARLY_SHA256")
expected_negative_r10_scan_prefix=$(read_pin "$pin_dir/NEGATIVE_R10_SCAN_PREFIX_SHA256")
expected_negative_r11_atomic_capability=$(read_pin "$pin_dir/NEGATIVE_R11_ATOMIC_CAPABILITY_SHA256")
expected_negative_r11_callback_redischarge=$(read_pin "$pin_dir/NEGATIVE_R11_CALLBACK_REDISCHARGE_SHA256")
expected_negative_r11_compare_exchange_failure_order=$(read_pin "$pin_dir/NEGATIVE_R11_COMPARE_EXCHANGE_FAILURE_ORDER_SHA256")
expected_negative_r11_collective_membership=$(read_pin "$pin_dir/NEGATIVE_R11_COLLECTIVE_MEMBERSHIP_SHA256")
expected_negative_r11_collective_partial_tail=$(read_pin "$pin_dir/NEGATIVE_R11_COLLECTIVE_PARTIAL_TAIL_SHA256")
expected_negative_r11_event_substitution=$(read_pin "$pin_dir/NEGATIVE_R11_EVENT_SUBSTITUTION_SHA256")
expected_negative_r11_mapping_early_release=$(read_pin "$pin_dir/NEGATIVE_R11_MAPPING_EARLY_RELEASE_SHA256")
expected_negative_r11_mapping_uncertain=$(read_pin "$pin_dir/NEGATIVE_R11_MAPPING_UNCERTAIN_SHA256")
expected_negative_r12_capability_count=$(read_pin "$pin_dir/NEGATIVE_R12_CAPABILITY_COUNT_SHA256")
expected_negative_r12_cross_queue_terminal=$(read_pin "$pin_dir/NEGATIVE_R12_CROSS_QUEUE_TERMINAL_SHA256")
expected_negative_r12_currentness_quarantine=$(read_pin "$pin_dir/NEGATIVE_R12_CURRENTNESS_QUARANTINE_SHA256")
expected_negative_r12_dependent_release=$(read_pin "$pin_dir/NEGATIVE_R12_DEPENDENT_RELEASE_SHA256")
expected_negative_r12_dependency_bypass=$(read_pin "$pin_dir/NEGATIVE_R12_DEPENDENCY_BYPASS_SHA256")
expected_negative_r12_indeterminate_drain=$(read_pin "$pin_dir/NEGATIVE_R12_INDETERMINATE_DRAIN_SHA256")
expected_negative_r12_published_cancel=$(read_pin "$pin_dir/NEGATIVE_R12_PUBLISHED_CANCEL_SHA256")
expected_negative_r12_published_release=$(read_pin "$pin_dir/NEGATIVE_R12_PUBLISHED_RELEASE_SHA256")
expected_negative_r12_queue_recreation=$(read_pin "$pin_dir/NEGATIVE_R12_QUEUE_RECREATION_SHA256")
expected_negative_r12_queue_occurrence=$(read_pin "$pin_dir/NEGATIVE_R12_QUEUE_OCCURRENCE_SHA256")
expected_negative_r12_slot_recycle=$(read_pin "$pin_dir/NEGATIVE_R12_SLOT_RECYCLE_SHA256")
expected_negative_r12_slot_generation=$(read_pin "$pin_dir/NEGATIVE_R12_SLOT_GENERATION_SHA256")
expected_negative_r12_stale_drain=$(read_pin "$pin_dir/NEGATIVE_R12_STALE_DRAIN_SHA256")
expected_negative_r13_currentness_quarantine=$(read_pin "$pin_dir/NEGATIVE_R13_CURRENTNESS_QUARANTINE_SHA256")
expected_negative_r13_dependency_bound=$(read_pin "$pin_dir/NEGATIVE_R13_DEPENDENCY_BOUND_SHA256")
expected_negative_r13_dependency_bypass=$(read_pin "$pin_dir/NEGATIVE_R13_DEPENDENCY_BYPASS_SHA256")
expected_negative_r13_dependent_release=$(read_pin "$pin_dir/NEGATIVE_R13_DEPENDENT_RELEASE_SHA256")
expected_negative_r13_fifo_bypass=$(read_pin "$pin_dir/NEGATIVE_R13_FIFO_BYPASS_SHA256")
expected_negative_r13_foreign_owner=$(read_pin "$pin_dir/NEGATIVE_R13_FOREIGN_OWNER_SHA256")
expected_negative_r13_foreign_terminal=$(read_pin "$pin_dir/NEGATIVE_R13_FOREIGN_TERMINAL_SHA256")
expected_negative_r13_lane_collision=$(read_pin "$pin_dir/NEGATIVE_R13_LANE_COLLISION_SHA256")
expected_negative_r13_non_tail_cancel=$(read_pin "$pin_dir/NEGATIVE_R13_NON_TAIL_CANCEL_SHA256")
expected_negative_r13_resource_overlap=$(read_pin "$pin_dir/NEGATIVE_R13_RESOURCE_OVERLAP_SHA256")
expected_negative_r13_third_lane=$(read_pin "$pin_dir/NEGATIVE_R13_THIRD_LANE_SHA256")
expected_negative_r14_abandon_release=$(read_pin "$pin_dir/NEGATIVE_R14_ABANDON_RELEASE_SHA256")
expected_negative_r14_capacity_bound=$(read_pin "$pin_dir/NEGATIVE_R14_CAPACITY_BOUND_SHA256")
expected_negative_r14_duplicate_registration=$(read_pin "$pin_dir/NEGATIVE_R14_DUPLICATE_REGISTRATION_SHA256")
expected_negative_r14_error_substitution=$(read_pin "$pin_dir/NEGATIVE_R14_ERROR_SUBSTITUTION_SHA256")
expected_negative_r14_key_order=$(read_pin "$pin_dir/NEGATIVE_R14_KEY_ORDER_SHA256")
expected_negative_r14_pending_removal=$(read_pin "$pin_dir/NEGATIVE_R14_PENDING_REMOVAL_SHA256")
expected_negative_r14_status_substitution=$(read_pin "$pin_dir/NEGATIVE_R14_STATUS_SUBSTITUTION_SHA256")
expected_negative_r14_stop_cancel=$(read_pin "$pin_dir/NEGATIVE_R14_STOP_CANCEL_SHA256")
expected_negative_r16_contract_substitution=$(read_pin "$pin_dir/NEGATIVE_R16_CONTRACT_SUBSTITUTION_SHA256")
expected_negative_r16_dependency_bound=$(read_pin "$pin_dir/NEGATIVE_R16_DEPENDENCY_BOUND_SHA256")
expected_negative_r16_handshake_downgrade=$(read_pin "$pin_dir/NEGATIVE_R16_HANDSHAKE_DOWNGRADE_SHA256")
expected_negative_r16_pre_custody=$(read_pin "$pin_dir/NEGATIVE_R16_PRE_CUSTODY_SHA256")
expected_negative_r16_reachability=$(read_pin "$pin_dir/NEGATIVE_R16_REACHABILITY_SHA256")
expected_negative_r16_response_custody=$(read_pin "$pin_dir/NEGATIVE_R16_RESPONSE_CUSTODY_SHA256")
expected_negative_r16_sidecar_scope=$(read_pin "$pin_dir/NEGATIVE_R16_SIDECAR_SCOPE_SHA256")
expected_negative_r16_sidecar_substitution=$(read_pin "$pin_dir/NEGATIVE_R16_SIDECAR_SUBSTITUTION_SHA256")
expected_negative_r16_terminal_reopen=$(read_pin "$pin_dir/NEGATIVE_R16_TERMINAL_REOPEN_SHA256")
expected_negative_r16_variant_mismatch=$(read_pin "$pin_dir/NEGATIVE_R16_VARIANT_MISMATCH_SHA256")
expected_closure=$(read_pin "$pin_dir/VERUS_CLOSURE_MANIFEST_SHA256")
expected_source_checker=$(read_pin "$pin_dir/PROOF_SOURCE_CHECKER_SHA256")
expected_transcript=$(read_pin "$pin_dir/TRANSCRIPT_SHA256")
expected_version=$(sed -n '1p' "$pin_dir/VERUS_VERSION")
case "$expected_version" in
    ''|*[!0-9A-Za-z.-]*) printf 'FAIL: invalid pinned Verus version\n' >&2; exit 1 ;;
esac

sha256_path=$(command -v sha256sum 2>/dev/null || true)
timeout_path=$(command -v timeout 2>/dev/null || true)
readlink_path=$(command -v readlink 2>/dev/null || true)
if [ -z "$sha256_path" ] || [ -z "$timeout_path" ] || [ -z "$readlink_path" ]; then
    printf 'FAIL: sha256sum, timeout, and readlink are required\n' >&2
    exit 1
fi

check_digest() {
    actual=$("$sha256_path" "$2" | awk '{ print $1 }')
    if [ "$actual" != "$1" ]; then
        printf 'FAIL: SHA-256 substitution for %s\n' "$2" >&2
        exit 1
    fi
}

check_sources() {
    check_digest "$expected_lifecycle" "$lifecycle_proof"
    check_digest "$expected_identity" "$identity_proof"
    check_digest "$expected_projection" "$projection_proof"
    check_digest "$expected_memory" "$memory_proof"
    check_digest "$expected_queue" "$queue_proof"
    check_digest "$expected_load_plan" "$load_plan_proof"
    check_digest "$expected_materialization" "$materialization_proof"
    check_digest "$expected_negative_lifecycle" "$negative_lifecycle"
    check_digest "$expected_negative_vm" "$negative_vm"
    check_digest "$expected_aql" "$aql_proof"
    check_digest "$expected_r7_async_resources" "$r7_async_resources_proof"
    check_digest "$expected_r8_execution_contracts" "$r8_execution_contracts_proof"
    check_digest "$expected_r9_native_evidence" "$r9_native_evidence_proof"
    check_digest "$expected_r10_closed_execution" "$r10_closed_execution_proof"
    check_digest "$expected_r11_runtime_semantics" "$r11_runtime_semantics_proof"
    check_digest "$expected_r12_native_concurrency" "$r12_native_concurrency_proof"
    check_digest "$expected_r13_logical_scheduler" "$r13_logical_scheduler_proof"
    check_digest "$expected_r14_async_observer" "$r14_async_observer_proof"
    check_digest "$expected_r16_worker_semantic_boundary" "$r16_worker_semantic_boundary_proof"
    check_digest "$expected_negative_stale" "$negative_stale"
    check_digest "$expected_negative_render" "$negative_render"
    check_digest "$expected_negative_projection_schema" "$negative_projection_schema"
    check_digest "$expected_negative_projection_history" "$negative_projection_history"
    check_digest "$expected_negative_projection_identity" "$negative_projection_identity"
    check_digest "$expected_negative_projection_currentness" "$negative_projection_currentness"
    check_digest "$expected_negative_memory_free" "$negative_memory_free"
    check_digest "$expected_negative_memory_unmap" "$negative_memory_unmap"
    check_digest "$expected_negative_memory_failed_full" "$negative_memory_failed_full"
    check_digest "$expected_negative_queue_resource_substitution" "$negative_queue_resource_substitution"
    check_digest "$expected_negative_queue_destroy_ambiguity" "$negative_queue_destroy_ambiguity"
    check_digest "$expected_negative_queue_destroy_source_restore" "$negative_queue_destroy_source_restore"
    check_digest "$expected_negative_queue_history_prefix" "$negative_queue_history_prefix"
    check_digest "$expected_negative_queue_sentinel_returned" "$negative_queue_sentinel_returned"
    check_digest "$expected_negative_queue_publication_owner" "$negative_queue_publication_owner"
    check_digest "$expected_negative_queue_ambiguous_id_reuse" "$negative_queue_ambiguous_id_reuse"
    check_digest "$expected_negative_queue_mapping_generation" "$negative_queue_mapping_generation"
    check_digest "$expected_negative_queue_illegal_ambiguity" "$negative_queue_illegal_ambiguity"
    check_digest "$expected_negative_queue_generic_create_ambiguity" "$negative_queue_generic_create_ambiguity"
    check_digest "$expected_negative_queue_cancel_retention" "$negative_queue_cancel_retention"
    check_digest "$expected_negative_queue_pending_create_overlap" "$negative_queue_pending_create_overlap"
    check_digest "$expected_negative_load_page_overlap" "$negative_load_page_overlap"
    check_digest "$expected_negative_load_descriptor_delta" "$negative_load_descriptor_delta"
    check_digest "$expected_negative_materialization_source" "$negative_materialization_source"
    check_digest "$expected_negative_materialization_zero" "$negative_materialization_zero"
    check_digest "$expected_closure" "$closure_manifest"
    check_digest 'c0f5f201dca9ea6b3fa953884cdfaca8ca38413ad2a9de7700b3aaeb3a610d0c' "$closure_checker"
    check_digest "$expected_negative_aql_vendor_body" "$negative_aql_vendor_body"
    check_digest "$expected_negative_aql_setup_substitution" "$negative_aql_setup_substitution"
    check_digest "$expected_negative_aql_replay" "$negative_aql_replay"
    check_digest "$expected_negative_aql_read_regression" "$negative_aql_read_regression"
    check_digest "$expected_negative_aql_full_overwrite" "$negative_aql_full_overwrite"
    check_digest "$expected_negative_r7_generation_reuse" "$negative_r7_generation_reuse"
    check_digest "$expected_negative_r7_cross_device" "$negative_r7_cross_device"
    check_digest "$expected_negative_r8_eager_publication" "$negative_r8_eager_publication"
    check_digest "$expected_negative_r8_conflicting_overlap" "$negative_r8_conflicting_overlap"
    check_digest "$expected_negative_r8_dependency_polarity" "$negative_r8_dependency_polarity"
    check_digest "$expected_negative_r8_binding_substitution" "$negative_r8_binding_substitution"
    check_digest "$expected_negative_r8_generation_substitution" "$negative_r8_generation_substitution"
    check_digest "$expected_negative_r8_epoch_substitution" "$negative_r8_epoch_substitution"
    check_digest "$expected_negative_r8_atomic_alignment" "$negative_r8_atomic_alignment"
    check_digest "$expected_negative_r8_atomic_coherence" "$negative_r8_atomic_coherence"
    check_digest "$expected_negative_r8_atomic_return" "$negative_r8_atomic_return"
    check_digest "$expected_negative_r8_early_collective" "$negative_r8_early_collective"
    check_digest "$expected_negative_r8_duplicate_collective" "$negative_r8_duplicate_collective"
    check_digest "$expected_negative_r9_duplicate_gpu" "$negative_r9_duplicate_gpu"
    check_digest "$expected_negative_r9_nonzero_begin_prefix" "$negative_r9_nonzero_begin_prefix"
    check_digest "$expected_negative_r9_map_prefix_substitution" "$negative_r9_map_prefix_substitution"
    check_digest "$expected_negative_r9_unmap_prefix_addition" "$negative_r9_unmap_prefix_addition"
    check_digest "$expected_negative_r9_early_compensation_release" "$negative_r9_early_compensation_release"
    check_digest "$expected_negative_r9_incomplete_compensation" "$negative_r9_incomplete_compensation"
    check_digest "$expected_negative_r9_reversed_route" "$negative_r9_reversed_route"
    check_digest "$expected_negative_r9_stale_topology" "$negative_r9_stale_topology"
    check_digest "$expected_negative_r9_reset_fence_drop" "$negative_r9_reset_fence_drop"
    check_digest "$expected_negative_r9_artifact_substitution" "$negative_r9_artifact_substitution"
    check_digest "$expected_negative_r9_receipt_substitution" "$negative_r9_receipt_substitution"
    check_digest "$expected_negative_r9_stale_dispatch" "$negative_r9_stale_dispatch"
    check_digest "$expected_negative_r9_incomplete_dependency" "$negative_r9_incomplete_dependency"
    check_digest "$expected_negative_r9_copy_inactive_mapping" "$negative_r9_copy_inactive_mapping"
    check_digest "$expected_negative_r9_uncertain_copy_release" "$negative_r9_uncertain_copy_release"
    check_digest "$expected_negative_r10_dependency_bypass" "$negative_r10_dependency_bypass"
    check_digest "$expected_negative_r10_partial_batch" "$negative_r10_partial_batch"
    check_digest "$expected_negative_r10_pool_generation" "$negative_r10_pool_generation"
    check_digest "$expected_negative_r10_peer_owner" "$negative_r10_peer_owner"
    check_digest "$expected_negative_r10_cancel_release" "$negative_r10_cancel_release"
    check_digest "$expected_negative_r10_quarantine_release" "$negative_r10_quarantine_release"
    check_digest "$expected_negative_r10_atomic_scope" "$negative_r10_atomic_scope"
    check_digest "$expected_negative_r10_atomic_fence" "$negative_r10_atomic_fence"
    check_digest "$expected_negative_r10_atomic_return" "$negative_r10_atomic_return"
    check_digest "$expected_negative_r10_wave_early" "$negative_r10_wave_early"
    check_digest "$expected_negative_r10_scan_prefix" "$negative_r10_scan_prefix"
    check_digest "$expected_negative_r11_atomic_capability" "$negative_r11_atomic_capability"
    check_digest "$expected_negative_r11_callback_redischarge" "$negative_r11_callback_redischarge"
    check_digest "$expected_negative_r11_compare_exchange_failure_order" "$negative_r11_compare_exchange_failure_order"
    check_digest "$expected_negative_r11_collective_membership" "$negative_r11_collective_membership"
    check_digest "$expected_negative_r11_collective_partial_tail" "$negative_r11_collective_partial_tail"
    check_digest "$expected_negative_r11_event_substitution" "$negative_r11_event_substitution"
    check_digest "$expected_negative_r11_mapping_early_release" "$negative_r11_mapping_early_release"
    check_digest "$expected_negative_r11_mapping_uncertain" "$negative_r11_mapping_uncertain"
    check_digest "$expected_negative_r12_capability_count" "$negative_r12_capability_count"
    check_digest "$expected_negative_r12_cross_queue_terminal" "$negative_r12_cross_queue_terminal"
    check_digest "$expected_negative_r12_currentness_quarantine" "$negative_r12_currentness_quarantine"
    check_digest "$expected_negative_r12_dependent_release" "$negative_r12_dependent_release"
    check_digest "$expected_negative_r12_dependency_bypass" "$negative_r12_dependency_bypass"
    check_digest "$expected_negative_r12_indeterminate_drain" "$negative_r12_indeterminate_drain"
    check_digest "$expected_negative_r12_published_cancel" "$negative_r12_published_cancel"
    check_digest "$expected_negative_r12_published_release" "$negative_r12_published_release"
    check_digest "$expected_negative_r12_queue_recreation" "$negative_r12_queue_recreation"
    check_digest "$expected_negative_r12_queue_occurrence" "$negative_r12_queue_occurrence"
    check_digest "$expected_negative_r12_slot_recycle" "$negative_r12_slot_recycle"
    check_digest "$expected_negative_r12_slot_generation" "$negative_r12_slot_generation"
    check_digest "$expected_negative_r12_stale_drain" "$negative_r12_stale_drain"
    check_digest "$expected_negative_r13_currentness_quarantine" "$negative_r13_currentness_quarantine"
    check_digest "$expected_negative_r13_dependency_bound" "$negative_r13_dependency_bound"
    check_digest "$expected_negative_r13_dependency_bypass" "$negative_r13_dependency_bypass"
    check_digest "$expected_negative_r13_dependent_release" "$negative_r13_dependent_release"
    check_digest "$expected_negative_r13_fifo_bypass" "$negative_r13_fifo_bypass"
    check_digest "$expected_negative_r13_foreign_owner" "$negative_r13_foreign_owner"
    check_digest "$expected_negative_r13_foreign_terminal" "$negative_r13_foreign_terminal"
    check_digest "$expected_negative_r13_lane_collision" "$negative_r13_lane_collision"
    check_digest "$expected_negative_r13_non_tail_cancel" "$negative_r13_non_tail_cancel"
    check_digest "$expected_negative_r13_resource_overlap" "$negative_r13_resource_overlap"
    check_digest "$expected_negative_r13_third_lane" "$negative_r13_third_lane"
    check_digest "$expected_negative_r14_abandon_release" "$negative_r14_abandon_release"
    check_digest "$expected_negative_r14_capacity_bound" "$negative_r14_capacity_bound"
    check_digest "$expected_negative_r14_duplicate_registration" "$negative_r14_duplicate_registration"
    check_digest "$expected_negative_r14_error_substitution" "$negative_r14_error_substitution"
    check_digest "$expected_negative_r14_key_order" "$negative_r14_key_order"
    check_digest "$expected_negative_r14_pending_removal" "$negative_r14_pending_removal"
    check_digest "$expected_negative_r14_status_substitution" "$negative_r14_status_substitution"
    check_digest "$expected_negative_r14_stop_cancel" "$negative_r14_stop_cancel"
    check_digest "$expected_negative_r16_contract_substitution" "$negative_r16_contract_substitution"
    check_digest "$expected_negative_r16_dependency_bound" "$negative_r16_dependency_bound"
    check_digest "$expected_negative_r16_handshake_downgrade" "$negative_r16_handshake_downgrade"
    check_digest "$expected_negative_r16_pre_custody" "$negative_r16_pre_custody"
    check_digest "$expected_negative_r16_reachability" "$negative_r16_reachability"
    check_digest "$expected_negative_r16_response_custody" "$negative_r16_response_custody"
    check_digest "$expected_negative_r16_sidecar_scope" "$negative_r16_sidecar_scope"
    check_digest "$expected_negative_r16_sidecar_substitution" "$negative_r16_sidecar_substitution"
    check_digest "$expected_negative_r16_terminal_reopen" "$negative_r16_terminal_reopen"
    check_digest "$expected_negative_r16_variant_mismatch" "$negative_r16_variant_mismatch"
    check_digest "$expected_source_checker" "$source_checker"
}

check_sources
"$source_checker" \
    "$lifecycle_proof" \
    "$identity_proof" \
    "$projection_proof" \
    "$memory_proof" \
    "$queue_proof" \
    "$load_plan_proof" \
    "$materialization_proof" \
    "$negative_lifecycle" \
    "$negative_vm" \
    "$negative_stale" \
    "$aql_proof" \
    "$r7_async_resources_proof" \
    "$r8_execution_contracts_proof" \
    "$r9_native_evidence_proof" \
    "$r10_closed_execution_proof" \
    "$r11_runtime_semantics_proof" \
    "$r12_native_concurrency_proof" \
    "$r13_logical_scheduler_proof" \
    "$r14_async_observer_proof" \
    "$r16_worker_semantic_boundary_proof" \
    "$negative_render" \
    "$negative_projection_schema" \
    "$negative_projection_history" \
    "$negative_projection_identity" \
    "$negative_projection_currentness" \
    "$negative_memory_free" \
    "$negative_memory_unmap" \
    "$negative_memory_failed_full" \
    "$negative_queue_resource_substitution" \
    "$negative_queue_destroy_ambiguity" \
    "$negative_queue_destroy_source_restore" \
    "$negative_queue_history_prefix" \
    "$negative_queue_sentinel_returned" \
    "$negative_queue_publication_owner" \
    "$negative_queue_ambiguous_id_reuse" \
    "$negative_queue_mapping_generation" \
    "$negative_queue_illegal_ambiguity" \
    "$negative_queue_generic_create_ambiguity" \
    "$negative_queue_cancel_retention" \
    "$negative_queue_pending_create_overlap" \
    "$negative_load_page_overlap" \
    "$negative_load_descriptor_delta" \
    "$negative_materialization_source" \
    "$negative_materialization_zero" \
    "$negative_aql_vendor_body" \
    "$negative_aql_setup_substitution" \
    "$negative_aql_replay" \
    "$negative_aql_read_regression" \
    "$negative_aql_full_overwrite" \
    "$negative_r7_generation_reuse" \
    "$negative_r7_cross_device" \
    "$negative_r8_eager_publication" \
    "$negative_r8_conflicting_overlap" \
    "$negative_r8_dependency_polarity" \
    "$negative_r8_binding_substitution" \
    "$negative_r8_generation_substitution" \
    "$negative_r8_epoch_substitution" \
    "$negative_r8_atomic_alignment" \
    "$negative_r8_atomic_coherence" \
    "$negative_r8_atomic_return" \
    "$negative_r8_early_collective" \
    "$negative_r8_duplicate_collective" \
    "$negative_r9_duplicate_gpu" \
    "$negative_r9_nonzero_begin_prefix" \
    "$negative_r9_map_prefix_substitution" \
    "$negative_r9_unmap_prefix_addition" \
    "$negative_r9_early_compensation_release" \
    "$negative_r9_incomplete_compensation" \
    "$negative_r9_reversed_route" \
    "$negative_r9_stale_topology" \
    "$negative_r9_reset_fence_drop" \
    "$negative_r9_artifact_substitution" \
    "$negative_r9_receipt_substitution" \
    "$negative_r9_stale_dispatch" \
    "$negative_r9_incomplete_dependency" \
    "$negative_r9_copy_inactive_mapping" \
    "$negative_r9_uncertain_copy_release" \
    "$negative_r10_dependency_bypass" \
    "$negative_r10_partial_batch" \
    "$negative_r10_pool_generation" \
    "$negative_r10_peer_owner" \
    "$negative_r10_cancel_release" \
    "$negative_r10_quarantine_release" \
    "$negative_r10_atomic_scope" \
    "$negative_r10_atomic_fence" \
    "$negative_r10_atomic_return" \
    "$negative_r10_wave_early" \
    "$negative_r10_scan_prefix" \
    "$negative_r11_atomic_capability" \
    "$negative_r11_callback_redischarge" \
    "$negative_r11_compare_exchange_failure_order" \
    "$negative_r11_collective_membership" \
    "$negative_r11_collective_partial_tail" \
    "$negative_r11_event_substitution" \
    "$negative_r11_mapping_early_release" \
    "$negative_r11_mapping_uncertain" \
    "$negative_r12_capability_count" \
    "$negative_r12_cross_queue_terminal" \
    "$negative_r12_currentness_quarantine" \
    "$negative_r12_dependent_release" \
    "$negative_r12_dependency_bypass" \
    "$negative_r12_indeterminate_drain" \
    "$negative_r12_published_cancel" \
    "$negative_r12_published_release" \
    "$negative_r12_queue_recreation" \
    "$negative_r12_queue_occurrence" \
    "$negative_r12_slot_recycle" \
    "$negative_r12_slot_generation" \
    "$negative_r12_stale_drain" \
    "$negative_r13_currentness_quarantine" \
    "$negative_r13_dependency_bound" \
    "$negative_r13_dependency_bypass" \
    "$negative_r13_dependent_release" \
    "$negative_r13_fifo_bypass" \
    "$negative_r13_foreign_owner" \
    "$negative_r13_foreign_terminal" \
    "$negative_r13_lane_collision" \
    "$negative_r13_non_tail_cancel" \
    "$negative_r13_resource_overlap" \
    "$negative_r13_third_lane" \
    "$negative_r14_abandon_release" \
    "$negative_r14_capacity_bound" \
    "$negative_r14_duplicate_registration" \
    "$negative_r14_error_substitution" \
    "$negative_r14_key_order" \
    "$negative_r14_pending_removal" \
    "$negative_r14_status_substitution" \
    "$negative_r14_stop_cancel" \
    "$negative_r16_contract_substitution" \
    "$negative_r16_dependency_bound" \
    "$negative_r16_handshake_downgrade" \
    "$negative_r16_pre_custody" \
    "$negative_r16_reachability" \
    "$negative_r16_response_custody" \
    "$negative_r16_sidecar_scope" \
    "$negative_r16_sidecar_substitution" \
    "$negative_r16_terminal_reopen" \
    "$negative_r16_variant_mismatch"

case "$verus_bin" in
    */*) [ -x "$verus_bin" ] && verus_path=$verus_bin || verus_path= ;;
    *) verus_path=$(command -v "$verus_bin" 2>/dev/null || true) ;;
esac
if [ -z "$verus_path" ]; then
    printf 'FAIL: Verus is unavailable; set VERUS=/absolute/path/to/verus\n' >&2
    exit 1
fi
verus_path=$("$readlink_path" -f "$verus_path")
if [ "$(basename "$verus_path")" != verus ]; then
    printf 'FAIL: pinned Verus executable must be named verus\n' >&2
    exit 1
fi
check_digest "$expected_verus" "$verus_path"
verus_root=$(CDPATH='' cd -- "$(dirname -- "$verus_path")" && pwd)
"$closure_checker" "$verus_root" "$closure_manifest"

runner_home=${HOME:-/nonexistent}
runner_path=${PATH:-/usr/local/bin:/usr/bin:/bin}
runner_rustup_home=${RUSTUP_HOME:-"$runner_home/.rustup"}
runner_cargo_home=${CARGO_HOME:-"$runner_home/.cargo"}
actual_version=$(
    env -i \
        "HOME=$runner_home" \
        "PATH=$runner_path" \
        "RUSTUP_HOME=$runner_rustup_home" \
        "CARGO_HOME=$runner_cargo_home" \
        "VERUS_Z3_PATH=$verus_root/z3" \
        "$verus_path" --version \
        | awk '/^[[:space:]]*Version:/ { print $2; exit }'
)
if [ "$actual_version" != "$expected_version" ]; then
    printf 'FAIL: Verus version does not match the pin\n' >&2
    exit 1
fi

timeout_seconds=${VERUS_TIMEOUT_SECONDS:-120}
case "$timeout_seconds" in
    ''|*[!0-9]*) printf 'FAIL: VERUS_TIMEOUT_SECONDS must be 1 through 300\n' >&2; exit 2 ;;
esac
if [ "$timeout_seconds" -lt 1 ] || [ "$timeout_seconds" -gt 300 ]; then
    printf 'FAIL: VERUS_TIMEOUT_SECONDS must be 1 through 300\n' >&2
    exit 2
fi

tmp_dir=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-runtime-model-verus.XXXXXX")
trap 'rm -rf "$tmp_dir"' EXIT HUP INT TERM

run_verus() {
    "$timeout_path" --foreground --signal=TERM --kill-after=5 "$timeout_seconds" \
        env -i \
        "HOME=$runner_home" \
        "PATH=$runner_path" \
        "RUSTUP_HOME=$runner_rustup_home" \
        "CARGO_HOME=$runner_cargo_home" \
        "VERUS_Z3_PATH=$verus_root/z3" \
        "$verus_path" --crate-type lib --triggers-mode silent "$1"
}

check_positive() {
    source=$1
    expected_summary=$2
    label=$3
    log="$tmp_dir/$label-positive.log"
    if ! run_verus "$source" >"$log" 2>&1; then
        printf 'FAIL: positive proof did not verify: %s\n' "$label" >&2
        cat "$log" >&2
        exit 1
    fi
    if ! grep -Fq "$expected_summary" "$log"; then
        printf 'FAIL: unexpected positive verification summary: %s\n' "$label" >&2
        cat "$log" >&2
        exit 1
    fi
    cat "$log"
}

check_negative() {
    source=$1
    marker=$2
    label=$3
    log="$tmp_dir/$label-negative.log"
    if run_verus "$source" >"$log" 2>&1; then
        printf 'FAIL: expected-negative proof unexpectedly verified: %s\n' "$label" >&2
        cat "$log" >&2
        exit 1
    fi
    if ! grep -Fq "$marker" "$log" \
        || ! grep -Fq 'error: postcondition not satisfied' "$log" \
        || ! grep -Fq 'verification results:: 0 verified, 1 errors' "$log"; then
        printf 'FAIL: mutation failed at an unexpected verification surface: %s\n' "$label" >&2
        cat "$log" >&2
        exit 1
    fi
    printf 'expected-negative rejected: %s\n' "$label"
}

check_positive "$lifecycle_proof" 'verification results:: 2 verified, 0 errors' lifecycle
check_positive "$identity_proof" 'verification results:: 4 verified, 0 errors' identity-generation
check_positive "$projection_proof" 'verification results:: 4 verified, 0 errors' device-projection-refinement
check_positive "$memory_proof" 'verification results:: 6 verified, 0 errors' memory-lifecycle
check_positive "$queue_proof" 'verification results:: 11 verified, 0 errors' queue-lifecycle
check_positive "$load_plan_proof" 'verification results:: 3 verified, 0 errors' load-plan
check_positive "$materialization_proof" 'verification results:: 8 verified, 0 errors' materialization
check_positive "$aql_proof" 'verification results:: 11 verified, 0 errors' aql-publication
check_positive "$r7_async_resources_proof" 'verification results:: 8 verified, 0 errors' r7-async-resources
check_positive "$r8_execution_contracts_proof" 'verification results:: 10 verified, 0 errors' r8-execution-contracts
check_positive "$r9_native_evidence_proof" 'verification results:: 14 verified, 0 errors' r9-native-evidence
check_positive "$r10_closed_execution_proof" 'verification results:: 20 verified, 0 errors' r10-closed-execution
check_positive "$r11_runtime_semantics_proof" 'verification results:: 18 verified, 0 errors' r11-runtime-semantics
check_positive "$r12_native_concurrency_proof" 'verification results:: 23 verified, 0 errors' r12-native-concurrency
check_positive "$r13_logical_scheduler_proof" 'verification results:: 20 verified, 0 errors' r13-logical-scheduler
check_positive "$r14_async_observer_proof" 'verification results:: 10 verified, 0 errors' r14-async-observer
check_positive "$r16_worker_semantic_boundary_proof" 'verification results:: 21 verified, 0 errors' r16-worker-semantic-boundary
check_negative "$negative_lifecycle" mutated_release_while_published_is_safe_v1 release-while-published
check_negative "$negative_vm" mutated_vm_generation_substitution_is_exact_v1 vm-generation-substitution
check_negative "$negative_stale" mutated_stale_generation_reuse_advances_v1 stale-generation-reuse
check_negative "$negative_render" mutated_render_substitution_correlates_v1 render-substitution
check_negative "$negative_projection_schema" mutated_projection_drops_drm_schema_v1 projection-schema-drop
check_negative "$negative_projection_history" mutated_history_forgets_predecessor_v1 projection-history-link
check_negative "$negative_projection_identity" mutated_cross_source_identity_mix_is_equal_v1 projection-identity-mix
check_negative "$negative_projection_currentness" mutated_projection_drops_reset_fence_v1 projection-currentness-drop
check_negative "$negative_memory_free" mutated_free_while_partial_is_safe_v1 memory-free-while-partial
check_negative "$negative_memory_unmap" mutated_unmap_uses_absolute_cumulative_progress_v1 memory-unmap-cumulative
check_negative "$negative_memory_failed_full" mutated_failed_full_unmap_is_unreleasable_v1 memory-unmap-failed-full
check_negative "$negative_queue_resource_substitution" mutated_queue_resource_substitution_preserves_roles_v1 queue-resource-substitution
check_negative "$negative_queue_destroy_ambiguity" mutated_indeterminate_destroy_remains_retaining_v1 queue-destroy-ambiguity
check_negative "$negative_queue_destroy_source_restore" mutated_active_destroy_failure_restores_exact_source_v1 queue-destroy-source-restore
check_negative "$negative_queue_history_prefix" mutated_queue_history_overwrite_preserves_prefix_v1 queue-history-prefix
check_negative "$negative_queue_sentinel_returned" mutated_returned_sentinel_is_rejected_v1 queue-sentinel-returned
check_negative "$negative_queue_publication_owner" mutated_generic_release_rejects_queue_owner_v1 queue-publication-owner
check_negative "$negative_queue_ambiguous_id_reuse" mutated_ambiguous_known_id_blocks_reuse_v1 queue-ambiguous-id-reuse
check_negative "$negative_queue_mapping_generation" mutated_mapping_generation_substitution_is_exact_v1 queue-mapping-generation
check_negative "$negative_queue_illegal_ambiguity" mutated_illegal_indeterminate_update_preserves_active_v1 queue-illegal-ambiguity
check_negative "$negative_queue_generic_create_ambiguity" mutated_generic_create_ambiguity_is_excluded_v1 queue-generic-create-ambiguity
check_negative "$negative_queue_cancel_retention" mutated_cancelled_plan_is_nonretaining_v1 queue-cancel-retention
check_negative "$negative_queue_pending_create_overlap" mutated_pending_create_blocks_second_begin_v1 queue-pending-create-overlap
check_negative "$negative_load_page_overlap" mutated_memory_only_check_rejects_page_overlap_v1 load-page-overlap
check_negative "$negative_load_descriptor_delta" mutated_descriptor_delta_substitution_is_bound_v1 load-descriptor-delta
check_negative "$negative_materialization_source" mutated_source_substitution_preserves_exact_byte_v1 materialization-source-substitution
check_negative "$negative_materialization_zero" mutated_zero_first_initializes_every_byte_v1 materialization-zero-omission
check_negative "$negative_aql_vendor_body" mutated_vendor_body_is_invalid_v1 aql-vendor-body
check_negative "$negative_aql_setup_substitution" mutated_setup_substitution_preserves_copied_setup_v1 aql-setup-substitution
check_negative "$negative_aql_replay" mutated_replay_advances_write_once_v1 aql-reservation-replay
check_negative "$negative_aql_read_regression" mutated_read_regression_is_nondecreasing_v1 aql-read-regression
check_negative "$negative_aql_full_overwrite" mutated_full_overwrite_is_rejected_v1 aql-full-overwrite
check_negative "$negative_r7_generation_reuse" mutated_released_generation_is_reusable_v1 r7-generation-reuse
check_negative "$negative_r7_cross_device" mutated_peer_copy_executes_on_source_v1 r7-cross-device
check_negative "$negative_r8_eager_publication" mutated_reservation_is_deferred_v1 r8-eager-publication
check_negative "$negative_r8_conflicting_overlap" mutated_conflicting_overlap_is_safe_v1 r8-conflicting-overlap
check_negative "$negative_r8_dependency_polarity" mutated_incomplete_dependency_blocks_publication_v1 r8-dependency-polarity
check_negative "$negative_r8_binding_substitution" mutated_ready_publication_retains_destination_v1 r8-binding-substitution
check_negative "$negative_r8_generation_substitution" mutated_ready_publication_retains_generation_v1 r8-generation-substitution
check_negative "$negative_r8_epoch_substitution" mutated_ready_publication_retains_epoch_v1 r8-epoch-substitution
check_negative "$negative_r8_atomic_alignment" mutated_valid_atomic_location_is_aligned_v1 r8-atomic-alignment
check_negative "$negative_r8_atomic_coherence" mutated_fetch_add_retains_coherence_v1 r8-atomic-coherence
check_negative "$negative_r8_atomic_return" mutated_fetch_add_returns_old_v1 r8-atomic-return
check_negative "$negative_r8_early_collective" mutated_partial_collective_cannot_publish_v1 r8-early-collective
check_negative "$negative_r8_duplicate_collective" mutated_duplicate_collective_arrival_does_not_advance_v1 r8-duplicate-collective
check_negative "$negative_r9_duplicate_gpu" mutated_canonical_gpu_ids_are_unique_v1 r9-duplicate-gpu
check_negative "$negative_r9_nonzero_begin_prefix" mutated_mapping_begins_with_zero_prefix_v1 r9-nonzero-begin-prefix
check_negative "$negative_r9_map_prefix_substitution" mutated_failed_map_retains_exact_prefix_v1 r9-map-prefix-substitution
check_negative "$negative_r9_unmap_prefix_addition" mutated_compensation_retains_absolute_cumulative_prefix_v1 r9-unmap-prefix-addition
check_negative "$negative_r9_early_compensation_release" mutated_partial_compensation_blocks_release_v1 r9-early-compensation-release
check_negative "$negative_r9_incomplete_compensation" mutated_complete_compensation_releases_exact_prefix_v1 r9-incomplete-compensation
check_negative "$negative_r9_reversed_route" mutated_reversed_xgmi_direction_is_rejected_v1 r9-reversed-route
check_negative "$negative_r9_stale_topology" mutated_stale_topology_generation_blocks_route_v1 r9-stale-topology
check_negative "$negative_r9_reset_fence_drop" mutated_reset_fence_is_required_for_route_v1 r9-reset-fence-drop
check_negative "$negative_r9_artifact_substitution" mutated_machine_evidence_retains_artifact_v1 r9-artifact-substitution
check_negative "$negative_r9_receipt_substitution" mutated_instruction_class_receipt_is_exact_v1 r9-receipt-substitution
check_negative "$negative_r9_stale_dispatch" mutated_any_stale_surface_blocks_dispatch_v1 r9-stale-dispatch
check_negative "$negative_r9_incomplete_dependency" mutated_incomplete_dependency_blocks_evidence_dispatch_v1 r9-incomplete-dependency
check_negative "$negative_r9_copy_inactive_mapping" mutated_xgmi_copy_requires_both_active_mappings_v1 r9-copy-inactive-mapping
check_negative "$negative_r9_uncertain_copy_release" mutated_uncertain_xgmi_completion_retains_owners_v1 r9-uncertain-copy-release
check_negative "$negative_r10_dependency_bypass" mutated_incomplete_dependency_blocks_closed_publication_v1 r10-dependency-bypass
check_negative "$negative_r10_partial_batch" mutated_unready_batch_has_no_partial_publication_v1 r10-partial-batch
check_negative "$negative_r10_pool_generation" mutated_completed_pool_release_advances_generation_v1 r10-pool-generation
check_negative "$negative_r10_peer_owner" mutated_peer_copy_executes_on_destination_v1 r10-peer-owner
check_negative "$negative_r10_cancel_release" mutated_published_cancellation_retains_leases_v1 r10-cancel-release
check_negative "$negative_r10_quarantine_release" mutated_indeterminate_failure_blocks_release_v1 r10-quarantine-release
check_negative "$negative_r10_atomic_scope" mutated_substituted_atomic_scope_never_corresponds_v1 r10-atomic-scope
check_negative "$negative_r10_atomic_fence" mutated_release_atomic_requires_pre_fence_v1 r10-atomic-fence
check_negative "$negative_r10_atomic_return" mutated_atomic_rmw_returns_old_value_v1 r10-atomic-return
check_negative "$negative_r10_wave_early" mutated_incomplete_wave64_cannot_publish_v1 r10-wave-early
check_negative "$negative_r10_scan_prefix" mutated_inclusive_scan_includes_current_lane_v1 r10-scan-prefix
check_negative "$negative_r11_atomic_capability" mutated_atomic_execution_capability_fails_closed_v1 r11-atomic-capability
check_negative "$negative_r11_callback_redischarge" mutated_repeated_completion_preserves_callback_count_v1 r11-callback-redischarge
check_negative "$negative_r11_compare_exchange_failure_order" mutated_release_failure_order_is_rejected_v1 r11-compare-exchange-failure-order
check_negative "$negative_r11_collective_membership" mutated_collective_membership_mismatch_is_rejected_v1 r11-collective-membership
check_negative "$negative_r11_collective_partial_tail" mutated_partial_tail_collective_geometry_is_rejected_v1 r11-collective-partial-tail
check_negative "$negative_r11_event_substitution" mutated_event_query_retains_source_status_v1 r11-event-substitution
check_negative "$negative_r11_mapping_early_release" mutated_batch_retention_blocks_mapping_release_v1 r11-mapping-early-release
check_negative "$negative_r11_mapping_uncertain" mutated_indeterminate_batch_blocks_mapping_release_v1 r11-mapping-uncertain
check_negative "$negative_r12_capability_count" mutated_single_queue_capability_is_rejected_v1 r12-capability-count
check_negative "$negative_r12_cross_queue_terminal" mutated_cross_queue_terminal_is_rejected_v1 r12-cross-queue-terminal
check_negative "$negative_r12_currentness_quarantine" mutated_currentness_loss_quarantines_published_v1 r12-currentness-quarantine
check_negative "$negative_r12_dependent_release" mutated_reserved_dependent_blocks_terminal_release_v1 r12-dependent-release
check_negative "$negative_r12_dependency_bypass" mutated_unready_dependency_blocks_publication_v1 r12-dependency-bypass
check_negative "$negative_r12_indeterminate_drain" mutated_indeterminate_state_blocks_drain_v1 r12-indeterminate-drain
check_negative "$negative_r12_published_cancel" mutated_published_cancellation_retains_custody_v1 r12-published-cancel
check_negative "$negative_r12_published_release" mutated_published_release_retains_custody_v1 r12-published-release
check_negative "$negative_r12_queue_recreation" mutated_drained_queue_recreation_advances_occurrence_v1 r12-queue-recreation
check_negative "$negative_r12_queue_occurrence" mutated_queue_occurrence_substitution_is_rejected_v1 r12-queue-occurrence
check_negative "$negative_r12_slot_recycle" mutated_cancel_advances_live_slot_generation_v1 r12-slot-recycle
check_negative "$negative_r12_slot_generation" mutated_slot_generation_substitution_is_rejected_v1 r12-slot-generation
check_negative "$negative_r12_stale_drain" mutated_stale_queue_occurrence_cannot_be_drained_v1 r12-stale-drain
check_negative "$negative_r13_currentness_quarantine" mutated_currentness_loss_quarantines_published_v1 r13-currentness-quarantine
check_negative "$negative_r13_dependency_bound" mutated_dependency_count_above_bound_is_admitted_v1 r13-dependency-bound
check_negative "$negative_r13_dependency_bypass" mutated_unready_dependency_blocks_publication_v1 r13-dependency-bypass
check_negative "$negative_r13_dependent_release" mutated_queued_dependent_retains_terminal_resources_v1 r13-dependent-release
check_negative "$negative_r13_fifo_bypass" mutated_non_head_cannot_publish_v1 r13-fifo-bypass
check_negative "$negative_r13_foreign_owner" mutated_foreign_lane_owner_cannot_complete_v1 r13-foreign-owner
check_negative "$negative_r13_foreign_terminal" mutated_foreign_lane_cannot_complete_v1 r13-foreign-terminal
check_negative "$negative_r13_lane_collision" mutated_publication_preserves_unique_lane_owners_v1 r13-lane-collision
check_negative "$negative_r13_non_tail_cancel" mutated_non_tail_cancel_is_rejected_v1 r13-non-tail-cancel
check_negative "$negative_r13_resource_overlap" mutated_resource_overlap_blocks_publication_v1 r13-resource-overlap
check_negative "$negative_r13_third_lane" mutated_third_physical_lane_is_supported_v1 r13-third-lane
check_negative "$negative_r14_abandon_release" mutated_abandon_preserves_runtime_custody_v1 r14-abandon-release
check_negative "$negative_r14_capacity_bound" mutated_capacity_above_bound_is_admitted_v1 r14-capacity-bound
check_negative "$negative_r14_duplicate_registration" mutated_duplicate_registration_is_atomic_v1 r14-duplicate-registration
check_negative "$negative_r14_error_substitution" mutated_runtime_error_observation_is_exact_v1 r14-error-substitution
check_negative "$negative_r14_key_order" mutated_event_key_order_is_lexicographic_v1 r14-key-order
check_negative "$negative_r14_pending_removal" mutated_pending_observation_preserves_waiter_v1 r14-pending-removal
check_negative "$negative_r14_status_substitution" mutated_terminal_observation_is_exact_v1 r14-status-substitution
check_negative "$negative_r14_stop_cancel" mutated_stop_preserves_runtime_custody_v1 r14-stop-cancel
check_negative "$negative_r16_contract_substitution" mutated_custody_preserves_exact_contract_v1 r16-contract-substitution
check_negative "$negative_r16_dependency_bound" mutated_dependency_count_above_v5_bound_is_rejected_v1 r16-dependency-bound
check_negative "$negative_r16_handshake_downgrade" mutated_v4_handshake_is_rejected_v1 r16-handshake-downgrade
check_negative "$negative_r16_pre_custody" mutated_invalid_request_remains_pre_custody_v1 r16-pre-custody
check_negative "$negative_r16_reachability" mutated_terminal_response_preserves_reachability_v1 r16-reachability
check_negative "$negative_r16_response_custody" mutated_rejection_does_not_accept_backend_custody_v1 r16-response-custody
check_negative "$negative_r16_sidecar_scope" mutated_worker_and_sidecar_scope_predicates_are_distinct_v1 r16-sidecar-scope
check_negative "$negative_r16_sidecar_substitution" mutated_sidecar_contract_substitution_is_rejected_v1 r16-sidecar-substitution
check_negative "$negative_r16_terminal_reopen" mutated_terminal_response_seals_and_absorbs_v1 r16-terminal-reopen
check_negative "$negative_r16_variant_mismatch" mutated_variant_mismatch_is_rejected_v1 r16-variant-mismatch

# Detect source, checker, closure, or executable replacement during the run.
check_sources
check_digest "$expected_verus" "$verus_path"
"$closure_checker" "$verus_root" "$closure_manifest"

transcript='FE2O3_RUNTIME_MODEL_VERUS_OK lifecycle_obligations=2 identity_obligations=4 projection_obligations=4 memory_obligations=6 queue_obligations=11 load_plan_obligations=3 materialization_obligations=8 aql_obligations=11 r7_async_resource_obligations=8 r8_execution_contract_obligations=10 r9_native_evidence_obligations=14 r10_closed_execution_obligations=20 r11_runtime_semantics_obligations=18 r12_native_concurrency_obligations=23 r13_logical_scheduler_obligations=20 r14_async_observer_obligations=10 r16_worker_semantic_boundary_obligations=21 mutations=121'
actual_transcript=$(printf '%s\n' "$transcript" | "$sha256_path" | awk '{ print $1 }')
if [ "$actual_transcript" != "$expected_transcript" ]; then
    printf 'FAIL: verification transcript does not match the pin\n' >&2
    exit 1
fi
printf '%s\n' "$transcript"
