# Compiler child completion before artifact recovery

Issue: [#272](https://github.com/harsh-nod/fe2o3/issues/272).
Base: `6a0d5e754b697235570aded4aa11475c73edca4a`.

## Change

The native Cargo continuation previously documented successful compiler exit as
a caller precondition. It now checks the actual `std::process::Child` handle
against the child PID retained from issuer readiness and observes its exit
status. A supplied PID or `ExitStatus` cannot stand in for that handle.

The check is nonblocking. A running child, wrong child, zero selected PID,
nonzero exit, signal termination, or status-query failure refuses completion.
Waiting and process cleanup still belong to the enclosing supervisor.

The native readiness owner retains the successful observation on its original
budget. Finalization, publication acquisition, and receipt admission reject
unobserved completion. The ready-attempt finalizer requires the child handle
before consuming its recipe. No new execution selector, authority grant,
profile fallback, signature format, or artifact format is introduced.

The selected V1 wrapper uses the same observation after its existing successful
`wait`, before managed-attempt completion. This is an explicit identity check at
the readiness boundary; it does not replace the wrapper's existing wait,
failure revocation, or child cleanup.

## Validation

Commands used the pinned nightly-2026-04-03 toolchain, offline shared cache,
one Cargo job, disabled HIP, empty GPU visibility, and one test thread. Source
files were unchanged while each build ran.

- `cargo test -p cargo-fe2o3 --bin cargo-fe2o3 compiler_execution_boundary:: -- --test-threads=1`:
  **29 passed, 2 failed, 0 ignored**. All four new real-subprocess tests passed:
  running then successful exit, unrelated successful child, exit code 7, and
  signal termination. Their guards kill/reap their own children on assertion
  failure as well as on success.
- The two failures are existing child-channel tests:
  `application_verifier_gets_service_channel_without_policy_capability` failed
  in `Command::spawn` with `Stale file handle`; and
  `preparation_installs_exact_policy_and_child_created_service_channel` failed
  in channel `finish` with `InvalidServicePeer`. Neither reaches the new
  completion check. These failures remain unresolved, not skipped or weakened.
- `cargo test -p cargo-fe2o3 --bin cargo-fe2o3 binding_wrapper -- --test-threads=1`:
  **7 passed, 0 failed, 0 ignored**, including child cleanup and revocation.
  An earlier `binding_wrapper::tests::` filter matched zero tests and is not
  counted as validation.
- `cargo check -p cargo-fe2o3 --all-targets`: passed, with warnings including
  unused native continuations. Those warnings reflect remaining integration.
- Final focused reruns of `compiler_execution_boundary::native::` and
  `compiler_execution_boundary::completion::`: **20 passed** and **4 passed**,
  respectively, with no failures or ignored tests. These overlap the broader
  boundary run above; they do not clear its two child-channel failures.
- Changed Rust files pass `rustfmt --check`; `git diff --check` passes.

Logs are retained outside the source tree in
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921`:

| Log | SHA-256 |
| --- | --- |
| `compiler-child-completion-boundary-20260927.log` | `cd4f5abaefeebdc95a6c2a5642e84114b44d645d6a67964a56f8daecf63713ab` |
| `compiler-child-completion-wrapper-20260927-r2.log` | `b3fe063620ed9c92ca32561f64fd346b3b2db9603a27bdd8f99da0cd5c76587c` |
| `compiler-child-completion-check-20260927.log` | `c6da27acc80742d0e85555531420fd66928107fef376678aa4f97d6609a8b474` |
| `compiler-child-completion-native-20260927.log` | `16d0eaadaa6d1ee4ba584cd531a1888a80ae60ad82adc93149ca814cb4757aac` |
| `compiler-child-completion-focused-20260927.log` | `32bb75c930305036c652abe76095fdde932ff36609fed29fadd6607bd8974510` |

## Limits

This is not protected execution evidence, a positive native artifact roundtrip,
or a newly completed #272 milestone. Native V3 configuration/transport,
selected rustc publication, conditional recovery policy provenance, sealed
verification, and generated safe-host integration remain incomplete. The
production wrapper still selects V1. No new GPU or 47/47 claim is supported.

During this checkpoint, MI350 and both GitHub remotes failed DNS resolution.
No remote job was started. Native delegation also remained unavailable due to
the agent thread limit; this patch was implemented locally by the primary agent.
