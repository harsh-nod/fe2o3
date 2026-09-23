# Historical hardware resource capture — 2026-09-23

This is a bounded capture/export and local-view increment for #281, not V3/V4
acceptance. Original accepted exits remain M1/V1/V2/U1/U2/U3 (6/18).

## Implemented path

The explicit `fe2o3-debug capture-rocgdb-kfd-resources-v1` command reuses the
existing native V5 launcher, admission and inspection path. Legacy V4/V5 defaults
and wire formats are unchanged; no extra MI inspection commands are introduced.
Only a retained, freshly rechecked same-stop projection can move out, once, after
successful final locals inspection or its explicit unsupported-command result.

The output adapter independently joins artifact/process/dispatch/association,
queue occurrence, geometry, full stopped scope and every register/evidence/value
binding. Available scalar/predicate bits, unavailable reasons and PC redaction
are preserved. The output-only DTO is historical data; it has no live-owner
deserialization or control authority.

The closed writer validates before output, uses fallible geometrically growing
storage and caps one JSON record plus LF at 2 MiB. Invalid/capped serialization
emits no partial record; output I/O itself may fail after a partial write.
Emission follows the existing teardown path's return, not a new confirmed-reap
guarantee.

The separate optional browser panel validates the entire bounded file/roster
before rendering, retains exact wide identities, and pages 64 of at most 1,024
rows. Duplicate/unknown fields, bad framing, inconsistent scopes, invalid values
and stale replacements refuse. Selection, cancellation and reset cannot restore
late reads. There is no upload, local storage, compiler action or live session
restoration. A consistently forged file remains untrusted.

## Compiler validation

The full protocol/CLI gate passed **383 tests**, three ignored, strict all-target
Clippy with warnings denied, and the CLI binary build. It includes 25 new
synthetic tests. Receipt:
`logs/phase28-resume-r4-compiler-hardware-resource-capture-r2/receipt.json`,
21,838 bytes, SHA-256
`eb3b36160a3de9420ab0bb3d929348c4e69b8380eba6d9cd11d75a87fbbab6ec`.
Source: 6,769 files / 102,178,051 bytes, SHA-256
`ec8964542a3f1c77af50f55d9703b6bf03c0c5aedc068fb50d6d9a4e98348a9d`.

Two additional PC-redaction/unavailable-reason preservation tests then passed
in the full CLI library rerun: **166 passed**, one ignored. Strict protocol/CLI
all-target Clippy, selected formatting and whitespace also passed. Receipt:
`logs/phase28-resume-r4-compiler-hardware-resource-capture-final-r3/receipt.json`,
21,184 bytes, SHA-256
`c0feb33d0098f770b66e2fd05aef5aeba06b3ef524e541a2524ff98bb91d329f`.
Source: 6,769 files / 102,183,904 bytes, SHA-256
`f4a56db95da9609ca51bbe2edf9033cd38e4f8ab6077b33dca035cb24a8a161c`.
These runs overlap; their counts must not be summed. The earlier strict-lint
failure is retained; its new Boolean expression was simplified without suppression.

## Actual non-launching negative

One real CLI invocation used the pinned installed native ROCgDB Python-3.12
binary, not its discovery wrapper. On MI350's unchanged eight-GPU topology,
omitting device selection produced the exact generic native refusal before KFD
admission or target launch. Independently absent HSACO/program paths prevented
a target launch even if device selection unexpectedly advanced.

The command exited zero in 68 ms, with empty stderr and one 486-byte record:
`native_capture / direct_kfd_device_unavailable`. All five inspection-registry
flags were true; they are discovery results, not successful hardware inspections.
Exact output SHA-256:
`1c0cfc1cf252216e8b5a91c8603552521b97a68dc8e7ede3f3002147d26988d6`.

The task supervisor observed its direct child reaped and its owned process group
empty, with no signals or adopted waits. It does not prove escaped-descendant
quiescence or upgrade the native Drop implementation's guarantees. Source-order
checks are not a dynamic syscall trace. This is not gfx950-specific admission
rejection or a successful physical-register capture.

Inner receipt: `phase28-hardware-resource-negative-r1/receipt.json`,
11,499 bytes, SHA-256
`9daf2031aa2d2f8a57df78b65c865ccca921d8c2f6b3e08a72a468125ad435ab`.
Outer receipt, including 11 passing pure qualifier tests:
`logs/phase28-resume-r4-compiler-hardware-resource-negative-r1/receipt.json`,
24,764 bytes, SHA-256
`49331db1dce52af9a5f78accf62f4b1cd059307a3682df1de32465ba9e1750f3`.
All selected inputs, stream hashes and source stability were independently checked.

CLI: 61,372,672 bytes, SHA-256
`a8089d25f5bd5bc835a244d95151170cfe66c01c71bfdd61b2d3c232ac323ef5`.
ROCgDB: 198,045,488 bytes, SHA-256
`17ed42c0993c086a3786869e274eac141ba174e89408654c378ba872e344dd97`.

## Browser validation

The full site gate passed **1,480 unit tests**, lint, type checking, build,
evidence validation, 21 lab tests and **182 desktop/mobile browser tests**.
This includes synthetic positive presentation of scalar/predicate values,
redacted PC, unavailable vector values, paging and stale/reset/no-network checks.
Desktop/mobile screenshots were visually inspected.

Receipt: `logs/phase28-resume-r4-site-historical-hardware-full-r3/receipt.json`,
28,932 bytes, SHA-256
`3a9f49c3c33ec4a6fbfa82c2c7b9d781bdc57e5edec8b3330677ffca5aa9d499`.
Source: 781 files / 19,812,923 bytes, SHA-256
`6662a932a0fea2ccc4914d7f6e6021360acc9dc68c7db780225aed39a4833023`.
The earlier failed test selector and test-only TypeScript option runs are retained.
No production behavior was relaxed to fix them.

The tutorial and exact actual negative example live in
[fe2o3-kernels](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/historical-hardware-resource-capture-v1.md).
Successful hardware cells, source/instruction/memory mappings, live browser
control, physical lifetimes and broader target adapters remain separate work.

## Final integration and actual-file browser checks

Both compiler candidates fast-forwarded to the independently published runtime
metadata fix `7a4ab6dad96e638b638c48615bb758f95b3a76ab`; all 19 authoring/capture
paths were byte-preserved. The combined kernel-IR/model/runtime/protocol/CLI
regression then passed **2,149 tests**, with 26 ignored. Strict all-target
Clippy passed for model/protocol/CLI; this does not erase the separately recorded
kernel-IR baseline strict-lint failures.

Integration receipt:
`logs/phase28-resume-r4-compiler-capture-packing-peer-integration-r4/receipt.json`,
22,665 bytes, SHA-256
`c86ef6378cfb8f06de7947949dc4ec35d21f9b29e0888f60e8f9c94240768037`.
Source: 6,771 files / 102,200,570 bytes, SHA-256
`b36cee47453593d0058174403b095343f15ddf4fe7ad568fe551d6aa577ea154`.
The actual negative above remains the separately pinned earlier CLI run, not
a new physical-hardware qualification of this peer's runtime change.

After adding the exact 486-byte negative example, the site reran all 1,480 unit
tests, lint/types/build, evidence validation and 21 lab tests. All four focused
browser cases passed: synthetic positive and actual retained negative, each on
desktop and mobile. The actual record's digest/bytes, nine exact registry flags,
absence of target/register rows, reset, width bounds and no-network behavior
were checked. No response was mocked and no producer/GPU was invoked by the
browser. This adds two actual-file cases to the preceding 182-case full run;
it is not a claim of a separate full 184-case invocation.

Receipt:
`logs/phase28-resume-r4-site-historical-hardware-actual-negative-r4/receipt.json`,
23,383 bytes, SHA-256
`a4574075752baf954e3b4f0c27e385d30c54d264519939966836db487c1a8c25`.
Source: 783 files / 19,823,128 bytes, SHA-256
`1455b74cd8dd188f0e54b9090cc97150310cf64a274ee303c00f0334aa9808e7`.
These final report additions are later documentation, not part of those source
snapshots.
