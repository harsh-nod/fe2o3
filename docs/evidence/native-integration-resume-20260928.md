# Native Integration Resume

Date: 2026-09-28. Local parent:
`399b3c6aa3faf7e7921068666fcaa600a047f022`; upstream parent:
`238879e313f883f45a5e08cd48b96df6e562a637`.

This checkpoint integrates the unpublished native-execution work with the V18
source-layout and analysis foundation. It does not complete any issue #272
M0-M7 milestone, activate conditional production compilation, establish a
protected conditional Worker/publication roundtrip, or establish 47/47 GPU runs.

## Integration Findings

The automatic merge needed one semantic fixture repair: V5 descriptor arguments
now retain the semantic type's actual layout identity. The first all-target
check failed with E0063 on that missing field; the corrected Cargo/backend check
passed. It remains a warning-producing build, not a complete workspace test.

The 54 selected Cargo configuration, wrapper and compiler-boundary tests passed
before the subsequent ownership repairs. This includes both positive channel
tests previously refused by the restricted environment's socket-query policy.
These results must not be substituted for protected compiler execution.

The unsafe-site review found a safe fixed-slot intake API without an ownership
argument, two close-error paths that could retry a consumed descriptor, a
startup ordering mismatch, and an unguarded test-child polling failure. The
review also identified stale inventory entries for moved and added native
modules. The [unsafe policy](../unsafe-code-policy.md) records the reviewed
contracts; inventory counts alone cannot establish them.

Consuming inherited client/host APIs now require an unsafe ownership transfer.
The safe rustc dynamic factory cannot promise that transfer: its pinned loader
caches a function pointer, not a single backend instance, and initializes threads
before loading. The factory instead owns private close-on-exec duplicates. It
never closes the original slot numbers, which remain process/caller-owned through
compiler exit. Successful capture marks both close-on-exec; failed capture can
leave originals inheritable and does not take their cleanup obligations. Safe
callback admission consumes the stored input once per backend instance, not once
per process. Tests cover existing Rust owners, repeated capture, missing
inputs followed by descriptor-number reuse, admission and drop without admission.

Root Cargo.lock changes added the native components' dependency edges. The
tutorial manifest still named the upstream lockfile. Its four affected input
pins and their dependent canonical commitments are updated using the existing
validator functions. Kernel selections, source bytes, feature choices and
qualification obligations are unchanged. The Cargo configuration test module
is extracted without changing its module path or test bodies.

After updating the two exact snapshots affected by those identity changes, all
79 tutorial-manifest tests passed in 560.084 seconds. The earlier 300-second run
expired and is not a pass. Pending execution/qualification states are unchanged.

The broader client suite exposed an existing test-oracle mismatch: a transfer
with one unit less than its full work requirement returns the nested handoff
constructor's resource error, not an outer transport resource error. The test now
requires that exact nested work-limit variant. Its assertions that no connection
occurs and that the original storage floor and ledger survive are unchanged.

## Validation

Commands use pinned `nightly-2026-04-03`, `--locked`, one Cargo job, no incremental
builds, stripped test/dev debug info, a 12 GiB virtual-memory limit, a 1,200-second
outer timeout and serial test threads. No GPU device is exposed to these tests.
Logs are retained in the sibling
`fe2o3-issue272-production-next-evidence-20260921` directory.

| Check | Result | Log |
| --- | --- | --- |
| Expanded all-target compiler/Cargo/client/supervisor/anchor check | Passed with warnings | `resumed-ownership-check-20260928-r2.log` |
| Supervisor and anchor startup/ownership regressions | 29 passed | `resumed-ownership-ipc-tests-20260928-r2.log` |
| Client library and four inherited/channel integration suites | 66 top-level tests passed | `resumed-client-ownership-tests-20260928-r2.log` |
| Client/host consuming-admission compile-fail doctests | 4 passed | `resumed-owned-admission-doctests-20260928-r1.log` |
| Backend startup, image budgets, conditional V5 and layout regressions | 57 top-level tests passed | `resumed-backend-integration-tests-20260928-r1.log` |
| Cargo boundary, wrapper, configuration and application execution | 56 passed after ownership repairs and test extraction | `resumed-cargo-boundary-tests-20260928-r2.log` |
| Production configuration integration | 20 passed | `resumed-production-config-tests-20260928-r1.log` |
| Tutorial manifest | 79 passed | `resumed-tutorial-manifest-20260928-r3.log` |
| Reviewed unsafe-source inventory and tokenizer tests | 5 passed; maintenance helper ignored | `resumed-unsafe-inventory-gate-20260928.log` |
| Source-hygiene policy unit tests | 8 passed | Terminal output |

The two native inherited-child helpers are ignored in ordinary enumeration and
explicitly executed in isolated subprocesses by their passing parent tests.
The anchor regressions also require a fresh child-written completion marker;
an empty test selection cannot satisfy them. These tests are not protected
compiler or GPU execution. The initial client suite and its diagnostic rerun
failed on the test-oracle mismatch described above, and are not passes.
The refreshed inventory diff was checked against the reviewed native boundaries;
formatting checks and `git diff --check` passed for the manual integration edits.
This is selected integration coverage, not a full workspace test run.

SHA-256 identities of the retained logs:

```text
57757e9b24fd2fe2fc369f5df11e23b83758d3d88855f70d878a8d88e999a3bd  resumed-ownership-check-20260928-r2.log
5677e5aa60b0ade622d6b0c1337ea66948795aff11a03a573e518c7e912a574c  resumed-ownership-ipc-tests-20260928-r2.log
d55e9e9e766a5bdb35ef1379c9d4b6738d50fa0dd26f8ff353ba50ddf90e236a  resumed-client-ownership-tests-20260928-r2.log
384048b993208685b37b0fc2a9c71f818b5fcc44f9f1e5f205d2c264f26595ab  resumed-owned-admission-doctests-20260928-r1.log
fcf41b32bf001efb5c139384c3267bb9205acb597a0948fa21030a77d745b859  resumed-backend-integration-tests-20260928-r1.log
53247e29e2fd658373af488f3634bc78b5461b437f5b6d0d88abe10d2394bb87  resumed-cargo-boundary-tests-20260928-r2.log
e29b3067178892637c9c5758beacb46c09a8a1f5944a923a93533ee3f6fdffb1  resumed-production-config-tests-20260928-r1.log
6b9465be2b18c553abcca5aa6e2557f5d296dcb5b71db1714894a53b2d438ddc  resumed-tutorial-manifest-20260928-r3.log
02e9b8a10f5a96453afa7a52619fe03398418a51e8baa725f28b6310908c081c  resumed-unsafe-inventory-gate-20260928.log
```

## Recovered Environment

Read-only SSH audits reached mi350, mi350-2 and mi300x. Both MI350 hosts retain
the pinned runtime in Docker volume
`fe2o3-authoring-proof-native-20260917-r1`. Its runtime and interpreter trees
contain 90 regular files whose hashes match the current manifest and Rust
target pins. The image is:

```text
sha256:fd5370f370708f6a02cec6d44818a4295609e5bc68aa42455e53f141168a9d5f
```

The runtime and interpreter directories must be mounted read-only at their
canonical paths, with a non-root proof UID, no GPU/network, bounded writable
output and the existing proof controller's container settings. The retained
recipe is on mi350-2 at
`/home/harmenon/fe2o3-authoring-280-282.FEW3gj/tools/protected-runtime-v1/README.md`.
This is a file/metadata audit, not fresh runtime-lease admission or a proof run.
No shared runtime, container, service or remote scratch was changed by the audit.
The production compiler client profiles and installed service remain absent.

## Next Coherent Admission

This was the design at this checkpoint. The subsequent
[source-bound roster checkpoint](native-policy-roster-20260928.md) records its
partial implementation and the independently approved compiler/runtime policy
still required. The design alone is not a qualified API.

1. Capture an owned, bounded conditional policy roster inside the existing
   retained-owner callback in `production_native_conditional_source_packet_v2`.
   Read effect/formula policies from the actual successful proof owners and
   preserve them through the original account postchecks and publication.
2. Canonically encode exact length, root count, source-packet digest/length and
   source-ordered rows. Each row binds semantic root, kernel binding, effect
   signer set/toolchain and formula signer/toolchain/boundary. Decode is inert.
3. Version the inner conditional metadata to include the roster alongside the
   existing metadata. Update packing and `capsule_v5` accessors together; reject
   metadata downgrade. SubjectV3 already commits the complete capsule, so the
   existing execution receipt can bind this roster without another proof graph
   or signing service. Preserve the existing source-packet and execution wires.
4. Admit the roster in Cargo only under independent production-profile,
   protected issuer and approved compiler/runtime provenance. The delegation is
   explicit: that approved compiler nominates retained proof signers for this
   exact invocation and subject. Generic profile construction, embedded keys,
   closure equality or a self-consistent carriage do not supply this authority.
5. Match independent invocation custody, authenticate the current receipt, read
   the roster from the same publication token, validate its source/root/policy
   associations and recheck currentness before conditional recovery. Retain its
   owned backing and carriage in parent custody through finalization and
   persistence. Target and history limits remain independent inputs.
6. Standalone restart must independently reacquire trusted policy and fresh
   challenge-bound Worker/external-anchor currentness through `VerifyCurrent`.
   Until connected, authenticated standalone restart remains unsupported; do
   not generate replacement signer policies for historical receipts.

Acceptance requires genuine protected positive execution and rejection of
coherently resealed packet/roster substitutions, foreign issuer/profile/closure,
missing/reordered roots, source/toolchain/boundary mutation, downgrade,
publication turnover, stale currentness/journals and insufficient or substituted
accounts. Two compilations under the same runtime must produce independently
admissible ephemeral rosters and reject cross-use. Signer-policy admission does
not replace the publication/load/launch authority gates in issues #209 and #213.
