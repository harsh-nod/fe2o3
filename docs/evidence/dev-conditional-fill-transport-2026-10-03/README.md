# Conditional Fill Transport

This checkpoint carries the guarded unsigned-32 fill proof through the singleton
compiler handoff and a distinct conditional importer. It does not admit a GPU
launch, complete multi-GPU application execution or establish HIP/HSA parity.

Parent commit: `2c5121198c362d5ec0376ae3010f2e8d32646b20`.

## Implementation

The bounded conditional codec retains the exact obligation preimage, embedded
verification key and signed boundary-4 receipt. It parses the closed condition
format, recomputes SHA256 of the exact preimage, reconstructs the expected
binding and strictly imports the signature, result and boundary. A separate
untrusted receipt inspection helper reuses the authoritative wire parser; it
does not verify signatures or grant authority. Serialization from live execution
also compares every retained coverage field with the live staging owner.

Ranked extent and logical source-output ordinals remain distinct. The codec
retains graph locations, allocation/noalias identities, element width in bits,
symbolic/static global-X extent, workgroup/subgroup dimensions, execution-domain
restriction and component receipt identities. A checksum or embedded-key
signature alone does not authenticate compiler origin.

The compiler retains a private total-or-conditional proof variant. Conditional
staging is freshly revalidated against the live ranked owner during custody
replay. The existing opaque V4 association transports the conditional bytes
without a protocol change. Conditional multi-root rosters remain unsupported
and fail closed.

The new move-only conditional compiler-input owner shares the unchanged
semantic/KIR stage decoder and lossless correspondence replay with the old V4
importer. It checks exact source, ranked and middle-end identities, a singleton
root, one effect and no unconditional TotalView or collective claims. A
separate target-lineage entrypoint shares the existing deterministic target/KIR
to LLVM replay. Neither result grants source-to-machine or runtime authority.

The old unconditional codec/importer and Worker protected-admission constructor
are not widened. Conditional bytes cannot produce the old proof-input owner.

The first genuine extraction exposed a previously unreachable translation-check
gap: KIR value normalization did not recognize the global-X invocation intrinsic
under the output's unsigned-64 to unsigned-32 cast. The narrow fix recognizes
only exact `global_id_1d()` with one exact `INDEX` result as ranked coordinate
symbol zero. The cast and mandatory expression-equality checks remain intact.
Other index kinds, axes, launch extents, result types and result counts reject.

A second genuine run reached an unrelated format gate: singleton preparation
first encoded a legacy multi-root correspondence payload that it subsequently
discarded. Singleton preparation now returns its existing canonical V5
correspondence and V4 formal admission directly after the same identity checks.
Their live-owner revalidation and neutral-KIR comparisons are preserved. The
legacy multi-root encoder/decoder is unchanged and is not qualified by this
singleton acceptance test.

A third run reached final correspondence custody. That check compared the inner
semantic SHA256 with the domain-separated outer receipt identity. Singleton
finalization now decodes the retained semantic record and compares both nested
semantic commitments with its inner digest. The genuine regression also asserts
that the two identity domains differ. Outer proof-association and target receipt
identities, neutral-KIR checks and all authority restrictions remain unchanged.
The analogous multi-root hash-domain mismatch is outside this singleton change
and remains unqualified, along with the legacy multi-root ordering issue.

## Qualification

Affected CPU qualification passes 914 tests:

| Suite | Passed | Default ignores |
| --- | ---: | ---: |
| Functional receipt import V2, internal staging | 7 | 0 |
| MIR-to-KIR lowering library | 149 | 0 |
| Verifier library | 146 | 7 |
| Compiler backend library | 524 | 0 |
| Middle-end V5 evidence, internal staging | 28 | 0 |
| Compiler proof binding V4, including conditional import | 25 | 0 |
| Verifier ownership/authority doctests | 35 | 0 |

Strict library Clippy passes for functional-proof, lower-mir-kernel, verifier and
compiler with `--no-deps -- -D warnings`. Targeted rustfmt and whitespace checks
also pass. The archive's `run-checks.sh` records the exact locked/offline commands
and pinned nightly/profile settings.

The separately selected protected tests pass:

- Conditional theorem: one test, including one proved profile and six rejected
  semantic mutants (underlaunch, reversed launch condition, off-by-one guard,
  wrong coordinates, false predicates and changed value), 73.95 seconds.
- Genuine compiler extraction: two tests, 84.20 seconds. The positive constructs
  the conditional V9 handoff, imports it, preserves the exact condition and
  replays target lineage without authority. The changed-reference case fails its
  proof and emits no handoff.

The protected runtime uses the unchanged pinned manifest
`ffef09bd240c90e72cbff31a82bc5173c796ba7ab9af239245e7ad892c25641c`.
Qualification uses private root-owned runtime provisioning in an isolated mount,
PID and network namespace, then drops to UID/GID 1000 with no capabilities for
the controller. `/opt` and `/tmp` are namespace-local; no global runtime install
or GPU execution is performed. Downloaded prerequisites are SHA256 checked.

Development failure logs retain the three genuine compiler gates described
above. They are failed intermediate candidates, not accepted evidence. Final
acceptance is in `protected-fill.log`; synthetic receipt fixtures establish
decoder and custody behavior only. No shared MI300X resources were used.

The code-only patch relative to the parent has SHA256
`e6371ec454d94174d5b601a2910fe030de3a1ca427ca265b7ae4ef71f5660214`.
The [qualification archive](qualification.tar.xz) contains that patch, exact
runner/provisioning scripts, final test logs and the three failed intermediate
compiler logs. Its SHA256 is
`1a233bac9e2c17c766e151da1fe1ce102b3d2f2f105d9ac6720ddf1ec7211fdf`.
Archive listing and extracted patch SHA256 were checked before cleanup.

## Remaining Multi GPU Path

Working admitted multi-GPU execution is the priority. Broader language, parity
and performance expansion is deferred until this vertical path passes.

1. Bind the condition to the exact semantic root and kernel descriptor. Reuse
   authenticated source-output/allocation relations and existing argument-plan
   validation. Never interpret the ranked argument ordinal as the source ABI
   ordinal; derive the physical offset from `SliceLengthU64`.
2. Discharge the condition against actual packed length, pointer fixup, backing
   buffer and checked byte extent, actual AQL grid-X and retained layout.
   Preserve the existing empty-slice representation. Bind the result to the
   selected kernel, packed invocation, device and prepared-dispatch identity
   before consuming one-shot application refinement.
3. Complete guard/index/address/value and dispatch-wide source-to-machine
   refinement and the protected conditional application provider. Then validate
   admitted fill, tracked upload, native XGMI peer transfer and readback for
   lengths 64, 65 and 4097 in both directions on freshly observed idle MI300X
   devices.

There is not yet a complete production two-GPU fill acceptance driver. Existing
native staging tests use synthetic input and cover the transport portion only.
The next machine-model step must derive wave inputs from descriptor/dispatch
state and prove dispatch-wide unique coverage, rather than accept caller-supplied
lane IDs or EXEC masks. Patched kernarg memory, source address/value relations,
completion visibility and the protected provider remain separate required joins.

Compiler extraction/projection and live classifier soundness remain trusted.
The existing conditional theorem proves coverage/value facts under `N <= G`;
this checkpoint transports and checks that obligation, not the actual launch.
Synthetic signed fixtures exercise decoding/custody only. Genuine protected
Rust extraction is the separate production-path test.
