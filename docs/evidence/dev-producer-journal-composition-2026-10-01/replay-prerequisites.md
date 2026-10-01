# Public Packet Notes

This is a README proposal and support inventory, not a new evidence packet or
portable replay harness. Preserve every original byte. Lossless gzip wrapping
of each original tar is acceptable only with recorded compressed and original
SHA-256 values and complete decompression/member readback.

## Minimal Published Support

Publish the exact rejected-prefix and completion durable tar/member/receipt
triplets, the thin signed-source bundle, both audited readback programs and
their controls, and their accepted audit JSON records. A small digest manifest
and this scope/prerequisite explanation suffice; no new general harness is
required.

- Prefix tar: `cc3135a54ac6fa7271641a8fb9d7052075eba107070c25cee4bd754b4bc43ff1`.
- Prefix members: `87c0eab6bebfea56289cb338d4f0469d663709a8e42c68e43361411ae0e54a7e`.
- Prefix receipt: `20c3b2e314ee4e566fe18f7dac53798143f448d9922fdf407685a2be8a8028c7`.
- Completion tar: `9694c4cefdc4dd4c6267379bfa98ca587f3598e2f9e166156894995c4ea9227b`.
- Completion members: `dc44f452fc0a942c33856182d08cbb4b7093fa6ddf3607eb1b62cbd621989af9`.
- Completion receipt: `e29bab9010f13d64e75381384cfc65f031bd1adaa28e408f32e6ce0ae76f4dc9`.
- Thin source bundle, supplied by root: `d70ebb7934df63d123b62e76dc4fb7d22d3bc09ba7afce935054a0f3761ab736`.
  Head is `fef0f92cb6385c57d1b77c2eff41db967490beb1`; required public ancestor
  is `6793b910c2aa662df20763fbbd349cec0994677f`.

Both tars already contain the pre-sign binding `49c06cd5...`, signed-source
record `bdc7b0bd...`, all 24 changed candidate files, original signed campaign
owner, preparation and controls, complete actual proof projections and raw
executed stages. These are `raw-helpers/<original-absolute-path>.source`
members, joined by `raw-archive-index.json`. Neither tar contains a Git bundle
or a complete standalone Git repository.

The completion tar also contains the exact completion owner
`606c17a081fb9b6ae0449aa66c07f72b9c7b900e45f006c495e0cbd33d826716`,
its preparation and controls, and the rejected-prefix auditor, its controls,
and accepted prefix audit. Separate duplicate copies are optional conveniences,
not missing evidence.

The subsequent completion auditor and accepted composed audit are not in either
tar and must be added separately. All paths below are under
`/run/shm/fe2o3-a2-concrete-opacity-draft-20261001-audit`:

| File | SHA-256 |
| --- | --- |
| `audit_rejected_signed_concrete_v1.py` | `5635ffe856c94dd01e1f97fe4c43fc52012f8313b078abdf53533d542ff2a998` |
| `test_audit_rejected_signed_concrete_v1.py` | `ebd78514beeb5fb57d48934a959d74df1fabb4ce4585e5c1c3d9fa6ee8758a3a` |
| `rejected-signed-concrete-post-audit-v1.json` | `c5b3bda8d16123440f8a960a6fd8df25222be1d9fa9f616381349515e315ee1d` |
| `audit_concrete_completion_v1.py` | `d2dc6596ab60561808f2ce9bc82b12019c8551c7a90437a1f1c549e50c53606f` |
| `test_audit_concrete_completion_v1.py` | `23fffdb9e5528d37a983b93069152fd0587f36ee43edb2914e5100637980f199` |
| `concrete-completion-post-audit-v1.json` | `b120471f71930fecff00108532d9d5f1919639cd17b1b32f1037f9289f8ccec4` |

Root independently obtained the same accepted report contents. Root's own
wrappers/receipts may also be included to record that independent execution.

## Honest Replay Boundary

The packet supports source/diagnostic/raw-stage and archive inspection. It is
not a self-contained, relocated, hermetic execution environment. The original
auditors intentionally reconstruct local current custody and refuse missing
or changed external inputs; they are not standalone scripts that run after
extracting only these two tars. The bundle restores signed source only when
its public ancestor is present. No absolute-path rewrite is implied.

Exact full-auditor replay additionally requires:

1. The original canonical source/helper/evidence paths in `raw_pins` and their
   inherited preparation trees. Archived helper bytes can be restored only to
   their exact canonical paths, without substituting altered helpers. The
   original concrete calibration tree has 1,803 path/hash entries under
   `original_capture_tree`; it is not recursively copied into these tars.
2. The four exact historical durable triplets enumerated in
   `preserved_durable_history`: rejected journal capture, opacity frontend,
   opacity experiment and concrete opacity capture. Their full tar bytes are
   externally retained, not nested in the new archives. Their tar hashes are,
   respectively, `45786c406b7c64925fd8793187f1243a337d8544b9a307c8e9ffb8d2628e3b71`,
   `58a5e866ff59efde5b0dfcff9e92a6d480ab20cb1b42709d0fda299038219d4c`,
   `e23d30654983d071dae35532d562e4781c904eb5bed9efd416610d0295f479a4`,
   and `6463dc53a1b6b817fc8723b12a8bc6a6980b8f0ee9b1c301b886dfa7894304c9`.
3. The two original/retained CPU ELF copies named in
   `preserved_cpu_artifact`, each 43,514,320 bytes, SHA-256
   `693f870199f0dbf0be4963f4501285de460d9681d65c024cc9e4ea503499e153`.
   They are custody prerequisites only, not CPU qualification of this source.
4. All 28 exact tool/signer-file entries in `tools`, plus the source-pinned
   Verus release closure. These include Verus `0.2026.08.09.92f466f`, its
   `rust_verify` and Z3, the recorded Rust toolchain, Python, Git, shell and
   core utilities. Dynamic loader and host libraries are not hermetically
   captured. Signed-source verification requires the recorded public allowed
   signers file, not a private signing key.
5. Both original packet trees and durable triplets at their exact bound paths,
   preserving files, modes, member maps, receipts, and missing-original-closing
   record. Saved fresh group censuses are read back without probing old PIDs.

The authoritative complete prerequisite maps are the archived prepared JSON,
not this explanatory list. Do not claim complete local replay from the compact
public packet alone. Future re-execution must use fresh namespaces and reviewed
bounds; rerunning an old owner into its existing packet is not supported.

## Suggested Result Summary

Signed actual journal/validator/fold composition is qualified by explicitly
composed 109+3 evidence. The original 112-stage campaign remains RAM-rejected.
Its independently audited 109-stage prefix contains 89 unique actual-body
negative observations and eight positives. Exactly three missing closing
commands were freshly executed under unchanged proof bounds, producing the
ninth positive and release/signature checks. All 109 original and three new
groups closed. Calibration captures were not promoted or rerun.

Each leaf/conditional/concrete root has three positive brackets at 42/0,
64/0 and 214/0, respectively; these counts are not additive runtime properties.
Eight forwarding faults exercise result equality, one only ghost trace.
Live/credit freshness, Arc/mutex/shared-state, compiler/ISA/GPU execution and
performance remain outside this component. A2 and broader milestones are not
declared complete. This packet adds no new CPU/GPU/performance result.
