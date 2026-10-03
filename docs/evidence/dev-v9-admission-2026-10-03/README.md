# V9 Singleton Admission Checkpoint

Base: `7b940ecdbc3760010767f9a4374c7d1662cd471b`.

This checkpoint removes two representation blockers on the multi-GPU protected-launch
path. It is not a source-to-machine proof, protected deployment, hardware execution,
multi-GPU acceptance result, or HIP/HSA performance claim.

## Changes

- Singleton V4 compiler-proof validation now retains exact canonical KIR V8 or V9.
  The correspondence version selects the matching strict decoder. Its verified
  digest and length must match both correspondence and formal-memory identities.
  V11 remains unsupported by this singleton contract; multi-root V11 is unchanged.
- The existing multi-root versioned owner is shared as
  `ValidatedCompilerKernelIrV1`. The old enum name is a re-export, preserving
  variant imports. No version conversion or fallback decoder is introduced.
- Write-only allocations already produce formal-obligation receipt V2. The V4
  formal-memory witness check now accepts its unchanged witness layout, after
  complete strict V1/V2 receipt validation. Previously this check rejected the
  live, successfully admitted write-only owner by assuming receipt V1.
- Host semantic-machine receipts, application admission and differential bindings
  explicitly retain and hash the KIR version. Existing V8-only local checked-add
  and physical-simulation adapters remain V8-only.

## Compatibility

`ValidatedCompilerProofInputsV4::kernel_ir()` now returns the versioned owner,
not `&VerifiedCanonicalKernelIrV8`. Version-specific consumers must use `as_v8()`
or `as_v9()` and handle unsupported input. Generic consumers use `canonical_bytes`,
`identity_digest`, `canonical_length` and `wire_version`.

The generated differential binding replaces `production_kir_v8_sha256/bytes`
with `production_kir_sha256/bytes` and adds `production_kir_version`.
These digests are version-domain-separated canonical identities, not raw byte
SHA-256 values. The semantic-machine receipt adds `kir_version()`.

All three host identity encodings now use V2 hash domains, including for V8 input.
Old cached identity hashes must be rebuilt. Public Rust type versions and hash
encoding versions are independent. No stored receipt is silently migrated.

## Qualification

Toolchain: `nightly-2026-04-03`; locked, offline Cargo; four build jobs; incremental
compilation and debug info disabled; test optimization 1 with debug assertions
and overflow checks enabled; one test thread; `FE2O3_HIP_SYS_DISABLE=1`.

- Compiler-proof V4 integration suite: 22 passed, none ignored.
- Legacy compiler-proof V3 integration suite: 6 passed, none ignored.
- Host library: 199 passed; the two-device native staging test remains ignored.
- Verifier library: 134 passed; three subprocess helpers and the pinned-runtime
  real-proof test remain ignored. Subprocess helper behavior is exercised by its
  parent tests.
- Genuine-rustc acceptance test: compiles, but its explicit ignored-test run
  failed at the required authenticated aggregate source-proof join, before
  emitting a handoff. This is an open acceptance target, not a passing test.
- Strict library Clippy passed for verifier, host and physical differential
  (`--lib -- -D warnings`). Changed Rust files pass rustfmt; whitespace checks pass.

The V9 positive uses synthetic semantic identities and a public test signing key,
but runs the actual ranked checks, source-site reconciliation, ordinary lowering,
same-owner formal admission and V4 decoding. It contains one real `GuardedStore`
and retains the exact V9 KIR and V2 obligation bytes. It is not a rustc extraction
or authenticated proof-execution claim. A separate codec-only test retains V9
encoding even when the module is representable in V8.

Negatives rebind outer receipt identities, exercising cross-version decoding,
independent and jointly substituted nested digests/lengths, malformed/truncated/
trailing KIR, singleton V11, wrong witness counts, V2-to-V1 downgrade and invalid
V1-to-V2 promotion. Existing span, parameter, induction and signature negatives
remain enabled. Initial fixture discovery failures are not qualification passes.

## Remaining Critical Path

The ignored `write_only_output_genuine_v9_singleton_proof_inputs_are_admitted`
acceptance target requests the actual Rust `fill_write_only` through the existing
V3 handoff, then requires aggregate proof import and exact target-lineage replay.
The explicit run reached production lineage preparation and failed with
`every production root requires authenticated MIR-to-PLIRON Verus execution`.
The current unannotated fill has no aggregate source-proof execution to retain.
Its source-proof composition must be implemented; merely provisioning Verus does
not fix this earlier gap. The test's own temporary build directory was removed.

The pinned retained Verus runtime is also absent at its
required `/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5`
path both locally and on MI300X.

The exact August 2 Verus distribution is cached locally, but provisioning also
pins rustup provenance, the host ELF interpreter and system libraries. Both hosts'
observed interpreter and libc hashes differ from the committed manifest. Copying
the distribution alone cannot satisfy that contract. Use the supported
`scripts/functional-refinement-verus-runtime-v1.sh` audits on a matching isolated
filesystem or complete a separately reviewed runtime-pin update; do not alter the
shared host loader or bypass authentication.

Besides these representation fixes, the complete source/neutral-KIR/optimized-target/machine
functional relation and protected refinement producer still need completion.
The [whole fill-wave model](../dev-fill-wave-2026-10-03/README.md) is a component,
not that composition. First hardware acceptance remains two-device launch and
readback, bidirectional peer transfer, synchronization, rejection paths and
bounded repeated teardown. No GPU was allocated by this checkpoint; MI300X was
only inspected. Cleanup was limited to regenerable build caches in this worktree.
