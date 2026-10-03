# Conditional Packed Coverage

This checkpoint binds the retained conditional fill proof to the exact compiler
ABI and checks coverage against actual packed arguments and AQL geometry. It
does not admit a GPU launch, authenticate device storage, complete application
multi-GPU execution or establish HIP/HSA parity.

Parent commit: `db1e854d3d8d95a4de90df93385863b9f481dfc3`.

## Implementation

`ConditionalOutputArgumentBindingV1` performs existing deterministic target
replay once per artifact and decodes the descriptor from that exact capsule's
ABI receipt. It binds the singleton semantic root/export/kernel identity,
source-output ownership, actual KIR V9 U32 WriteOnly parameter and retained
semantic-to-KIR parameter correspondence. The target must be gfx942:xnack-,
the code object version V6 in both owners, and the export manifests identical.

The generated layout passes existing full descriptor/packing validation. The
selected source-output ordinal derives its physical pointer and SliceLengthU64
offsets from that descriptor, never from the ranked extent ordinal. The current
compiler profile requires Index1D U32 output, exact allocation/noalias ordinal
relations and workgroup/subgroup size 64.

The plan and packed owner now share immutable typed ABI fields through Arc.
Borrowed, owned and charged packing preserve those fields and pointer width;
the checker compares values, not Arc addresses. Equivalent independently built
plans remain valid. A same-width I32 output plan with an identical physical V1
packing observation rejects, including for empty output. The public observation
schema and hashes are unchanged.

`CheckedConditionalPackedCoverageV1` borrows both the binding and actual packed
storage. It checks exact kernel, typed fields, components, kernarg size/alignment
and bytes; derives N from the actual length component; checks 4*N without
overflow; and requires the unique selected WriteOnly buffer and exact pointer
fixup. Empty output preserves the existing no-buffer/no-fixup representation.
Overlapping or reused fixups and ambiguous buffer observations reject.

The actual AQL geometry must satisfy the descriptor and condition, one-dimensional
grid, Y/Z equal to one, exact workgroup, optional static extent and full-workgroup
restriction, and N <= grid-X. Per-invocation checking is linear in kernarg bytes
and bounded metadata, not output length; it never rehashes output payloads.
Typed field retention is one Arc clone, not a deep copy per invocation. Existing
result-credit accounting is unchanged; this is not a claim that all metadata
allocation bytes are charged to that result budget.

The checked value is move-only and cannot outlive its packed owner. It cannot
patch pointers, select a device, prepare or issue work, publish results, or enter
the existing protected Worker constructor. The same checker body is reused for
borrowed, owned and charged argument owners. Charged checks preserve credit and
observer state, including rejection and disposal.

## Qualification

Affected CPU qualification passes 273 tests:

| Suite | Passed | Default ignores |
| --- | ---: | ---: |
| Host library | 211 | 1 |
| Host ownership and authority doctests | 37 | 0 |
| Compiler proof binding V4, including conditional import | 25 | 0 |

Strict host/compiler library Clippy passes with `--no-deps -- -D warnings`.
Targeted rustfmt and whitespace checks pass. The archived `run-checks.sh`
records exact locked/offline commands and pinned nightly/profile settings.

The separately selected genuine protected compiler pair passes 2/2 tests in
99.94 seconds. The positive constructs and imports the real conditional V9
handoff, replays target lineage, builds the public binding, then checks actual
borrowed and owned typed packing for N=0/1/63/64/65/4097. Underlaunch rejects.
Valid replacement ABI and altered proof-association receipts with recomputed
outer identities reject at the exact retained ABI/proof join. The proof mutant
is not a second valid signed proof. The existing changed-reference negative still
rejects a changed reference value and emits no accepted handoff.

CPU adversarial controls cover distinct ranked/source ordinals, types and
mapping, pointer/length bytes, empty representation, overflow, buffer access and
extent, missing/duplicated/straddling/reused fixups, ambiguous observations,
geometry, same-width signedness substitution, original-plan disposal, and
charged result-credit/observer custody. Synthetic signed fixtures exercise
decoder and adapter behavior only; genuine extraction is the separate test.

The protected run uses the unchanged pinned runtime manifest
`ffef09bd240c90e72cbff31a82bc5173c796ba7ab9af239245e7ad892c25641c`.
Private root-owned provisioning runs inside isolated mount, PID and network
namespaces, then drops to UID/GID 1000 without capabilities for the controller.
`/opt` and `/tmp` are namespace-local, prerequisites are SHA256 checked, and no
shared MI300X resource or GPU execution is used. The preexisting conditional
theorem body is unchanged and was not rerun as a new proof claim.

The code-only patch relative to the parent has SHA256
`75a2a49064659c5a420970e7a6dab06b4facbcd7f679be8ee025d0b53f129960`.
The qualification archive retains that patch, runner/provisioning scripts,
accepted logs and two intermediate test-compilation failures. Those failures
were literal/budget integer type mismatches, not accepted results. A subsequent
test-only module-path typo was also fixed before the accepted CPU run.
The [qualification archive](qualification.tar.xz) has SHA256
`26feaf24d00ecae3da893fd5ea186aa885e775d53f4857515a049069c13b7376`.
Its listing and extracted code-patch digest were checked before cleanup.

## Remaining Critical Path

The protected Worker artifact verifier currently takes unconditional compiler
inputs before invocation arguments exist. It needs a distinct conditional
artifact profile retaining the universal machine contract, followed by actual
per-invocation discharge tied to the prepared dispatch, device and patched
storage. No old unconditional admission gate was widened here.

Complete the existing per-wave machine model's descriptor/dispatch input join,
source index/guard/address/value relation, dispatch-wide unique coverage, memory
validity and completion visibility. Then implement the protected conditional
provider and qualify genuine admitted fill -> tracked upload -> native XGMI ->
full guarded readback for N=64/65/4097 in both device directions. The existing
result-staging witness supplies the transport portion but currently starts with
synthetic output, not an admitted application completion.

The signed proof and deterministic target replay do not themselves authenticate
the compiler producer. This new host checker is implementation/test evidence,
not a new whole-adapter formal proof. Existing native multi-GPU qualification
remains scoped to its recorded binaries and profiles.
