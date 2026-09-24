# XGMI Backing-Budget Witness: CPU Qualification

This packet qualifies the example and sibling campaign described in the
[witness contract](../../runtime-xgmi-backing-budget-witness-v1.md), not their
native execution. The production implementation remains unchanged from signed
`685879ab5b3a3e0b31c10f4a44944e0e0962e6ff` and its separate
[full library qualification](../dev-xgmi-backing-budgets-cpu-2026-09-24/README.md).

## Final Campaign

`cpu3` passes all twelve serialized stages from `runner.py`:

- Five example tests pass on GNU and five on musl, without ignores or filters.
- The default-feature CLI builds; its invalid arguments reject before native
  initialization in the compiled-CLI Python test, which runs without skipping.
- Twenty Python tests pass: five strict receipt tests, nine campaign tests and
  six native-runner orchestration tests.
- Formatting and all-feature/all-target runtime Clippy with `-D warnings` pass.
- Rust/Cargo identities are byte-identical before and after the campaign, and
  all 3,917 selected source hashes are unchanged.

The pinned toolchain is `nightly-2026-04-03`. Test optimization is level 1 with
debug assertions enabled and debug information disabled; incremental
compilation is disabled and builds use two jobs. The ordinary CLI build uses
the default development optimization. The exact environment and time bounds
are frozen in the runner, whose SHA256 is
`06c4c68c25ac96009d749bc49f8f6981b1703f62ceef250eb9faa53fc00153e5`.
Raw command receipts, output, process-group cleanup and source brackets are
retained. This is host test evidence, not a hermetic build or native receipt.

The Rust tests cover CLI identities, asymmetric pressure arithmetic, all-byte
payload/guard mutations, exact ordered accounting/custody fields, and primary
error preservation. Python tests cover exact receipt framing and every field,
ordered identities, fixed controls, nested boolean substitutions, six observer
endpoints, no retry after failure, postflight precedence, changed source/ELF/host
identities, pre-import authentication, tampered remote bootstrap input, scrubbed
Git authority, qualified source ancestry/deltas, and collection-before-cleanup.
Runtime cleanup and remote owned-directory/process cleanup are distinct facts.

## Preserved Development History

`cpu1` passed formatting and five GNU example tests, then was intentionally
interrupted during musl compilation after review found harness authentication,
Git-environment and cleanup-label issues. Its final receipt records interruption
and reaping. It is not a passing qualification.

`cpu2` passed its twelve stages, including nineteen Python tests, after those
corrections. It is superseded: final review then added top-level `examples` to
the qualified source-delta check and a hostile workspace-example test. `cpu3`
reruns the whole campaign on that final source; no old passing stage substitutes
for a final-stage result. The runner is identical across all three attempts.

No GPU workload, native fault injection or performance measurement is included.
The next step is a signed clean-checkout MI300X campaign with freshly admitted
endpoints, independent complete-buffer checks, exact collection and owned
cleanup. The private target cache is retained temporarily for that handoff;
raw logs are retained independently. A1/A2, aggregate memory, formal production
correspondence and HIP/HSA parity remain open.
