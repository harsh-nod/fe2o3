// Prospective integrated qualification plan. No mutation campaign has run.
const mutationPlan = require('./r119-persistent-mutations-v2.js');
const shared = new Set(mutationPlan.sharedSourceGroups.flat());
const mutationSourceGroups = [...mutationPlan.sharedSourceGroups,
  ...mutationPlan.mutations.filter(m => !shared.has(m.name)).map(m => [m.name])]
  .map(group => [...group].sort())
  .sort((a, b) => a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0);
const config = {
  "parent": "8e2c8532cb60918de523c6cab1b861bcd61fc319",
  "accepted": "8e2c8532cb60918de523c6cab1b861bcd61fc319",
  "previous": "docs/evidence/local-r118b-admission-lifecycle-2026-09-15/",
  "sourceMapSha256": "542110b3429161394392f24a398929aba37aa1d74950d0a86204adda2d0959d2",
  "sourceCount": 5693,
  "baselineFullPassed": 2780,
  "changedTestTarget": "fe2o3_kfd",
  "fullTest": [
    "cargo",
    "+nightly-2026-04-03",
    "test",
    "--locked",
    "--offline",
    "--no-fail-fast",
    "-p",
    "fe2o3-completion",
    "-p",
    "fe2o3-runtime-model",
    "-p",
    "fe2o3-resource-accounting",
    "-p",
    "fe2o3-kfd",
    "-p",
    "fe2o3-runtime",
    "--all-features",
    "--all-targets"
  ],
  "test": [
    "cargo",
    "+nightly-2026-04-03",
    "test",
    "--locked",
    "--offline",
    "-p",
    "fe2o3-kfd",
    "--all-features",
    "--lib"
  ],
  "newTests": [
    "queue::dispatch_binding::control_release::tests::persistent::persistent_cancelled_history_and_exhausted_next_generation_preserve_admission",
    "queue::dispatch_binding::control_release::tests::persistent::persistent_cardinality_and_capacity_reject_before_disposal",
    "queue::dispatch_binding::control_release::tests::persistent::persistent_consuming_methods_route_full_owner_before_validation",
    "queue::dispatch_binding::control_release::tests::persistent::persistent_currentness_failures_preserve_all_eighteen_control_boundaries",
    "queue::dispatch_binding::control_release::tests::persistent::persistent_generation_errors_precede_cardinality_and_capacity",
    "queue::dispatch_binding::control_release::tests::persistent::persistent_genuine_single_and_three_binding_preparation_preserve_metadata_and_data",
    "queue::dispatch_binding::control_release::tests::persistent::persistent_incomplete_callbacks_return_data_but_never_claim_control_completion",
    "queue::dispatch_binding::control_release::tests::persistent::persistent_native_errors_return_data_while_panics_retain_it_at_every_control",
    "queue::dispatch_binding::control_release::tests::persistent::persistent_output_capacity_survives_success_and_error_extraction",
    "queue::dispatch_binding::control_release::tests::persistent::persistent_panic_root_rejects_extraction_without_mutating_retained_custody",
    "queue::dispatch_binding::control_release::tests::persistent::persistent_partial_unmap_returns_data_without_losing_original_control",
    "queue::dispatch_binding::control_release::tests::persistent::persistent_projection_and_actual_commit_failures_preserve_receipts_and_output_split",
    "queue::dispatch_binding::control_release::tests::persistent::persistent_success_preserves_exact_mixed_data_and_forward_cleanup",
    "queue::dispatch_binding::control_release::tests::persistent::persistent_wrong_wrappers_and_repeated_extraction_reject_before_effects",
    "queue::dispatch_binding::control_release::tests::persistent::persistent_zero_data_uses_explicit_readiness_and_one_shot_transfer"
  ],
  "sourceGuard": "queue::dispatch_binding::control_release::tests::persistent::persistent_consuming_methods_route_full_owner_before_validation",
  "retainedSourceGuards": [
    "queue::dispatch_binding::control_release::tests::detached::detached_consuming_method_roots_owner_before_generation_validation",
    "queue::dispatch_binding::control_release::tests::returning_consuming_methods_use_shared_root_before_validation"
  ],
  "sourceDelta": [
    "crates/fe2o3-kfd/src/queue_dispatch_binding.rs",
    "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
    "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/persistent_tests.rs",
    "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/tests.rs"
  ],
  "addedSource": [
    "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/persistent_tests.rs"
  ],
  "focused": [
    [
      "persistent",
      "queue::dispatch_binding::control_release::tests::persistent::",
      15
    ],
    [
      "detached",
      "queue::dispatch_binding::control_release::tests::detached::",
      16
    ],
    [
      "returning",
      "queue::dispatch_binding::control_release::tests::returning_",
      15
    ],
    [
      "pristine",
      "pristine_abort",
      37
    ],
    [
      "cleanup",
      "shared_memory::tests::pristine_abort::cleanup_tests::",
      7
    ],
    [
      "transport",
      "queue::live::pristine_abort::tests::transport_tests::",
      9
    ]
  ],
  "docRelocations": [
    {
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding.rs",
      "from": 3049,
      "to": 3022
    },
    {
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding.rs",
      "from": 3059,
      "to": 3032
    },
    {
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding.rs",
      "from": 3067,
      "to": 3040
    },
    {
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding.rs",
      "from": 3093,
      "to": 3066
    }
  ],
  "prerequisites": [
    [
      "r119-integrated-gnu-all",
      [
        "cargo",
        "+nightly-2026-04-03",
        "test",
        "--locked",
        "--offline",
        "--no-fail-fast",
        "-p",
        "fe2o3-completion",
        "-p",
        "fe2o3-runtime-model",
        "-p",
        "fe2o3-resource-accounting",
        "-p",
        "fe2o3-kfd",
        "-p",
        "fe2o3-runtime",
        "--all-features",
        "--all-targets"
      ],
      "r119-integrated-control-focused.json"
    ],
    [
      "r119-integrated-musl-all",
      [
        "cargo",
        "+nightly-2026-04-03",
        "test",
        "--locked",
        "--offline",
        "--no-fail-fast",
        "-p",
        "fe2o3-completion",
        "-p",
        "fe2o3-runtime-model",
        "-p",
        "fe2o3-resource-accounting",
        "-p",
        "fe2o3-kfd",
        "-p",
        "fe2o3-runtime",
        "--all-features",
        "--all-targets",
        "--target",
        "x86_64-unknown-linux-musl"
      ],
      "r119-integrated-gnu-all.json"
    ]
  ]
};
module.exports = {...config, mutations: mutationPlan.mutations,
  mutationPaths: [mutationPlan.path], expected: mutationPlan.expected,
  uniqueMutationCount: mutationPlan.expected.distinctSources, mutationSourceGroups,
  allowedMutationTests: config.newTests.filter(name => name !== config.sourceGuard),
  isolatedSourcePrerequisite: mutationPlan.sourcePrerequisite};
