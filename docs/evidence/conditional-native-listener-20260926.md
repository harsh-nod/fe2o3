# Native Session and Listener Checkpoint

Date: 2026-09-26. Continuation of
[native consuming/session work](conditional-native-consuming-v3-20260926.md),
for issue #272. M0-M7 and the 47/47 production-to-safe-GPU-launch matrix remain
incomplete. This checkpoint grants no protected-proof, numerical-refinement,
listener-to-issuer execution or GPU credit.

Follow-up: [native controller and provisioning checkpoint](conditional-native-provisioning-20260926.md)
adds finite dispatch, native deployment/bootstrap records and guarded key reissue.

## Implementation

Base: `756c481c7bc8dc9976a19e2669c5fa364ac9bece`.
Integrated session fixtures: `51886d14c`.
Listener implementation: `431de739cbe79e0c22bac51d8bbcde14257536cb`.

- Separate V2/V3 isolated coordinators call the public `run_session` API for
  readiness success, missing EOF and trailing-byte refusal. They retain the
  original request ledger, separately funded cleanup, terminal reap checks and
  descriptor accounting. Public readiness must end in EOF without unread bytes.
  Existing explicit-stage fixtures remain separate.
- The missing-EOF session case has a three-second readiness bound. It establishes
  bounded refusal, not an independent witness that a complete frame was written.
  Drop-before-readiness remains an explicit-stage case, not a session case.
- `ProtectedIssuerServiceV2` and `ProtectedIssuerServiceV3` consume their actual
  native supervisor and the fixed-path inherited listener. Shared socket custody
  contains filesystem and descriptor facts, not legacy policy authority.
- `serve_one` prepays finite acceptance attempts, checks continuity before and
  after accept, and dispatches the matching native session on the original budget.
  Late accepted descriptors are closed. Exited owners retire before the final
  continuity check; only inert observations escape. Cleanup remains independently
  funded and controller-owned. A continuity failure takes precedence over the
  session result, matching the legacy dispatch contract.
- The original V1 listener and worker path retain their existing admission
  predicates and diagnostics. Native single-dispatch APIs do not activate native
  deployment, provisioning, recovery or a parallel worker pool.

A native worker implemented the session fixtures in a private worktree. The
primary integrated them and owned all builds. Read-only review found no concrete
listener defect, but identified missing tests through genuine native service
owners: bind/dispatch accounting boundaries, post-accept descriptor retirement
and post-session continuity precedence. Helper tests do not close those gaps.

## Validation

Pinned `nightly-2026-04-03`, offline, one Cargo job at a time, no incremental
compilation. Local logs are retained under
`/home/harsh/work/fe2o3-issue272-production-next-evidence-20260921/`.

| Check | Observed result |
| --- | --- |
| Focused native supervisor tests, R2 | 87 passed, 1 failed, 28 ignored, 176 filtered out |
| Supervisor doctests | 4 compile-only examples and 119 compile-fail examples passed |
| Six-package all-target check | Passed: supervisor, issuer, client, host, cargo-fe2o3, rustc-codegen-fe2o3 |
| Session role discovery from built libtest JSON | Six coordinators and four helpers uniquely registered and ignored |
| Static V3 fixtures, R3 | All four modes built and passed ELF checks; checksum verification passed |
| Changed Rust formatting and whitespace | Passed |
| Hygiene delta against the base above | Passed at the listener implementation commit |
| DCO through the listener implementation commit | 83 signed-off commits, no inherited exceptions |

The failed test is
`listener::native_accept::tests::named_socket_accepts_exact_control_and_rejects_path_drift`.
The environment rejects `bind` with `EPERM` at
`listener_native_accept_tests.rs:127`, before socket admission or acceptance.
This failure was neither skipped nor converted into a pass. The focused suite
returned 101 and is not a full-suite success. Its private socket directory was
cleaned up. All-target refers to Cargo target kinds, not GPU architectures.
Discovery is not execution: none of the six new isolated session cases ran.

### Static V3 Artifacts

The third bounded build completed at the listener implementation commit using
the existing warm musl cache. The script preserved its per-mode 900-second wall
and CPU limits, 8 GiB address-space limit, one build job and frozen offline
dependencies. It built `ready`, `no-eof`, `trailing` and `silent` separately.
Every artifact is x86-64 ELF64 `ET_EXEC`, enters at `fe2o3_secure_start_v1`, has
no dynamic loader/dependencies or undefined symbols, and has a non-executable
stack. No issuer was executed.

Artifacts, build logs, ELF reports and `SHA256SUMS` are retained in
`/home/harsh/work/.fe2o3-native-v3-static-20260926.Y0FkqPh3/`.
The empty temporary directory was removed. To make build headroom, 377,646,774
unique bytes of inactive backend debug library artifacts were removed from our
private cache while Cargo was idle. No source, reports, active release artifacts
or remote files were removed.

Artifact SHA-256 values, relative to that output directory:

```text
e7f221c407530365668a75e67a39be3afce0362aa30ad94397c6428a43839d90  native-ready-fixture-v3-ready
0f0831554c6cf4358b40bc63881b0d8e9e666f050c321dae3bc59c30ee020139  native-ready-fixture-v3-no-eof
03ce673f146fc3649affe1d6a5f3a146a644ebe02e11c8b589eb9180d6ec53f7  native-ready-fixture-v3-trailing
05be887acb920c52d9f9bb9250a0cbe9fa1b0a1a6b7bdf03dbb41d6f1e5e8ff9  native-ready-fixture-v3-silent
```

## Evidence Digests

SHA-256 for files in the evidence directory above:

```text
8958ab4f151baf829207f59956844bb795b862093eb077144e1576c71d0ae2c3  conditional-native-listener-tests-r2.log
67751f337a1021d5e6ab620e5e07a323b91346e26a81cc9489ec844e563f7fd6  conditional-native-listener-doctests-r1.log
bdb9aad55be2acb28b887dc1d9d7b4bd705e1ac10c10c03f5dc261eb5ef0c39e  conditional-native-listener-integrated-check-r1.log
e468e71a2caddddd881adf699ebceb129326c232420f69c9357e29b3e451e91f  conditional-native-listener-hygiene-r1.log
3eaa8cae550c7519674f81a767299c61066d09c0153b51e5d0a6817c08f6abef  conditional-native-session-discovery-r1.jsonl
23122070728257cd728a23d6df8e0487a208031d691b82d710b5243825b9de2c  conditional-native-consuming-v3-static-build-r3.log
```

## Remaining Gates

1. Execute the V3 consuming cases and both families' public session cases in the
   isolated protected profile with the separately measured static launcher.
2. Exercise the genuine native listener-to-issuer path, accounting refusals and
   continuity precedence. Integrate native provisioning, deployment and recovery
   with independent parent/runtime/policy pins and persistent cleanup ownership.
3. Connect native admission, conditional preparation, postchecks, invocation
   completion, revalidation, V5 publication and SubjectV3 acquisition/transport
   through the normal compiler entry point. Normal backend admission remains V1;
   conditional publication still refuses.
4. Complete machine/numerical refinement and the protected gfx942/gfx950 GPU
   matrix, plus generic non-AMD coverage. Permitted numerical differences require
   explicit compiler-proved bounds, not unchecked test tolerances.

Fetches from both GitHub remotes and probes of mi350, mi350-2 and mi300x failed DNS
resolution during this checkpoint. No remote job or scratch directory was
created. Remote main state remains unverified; synchronization must use normal
fast-forward pushes after reconciliation, never a forced update.
