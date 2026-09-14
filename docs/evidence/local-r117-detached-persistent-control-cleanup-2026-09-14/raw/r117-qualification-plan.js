// Prospective exact commands, source identities and behavioral oracles. Not executed qualification.
module.exports = {
  "parent": "4a49234a03fc3759bd1d3ca330197619b4ae8ab0",
  "accepted": "4a49234a03fc3759bd1d3ca330197619b4ae8ab0",
  "previous": "docs/evidence/local-r116-context-version-settlement-2026-09-14/",
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
    "queue::dispatch_binding::control_release::tests::detached::detached_actual_commit_rejection_keeps_native_settlement_without_model_commit",
    "queue::dispatch_binding::control_release::tests::detached::detached_and_returning_wrappers_reject_wrong_modes_without_losing_outputs",
    "queue::dispatch_binding::control_release::tests::detached::detached_cleanup_allows_cancelled_later_reservation_and_exhausted_next_generation",
    "queue::dispatch_binding::control_release::tests::detached::detached_consuming_method_roots_owner_before_generation_validation",
    "queue::dispatch_binding::control_release::tests::detached::detached_currentness_boundaries_keep_exact_mapped_unmapped_and_disposed_custody",
    "queue::dispatch_binding::control_release::tests::detached::detached_generation_errors_precede_malformed_state_without_cleanup_entry",
    "queue::dispatch_binding::control_release::tests::detached::detached_generation_rejects_with_otherwise_valid_detached_state",
    "queue::dispatch_binding::control_release::tests::detached::detached_incomplete_callback_retains_active_owner_and_cannot_retry_or_extract",
    "queue::dispatch_binding::control_release::tests::detached::detached_native_errors_and_panics_preserve_each_control_and_destructive_prefix",
    "queue::dispatch_binding::control_release::tests::detached::detached_partial_unmap_never_advances_or_reconstructs_authority",
    "queue::dispatch_binding::control_release::tests::detached::detached_projection_failures_keep_native_receipts_and_uncommitted_model",
    "queue::dispatch_binding::control_release::tests::detached::detached_real_preparation_error_and_panic_keep_metadata_controls_and_data",
    "queue::dispatch_binding::control_release::tests::detached::detached_real_preparation_preserves_populated_metadata_and_separate_native_data",
    "queue::dispatch_binding::control_release::tests::detached::detached_state_cardinality_and_generation_mismatch_retain_exact_owner",
    "queue::dispatch_binding::control_release::tests::detached::detached_success_preserves_separate_data_metadata_order_and_zero_return_storage",
    "queue::dispatch_binding::control_release::tests::detached::detached_wrapper_retains_root_before_error_and_original_panic_and_drops_only_complete"
  ],
  "sourceGuard": "queue::dispatch_binding::control_release::tests::detached::detached_consuming_method_roots_owner_before_generation_validation",
  "retainedSourceGuard": "queue::dispatch_binding::control_release::tests::returning_consuming_methods_use_shared_root_before_validation",
  "mutations": [
    {
      "name": "constructor-metadata-loss",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "code_identity: owner.code_identity,",
          "code_identity: Vec::new(),"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::returning_populated_preparation_metadata_survives_rejection_and_cleanup",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/tests.rs",
      "oracle_line": 225,
      "expected": [
        "assertion `left == right` failed",
        "constructor changed original owner"
      ]
    },
    {
      "name": "reverse-control-order",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "self.code.next()",
          "self.code.next_back()"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::returning_success_preserves_exact_data_premises_storage_and_forward_order",
      "oracle_path": "crates/fe2o3-kfd/src/shared_memory/tests/pristine_abort/cleanup_tests.rs",
      "oracle_line": 303,
      "expected": [
        "assertion `left == right` failed",
        "exact native identities and order"
      ]
    },
    {
      "name": "admit-unrecycled-mode",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "ReturningControlModeV1::AfterRecycle => Some(self.generation.returned_generation()?),",
          "ReturningControlModeV1::AfterRecycle => Some(self.generation.returning_destroy_generation()?),"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::returning_modes_preserve_cancelled_history_max_recycle_and_exhaustion",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/tests.rs",
      "oracle_line": 425,
      "expected": [
        "assertion failed: matches!",
        "ResourcePhase"
      ]
    },
    {
      "name": "swallow-generation-rejection",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "ReturningControlModeV1::AfterRecycle => Some(self.generation.returned_generation()?),",
          "ReturningControlModeV1::AfterRecycle => Some(self.generation.returned_generation().unwrap_or(0)),"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::returning_generation_rejection_precedes_cardinality_and_preserves_full_owner",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/tests.rs",
      "oracle_line": 362,
      "expected": [
        "assertion failed: matches!",
        "Poisoned"
      ]
    },
    {
      "name": "skip-cardinality",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "if self.data.len() != self.data_premises.len() {",
          "if false && self.data.len() != self.data_premises.len() {"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::returning_cardinality_and_capacity_reject_before_native_entry",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/tests.rs",
      "oracle_line": 395,
      "expected": [
        "assertion failed: matches!",
        "InvalidData",
        "index: 4"
      ]
    },
    {
      "name": "skip-return-capacity-check",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "if self.returned.capacity() < self.data.len() {",
          "if false && self.returned.capacity() < self.data.len() {"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::returning_cardinality_and_capacity_reject_before_native_entry",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/tests.rs",
      "oracle_line": 403,
      "expected": [
        "assertion failed: matches!",
        "HostAllocationCapacity"
      ]
    },
    {
      "name": "permit-cleanup-retry",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "if self.started {",
          "if false && self.started {"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::returning_generation_rejection_precedes_cardinality_and_preserves_full_owner",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/tests.rs",
      "oracle_line": 245,
      "expected": [
        "assertion failed: matches!",
        "ResourcePhase"
      ]
    },
    {
      "name": "remove-active-custody",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "        let active = self.active_control.as_mut().expect(\"rooted active control\");\n        memory.release_control(active)?;",
          "        let mut active = self.active_control.take().expect(\"rooted active control\");\n        memory.release_control(&mut active)?;"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::returning_native_failures_retain_every_control_position_and_exact_prefix",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/tests.rs",
      "oracle_line": 468,
      "expected": [
        "failed control stays rooted"
      ]
    },
    {
      "name": "accept-incomplete-callback",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "if !active.is_complete() {",
          "if false && !active.is_complete() {"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::returning_incomplete_callbacks_cannot_advance_extract_or_retry",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/tests.rs",
      "oracle_line": 775,
      "expected": [
        "assertion failed: matches!",
        "ResourcePhase"
      ]
    },
    {
      "name": "extract-failed-output",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "if !self.complete || matches!",
          "if false && !self.complete || matches!"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::returning_native_failures_retain_every_control_position_and_exact_prefix",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/tests.rs",
      "oracle_line": 250,
      "expected": [
        "assertion failed: matches!",
        "ResourcePhase"
      ]
    },
    {
      "name": "reverse-returned-authorities",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "self.data.drain(..).zip(self.data_premises.drain(..))",
          "self.data.drain(..).rev().zip(self.data_premises.drain(..))"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::returning_success_preserves_exact_data_premises_storage_and_forward_order",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/tests.rs",
      "oracle_line": 317,
      "expected": [
        "assertion `left == right` failed",
        "Data {"
      ]
    },
    {
      "name": "corrupt-returned-premise",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "ReturnedDispatchDataLeaseV1 { authority, premise }",
          "ReturnedDispatchDataLeaseV1 { authority, premise: RetainedDataPremiseV1 { fully_initialized: !premise.fully_initialized, ..premise } }"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::returning_success_preserves_exact_data_premises_storage_and_forward_order",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/tests.rs",
      "oracle_line": 318,
      "expected": [
        "assertion `left == right` failed",
        "fully_initialized:"
      ]
    },
    {
      "name": "drop-error-root",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "    match result {\n        Ok(Ok(returned)) => Ok(returned),\n        Ok(Err(error)) => {\n            retain(root);\n            Err(error)\n        }\n        Err(payload) => {\n            retain(root);\n            resume_unwind(payload)\n        }\n    }",
          "    match result {\n        Ok(Ok(returned)) => Ok(returned),\n        Ok(Err(error)) => {\n            drop(root);\n            Err(error)\n        }\n        Err(payload) => {\n            retain(root);\n            resume_unwind(payload)\n        }\n    }"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::returning_wrapper_retains_full_root_before_error_or_original_panic",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/tests.rs",
      "oracle_line": 811,
      "expected": [
        "failure retained its complete root"
      ]
    },
    {
      "name": "drop-panic-root",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "    match result {\n        Ok(Ok(returned)) => Ok(returned),\n        Ok(Err(error)) => {\n            retain(root);\n            Err(error)\n        }\n        Err(payload) => {\n            retain(root);\n            resume_unwind(payload)\n        }\n    }",
          "    match result {\n        Ok(Ok(returned)) => Ok(returned),\n        Ok(Err(error)) => {\n            retain(root);\n            Err(error)\n        }\n        Err(payload) => {\n            drop(root);\n            resume_unwind(payload)\n        }\n    }"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::returning_wrapper_retains_full_root_before_error_or_original_panic",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/tests.rs",
      "oracle_line": 811,
      "expected": [
        "failure retained its complete root"
      ]
    },
    {
      "name": "late-return-capacity",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "        if let Some(generation) = generation {\n            if self.data.len() != self.data_premises.len() {\n                return Err(Gfx942DispatchBindingErrorV1::InvalidData {\n                    index: self.data.len().min(self.data_premises.len()),\n                    detail: \"retained data/premise cardinality\",\n                });\n            }\n            let capacity = self.data.len();\n            #[cfg(test)]\n            let capacity = self.return_capacity_override.unwrap_or(capacity);\n            self.returned.try_reserve_exact(capacity).map_err(|_| {\n                Gfx942DispatchBindingErrorV1::HostAllocationCapacity {\n                    operation: \"returning dispatch data\",\n                }\n            })?;\n            if self.returned.capacity() < self.data.len() {\n                return Err(Gfx942DispatchBindingErrorV1::HostAllocationCapacity {\n                    operation: \"returning dispatch data\",\n                });\n            }\n            self.returned_generation = Some(generation);\n        }",
          ""
        ],
        [
          "        if generation.is_some() {",
          "        if let Some(generation) = generation {\n            if self.data.len() != self.data_premises.len() {\n                return Err(Gfx942DispatchBindingErrorV1::InvalidData {\n                    index: self.data.len().min(self.data_premises.len()),\n                    detail: \"retained data/premise cardinality\",\n                });\n            }\n            let capacity = self.data.len();\n            #[cfg(test)]\n            let capacity = self.return_capacity_override.unwrap_or(capacity);\n            self.returned.try_reserve_exact(capacity).map_err(|_| {\n                Gfx942DispatchBindingErrorV1::HostAllocationCapacity {\n                    operation: \"returning dispatch data\",\n                }\n            })?;\n            if self.returned.capacity() < self.data.len() {\n                return Err(Gfx942DispatchBindingErrorV1::HostAllocationCapacity {\n                    operation: \"returning dispatch data\",\n                });\n            }\n            self.returned_generation = Some(generation);\n        }\n        if generation.is_some() {"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::returning_native_failures_retain_every_control_position_and_exact_prefix",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/tests.rs",
      "oracle_line": 498,
      "expected": [
        "return capacity precedes disposal"
      ]
    },
    {
      "name": "truncate-returned-output",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "        self.complete = true;",
          "        self.returned.pop();\n        self.complete = true;"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::returning_success_preserves_exact_data_premises_storage_and_forward_order",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/tests.rs",
      "oracle_line": 305,
      "expected": [
        "assertion `left == right` failed",
        "left: 4",
        "right: 5"
      ]
    },
    {
      "name": "detached-admit-unrecycled-generation",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "                    self.generation.returned_generation()?,\n                    expected_generation,",
          "                    self.generation.returning_destroy_generation()?,\n                    expected_generation,"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::detached::detached_generation_rejects_with_otherwise_valid_detached_state",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/detached_tests.rs",
      "oracle_line": 366,
      "expected": [
        "assertion failed: matches!",
        "ResourcePhase"
      ]
    },
    {
      "name": "detached-swallow-generation-error",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "                    self.generation.returned_generation()?,\n                    expected_generation,",
          "                    self.generation.returned_generation().unwrap_or(expected_generation),\n                    expected_generation,"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::detached::detached_generation_errors_precede_malformed_state_without_cleanup_entry",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/detached_tests.rs",
      "oracle_line": 219,
      "expected": [
        "assertion failed: matches!",
        "Poisoned"
      ]
    },
    {
      "name": "detached-ignore-state-validation",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "                validate_detached_persistent_control_release_state_v1(",
          "                let _ = validate_detached_persistent_control_release_state_v1("
        ],
        [
          "                )?;\n                None",
          "                );\n                None"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::detached::detached_state_cardinality_and_generation_mismatch_retain_exact_owner",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/detached_tests.rs",
      "oracle_line": 267,
      "expected": [
        "assertion failed: matches!",
        "ResourcePhase"
      ]
    },
    {
      "name": "detached-ignore-expected-generation",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "                    expected_generation,\n                )?;",
          "                    { let _ = expected_generation; self.generation.returned_generation()? },\n                )?;"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::detached::detached_state_cardinality_and_generation_mismatch_retain_exact_owner",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/detached_tests.rs",
      "oracle_line": 267,
      "expected": [
        "assertion failed: matches!",
        "ResourcePhase"
      ]
    },
    {
      "name": "detached-constructor-data-loss",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "            data: owner.data,",
          "            data: if matches!(mode, ReturningControlModeV1::DetachedPersistent { .. }) { Vec::new() } else { owner.data },"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::detached::detached_generation_errors_precede_malformed_state_without_cleanup_entry",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/tests.rs",
      "oracle_line": 225,
      "expected": [
        "assertion `left == right` failed",
        "constructor changed original owner"
      ]
    },
    {
      "name": "detached-allocate-return-storage",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "                None\n            }\n        };",
          "                self.returned.try_reserve_exact(1).unwrap();\n                None\n            }\n        };"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::detached::detached_success_preserves_separate_data_metadata_order_and_zero_return_storage",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/detached_tests.rs",
      "oracle_line": 150,
      "expected": [
        "assertion `left == right` failed"
      ]
    },
    {
      "name": "detached-convert-returned-data",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "        if generation.is_some() {",
          "        if generation.is_some() || matches!(self.mode, ReturningControlModeV1::DetachedPersistent { .. }) {"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::detached::detached_success_preserves_separate_data_metadata_order_and_zero_return_storage",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/detached_tests.rs",
      "oracle_line": 146,
      "expected": [
        "assertion `left == right` failed"
      ]
    },
    {
      "name": "detached-truncate-code-cleanup",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "        while let Some(code) = self.code.next() {",
          "        if let Some(code) = self.code.next() {"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::detached::detached_real_preparation_preserves_populated_metadata_and_separate_native_data",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/detached_tests.rs",
      "oracle_line": 973,
      "expected": [
        "assertion `left == right` failed"
      ]
    },
    {
      "name": "detached-unit-wrapper-admits-returning",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "        if !matches!(root.mode, ReturningControlModeV1::DetachedPersistent { .. }) {",
          "        if false && !matches!(root.mode, ReturningControlModeV1::DetachedPersistent { .. }) {"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::detached::detached_and_returning_wrappers_reject_wrong_modes_without_losing_outputs",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/detached_tests.rs",
      "oracle_line": 840,
      "expected": [
        "assertion `left == right` failed"
      ]
    },
    {
      "name": "returning-wrapper-admits-detached",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "        if matches!(root.mode, ReturningControlModeV1::DetachedPersistent { .. }) {",
          "        if false && matches!(root.mode, ReturningControlModeV1::DetachedPersistent { .. }) {"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::detached::detached_and_returning_wrappers_reject_wrong_modes_without_losing_outputs",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/detached_tests.rs",
      "oracle_line": 872,
      "expected": [
        "assertion `left == right` failed"
      ]
    },
    {
      "name": "detached-drop-error-root",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "    match result {\n        Ok(Ok(())) => Ok(()),\n        Ok(Err(error)) => {\n            retain(root);\n            Err(error)\n        }\n        Err(payload) => {\n            retain(root);\n            resume_unwind(payload)\n        }\n    }",
          "    match result {\n        Ok(Ok(())) => Ok(()),\n        Ok(Err(error)) => {\n            drop(root);\n            Err(error)\n        }\n        Err(payload) => {\n            retain(root);\n            resume_unwind(payload)\n        }\n    }"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::detached::detached_wrapper_retains_root_before_error_and_original_panic_and_drops_only_complete",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/detached_tests.rs",
      "oracle_line": 820,
      "expected": [
        "assertion `left == right` failed"
      ]
    },
    {
      "name": "detached-drop-panic-root",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "    match result {\n        Ok(Ok(())) => Ok(()),\n        Ok(Err(error)) => {\n            retain(root);\n            Err(error)\n        }\n        Err(payload) => {\n            retain(root);\n            resume_unwind(payload)\n        }\n    }",
          "    match result {\n        Ok(Ok(())) => Ok(()),\n        Ok(Err(error)) => {\n            retain(root);\n            Err(error)\n        }\n        Err(payload) => {\n            drop(root);\n            resume_unwind(payload)\n        }\n    }"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::detached::detached_wrapper_retains_root_before_error_and_original_panic_and_drops_only_complete",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/detached_tests.rs",
      "oracle_line": 820,
      "expected": [
        "assertion `left == right` failed"
      ]
    },
    {
      "name": "detached-reverse-control-order",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "self.code.next()",
          "self.code.next_back()"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::detached::detached_real_preparation_preserves_populated_metadata_and_separate_native_data",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/detached_tests.rs",
      "oracle_line": 973,
      "expected": [
        "assertion `left == right` failed"
      ]
    },
    {
      "name": "detached-remove-active-custody",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "        let active = self.active_control.as_mut().expect(\"rooted active control\");\n        memory.release_control(active)?;",
          "        let mut active = self.active_control.take().expect(\"rooted active control\");\n        memory.release_control(&mut active)?;"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::detached::detached_native_errors_and_panics_preserve_each_control_and_destructive_prefix",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/tests.rs",
      "oracle_line": 468,
      "expected": [
        "failed control stays rooted"
      ]
    },
    {
      "name": "detached-permit-cleanup-retry",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "if self.started {",
          "if false && self.started {"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::detached::detached_generation_rejects_with_otherwise_valid_detached_state",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/tests.rs",
      "oracle_line": 245,
      "expected": [
        "assertion failed: matches!",
        "ResourcePhase"
      ]
    },
    {
      "name": "detached-accept-incomplete-callback",
      "path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
      "edits": [
        [
          "if !active.is_complete() {",
          "if false && !active.is_complete() {"
        ]
      ],
      "test": "queue::dispatch_binding::control_release::tests::detached::detached_incomplete_callback_retains_active_owner_and_cannot_retry_or_extract",
      "oracle_path": "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/detached_tests.rs",
      "oracle_line": 747,
      "expected": [
        "assertion failed: matches!",
        "ResourcePhase"
      ]
    }
  ],
  "mutationSourceGroups": [
    [
      "accept-incomplete-callback",
      "detached-accept-incomplete-callback"
    ],
    [
      "admit-unrecycled-mode"
    ],
    [
      "constructor-metadata-loss"
    ],
    [
      "corrupt-returned-premise"
    ],
    [
      "detached-admit-unrecycled-generation"
    ],
    [
      "detached-allocate-return-storage"
    ],
    [
      "detached-constructor-data-loss"
    ],
    [
      "detached-convert-returned-data"
    ],
    [
      "detached-drop-error-root"
    ],
    [
      "detached-drop-panic-root"
    ],
    [
      "detached-ignore-expected-generation"
    ],
    [
      "detached-ignore-state-validation"
    ],
    [
      "detached-permit-cleanup-retry",
      "permit-cleanup-retry"
    ],
    [
      "detached-remove-active-custody",
      "remove-active-custody"
    ],
    [
      "detached-reverse-control-order",
      "reverse-control-order"
    ],
    [
      "detached-swallow-generation-error"
    ],
    [
      "detached-truncate-code-cleanup"
    ],
    [
      "detached-unit-wrapper-admits-returning"
    ],
    [
      "drop-error-root"
    ],
    [
      "drop-panic-root"
    ],
    [
      "extract-failed-output"
    ],
    [
      "late-return-capacity"
    ],
    [
      "returning-wrapper-admits-detached"
    ],
    [
      "reverse-returned-authorities"
    ],
    [
      "skip-cardinality"
    ],
    [
      "skip-return-capacity-check"
    ],
    [
      "swallow-generation-rejection"
    ],
    [
      "truncate-returned-output"
    ]
  ],
  "uniqueMutationCount": 28,
  "allowedMutationTests": [
    "queue::dispatch_binding::control_release::tests::detached::detached_and_returning_wrappers_reject_wrong_modes_without_losing_outputs",
    "queue::dispatch_binding::control_release::tests::detached::detached_generation_errors_precede_malformed_state_without_cleanup_entry",
    "queue::dispatch_binding::control_release::tests::detached::detached_generation_rejects_with_otherwise_valid_detached_state",
    "queue::dispatch_binding::control_release::tests::detached::detached_incomplete_callback_retains_active_owner_and_cannot_retry_or_extract",
    "queue::dispatch_binding::control_release::tests::detached::detached_native_errors_and_panics_preserve_each_control_and_destructive_prefix",
    "queue::dispatch_binding::control_release::tests::detached::detached_real_preparation_preserves_populated_metadata_and_separate_native_data",
    "queue::dispatch_binding::control_release::tests::detached::detached_state_cardinality_and_generation_mismatch_retain_exact_owner",
    "queue::dispatch_binding::control_release::tests::detached::detached_success_preserves_separate_data_metadata_order_and_zero_return_storage",
    "queue::dispatch_binding::control_release::tests::detached::detached_wrapper_retains_root_before_error_and_original_panic_and_drops_only_complete",
    "queue::dispatch_binding::control_release::tests::returning_cardinality_and_capacity_reject_before_native_entry",
    "queue::dispatch_binding::control_release::tests::returning_generation_rejection_precedes_cardinality_and_preserves_full_owner",
    "queue::dispatch_binding::control_release::tests::returning_incomplete_callbacks_cannot_advance_extract_or_retry",
    "queue::dispatch_binding::control_release::tests::returning_modes_preserve_cancelled_history_max_recycle_and_exhaustion",
    "queue::dispatch_binding::control_release::tests::returning_native_failures_retain_every_control_position_and_exact_prefix",
    "queue::dispatch_binding::control_release::tests::returning_populated_preparation_metadata_survives_rejection_and_cleanup",
    "queue::dispatch_binding::control_release::tests::returning_success_preserves_exact_data_premises_storage_and_forward_order",
    "queue::dispatch_binding::control_release::tests::returning_wrapper_retains_full_root_before_error_or_original_panic"
  ],
  "productionPaths": [
    "crates/fe2o3-kfd/src/queue_dispatch_binding.rs",
    "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs"
  ],
  "sourceDelta": [
    "crates/fe2o3-kfd/src/queue_dispatch_binding.rs",
    "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release.rs",
    "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/detached_tests.rs",
    "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/tests.rs",
    "crates/fe2o3-kfd/src/queue_dispatch_binding/preparation.rs",
    "crates/fe2o3-kfd/src/queue_dispatch_binding/preparation_tests.rs"
  ],
  "addedSource": [
    "crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/detached_tests.rs"
  ],
  "sourceCount": 5689,
  "baselineFullPassed": 2746,
  "fullPassed": 2762,
  "initialRuns": [
    [
      "r117-initial-format",
      [
        "cargo",
        "+nightly-2026-04-03",
        "fmt",
        "--all",
        "--",
        "--check"
      ],
      null
    ],
    [
      "r117-initial-controls",
      [
        "cargo",
        "+nightly-2026-04-03",
        "test",
        "--locked",
        "--offline",
        "-p",
        "fe2o3-kfd",
        "--all-features",
        "--lib",
        "control_release::tests::"
      ],
      "r117-initial-format.json"
    ]
  ],
  "prerequisites": [
    [
      "r117-reviewed-gnu-all",
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
      "r117-initial-controls.json"
    ],
    [
      "r117-reviewed-musl-all",
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
      "r117-reviewed-gnu-all.json"
    ]
  ],
  "focused": [
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
    [
      "queue_dispatch_binding.rs",
      "queue::dispatch_binding::Gfx942DispatchBatchV1",
      3047,
      3049
    ],
    [
      "queue_dispatch_binding.rs",
      "queue::dispatch_binding::Gfx942DispatchBatchV1",
      3057,
      3059
    ],
    [
      "queue_dispatch_binding.rs",
      "queue::dispatch_binding::Gfx942DispatchBatchV1",
      3065,
      3067
    ],
    [
      "queue_dispatch_binding.rs",
      "queue::dispatch_binding::Gfx942CompletedDispatchBatchV1",
      3091,
      3093
    ]
  ],
  "freezeHelpers": [
    "r117-run-v1.js",
    "r117-source-gate.py",
    "r117-auxiliary-gates.py",
    "r117-freeze.js",
    "r117-runner-tests.js",
    "r117-freeze-tests.js",
    "r117-qualification-plan.js",
    "r117-qualification-evidence.js"
  ],
  "freezeContractCount": 31,
  "qualificationContractCount": 117,
  "helpers": [
    "r117-run-v1.js",
    "r117-qualification-plan.js",
    "r117-qualification-evidence.js",
    "r117-qualification-tests.js",
    "r117-qualification-prepare.js",
    "r117-qualification-run.js",
    "r117-qualification-collect.js"
  ],
  "isolatedRuns": [
    {
      "name": "detachedcandidate-initial-format",
      "command": [
        "cargo",
        "+nightly-2026-04-03",
        "fmt",
        "--all"
      ],
      "before": "c3ffd75036ea2098ec9ddb8421b55e84ad6024ff4d394bc9fde25ba2f84ffe77",
      "after": "98fa3d784a1e47d475850200b586bcec795881a5661ef70bedce69571f0b5bb0",
      "code": 0
    },
    {
      "name": "detachedcandidate-initial-controls",
      "command": [
        "cargo",
        "+nightly-2026-04-03",
        "test",
        "--locked",
        "--offline",
        "-p",
        "fe2o3-kfd",
        "--all-features",
        "--lib",
        "control_release::tests::"
      ],
      "before": "98fa3d784a1e47d475850200b586bcec795881a5661ef70bedce69571f0b5bb0",
      "after": "98fa3d784a1e47d475850200b586bcec795881a5661ef70bedce69571f0b5bb0",
      "code": 101
    },
    {
      "name": "detachedcandidate-reviewed-controls",
      "command": [
        "cargo",
        "+nightly-2026-04-03",
        "test",
        "--locked",
        "--offline",
        "-p",
        "fe2o3-kfd",
        "--all-features",
        "--lib",
        "control_release::tests::"
      ],
      "before": "93c37126c85950c1f27b52520f41b8e3476078e51533535a82f2e642e3cac2ef",
      "after": "93c37126c85950c1f27b52520f41b8e3476078e51533535a82f2e642e3cac2ef",
      "code": 0
    },
    {
      "name": "detachedcandidate-initial-clippy",
      "command": [
        "cargo",
        "+nightly-2026-04-03",
        "clippy",
        "--locked",
        "--offline",
        "-p",
        "fe2o3-kfd",
        "--all-features",
        "--all-targets",
        "--",
        "-D",
        "warnings"
      ],
      "before": "93c37126c85950c1f27b52520f41b8e3476078e51533535a82f2e642e3cac2ef",
      "after": "93c37126c85950c1f27b52520f41b8e3476078e51533535a82f2e642e3cac2ef",
      "code": 101
    },
    {
      "name": "detachedcandidate-reviewed-format",
      "command": [
        "cargo",
        "+nightly-2026-04-03",
        "fmt",
        "--all",
        "--",
        "--check"
      ],
      "before": "60adae0665fc2995e98e6f647e630c26fbbe4c7e6e20eb603fec05c1986a8d4d",
      "after": "60adae0665fc2995e98e6f647e630c26fbbe4c7e6e20eb603fec05c1986a8d4d",
      "code": 0
    },
    {
      "name": "detachedcandidate-final-controls",
      "command": [
        "cargo",
        "+nightly-2026-04-03",
        "test",
        "--locked",
        "--offline",
        "-p",
        "fe2o3-kfd",
        "--all-features",
        "--lib",
        "control_release::tests::"
      ],
      "before": "60adae0665fc2995e98e6f647e630c26fbbe4c7e6e20eb603fec05c1986a8d4d",
      "after": "60adae0665fc2995e98e6f647e630c26fbbe4c7e6e20eb603fec05c1986a8d4d",
      "code": 0
    },
    {
      "name": "detachedcandidate-reviewed-clippy",
      "command": [
        "cargo",
        "+nightly-2026-04-03",
        "clippy",
        "--locked",
        "--offline",
        "-p",
        "fe2o3-kfd",
        "--all-features",
        "--all-targets",
        "--",
        "-D",
        "warnings"
      ],
      "before": "60adae0665fc2995e98e6f647e630c26fbbe4c7e6e20eb603fec05c1986a8d4d",
      "after": "60adae0665fc2995e98e6f647e630c26fbbe4c7e6e20eb603fec05c1986a8d4d",
      "code": 0
    }
  ],
  "isolatedArtifacts": [
    "detachedcandidate-final-controls-source-after.json",
    "detachedcandidate-final-controls-source.json",
    "detachedcandidate-final-controls.json",
    "detachedcandidate-final-controls.log",
    "detachedcandidate-initial-clippy-source-after.json",
    "detachedcandidate-initial-clippy-source.json",
    "detachedcandidate-initial-clippy.json",
    "detachedcandidate-initial-clippy.log",
    "detachedcandidate-initial-controls-source-after.json",
    "detachedcandidate-initial-controls-source.json",
    "detachedcandidate-initial-controls.json",
    "detachedcandidate-initial-controls.log",
    "detachedcandidate-initial-format-source-after.json",
    "detachedcandidate-initial-format-source.json",
    "detachedcandidate-initial-format.json",
    "detachedcandidate-initial-format.log",
    "detachedcandidate-negative-handoff.md",
    "detachedcandidate-reviewed-clippy-source-after.json",
    "detachedcandidate-reviewed-clippy-source.json",
    "detachedcandidate-reviewed-clippy.json",
    "detachedcandidate-reviewed-clippy.log",
    "detachedcandidate-reviewed-controls-source-after.json",
    "detachedcandidate-reviewed-controls-source.json",
    "detachedcandidate-reviewed-controls.json",
    "detachedcandidate-reviewed-controls.log",
    "detachedcandidate-reviewed-format-source-after.json",
    "detachedcandidate-reviewed-format-source.json",
    "detachedcandidate-reviewed-format.json",
    "detachedcandidate-reviewed-format.log",
    "detachedcandidate-run-v1.js"
  ],
  "isolatedContext": {
    "head": "4756971168f6b4f2c33d217f5d476ccde8ea2740",
    "cwd": "/home/harsh/.codex-tmp/fe2o3-detached-control",
    "contract": "detachedcandidate-raw-utc-boot-monotonic-v1",
    "runner": "detachedcandidate-run-v1.js"
  },
  "isolatedRunnerHash": "b84d2ed52df9eb2b3061d49e7e7eb2e5419c0ba933376fa0967f3da3d70a8fe9",
  "isolatedSourceCount": 5686,
  "isolatedPrevious": "docs/evidence/local-r115-returning-control-cleanup-2026-09-14/"
};
