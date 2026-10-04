# Checked gfx950 artifact observations — 2026-09-23

This qualifies a **read-only device/artifact companion**, not stopped-wave
capture, kernel execution, source admission or protected publication. M4/V4
advance; accepted original exits remain M1/V1/V2/U1/U2/U3 (6/18).

## Implemented boundary

The Linux RocgdbCheckedGfx950ArtifactV1 API retains a genuine mutable checked
gfx950 device, immutable artifact bytes/name and an explicit caller load base.
Inspection and revalidation fence the existing full device-currentness check
before and after bounded descriptor inspection. Revalidation compares complete
content identity and exact selected entry/load-base binding. Its copied
artifact identity is inert, including after later revalidation failure.

The profile is exactly gfx950:xnack-/Wave64, without an explicit SRAM-ECC claim.
Existing inspector limits remain 64 MiB artifact / 4,096-byte metadata name.
No raw-device constructor, Clone, deserialization, public correlation gateway,
capture slot, protocol DTO, queue or telemetry producer is added. Stopped-wave
support still needs genuine runtime metadata and compatible trap handling.

The additive observe_gfx950_checked_artifact_v1 example uses canonical bounded
arguments, no-follow regular-file snapshots, exact supplied hash/size, explicit
node and unique-ID selection, two revalidations, bounded file reread and a final
device fence. It reports closed refusal phases, not raw driver errors.
Its load base is **not an observed loaded address**.

## Tests and lint

Commands ran on mi350 from base c859548ab9487e824b644ed2b9aa92cab86ae46c,
using the bounded task runner, offline/locked Cargo and pinned nightly.
Source/tool inputs were unchanged before and after each passing run.

- Debugger CLI/protocol tests and doctests: **392 passed, 0 failed, 3 ignored**,
  including five new synthetic artifact tests and two compile-fail ownership
  examples. Strict all-target no-deps Clippy passed.
- The separate example gate initially passed 11/12. Same-byte inode replacement
  correctly refused, but forwarded an initial file-admission phase when the old
  retained fd's link count became zero.
- The targeted fix maps retained fd/path stamp failures to retained_currentness.
  It preserves the replacement assertion. The rerun passed **12/12**, built the
  ordinary example and passed strict all-target CLI Clippy. These are separate
  gates, not a second full suite.

The companion gate census was 6,784 files / 102,312,990 bytes:
9bb22ade7f7566a36c8dce202e337205181aacfaf81cee8da30924ee36b5bd1e.
The corrected example and actual-device census was 6,788 files / 102,334,400 bytes:
de0901408b46df96f602fb154d3f2b366f19c486cb14dfca18707090e25c0926.
Byte-identical C++/CMake fixture sources and expanded public documentation
were added afterward; these are not eventual commit hashes.

## Native artifact and actual checked device

A fixed diagnostic LLVM module was compiled through the existing pinned
LLVM/LLD worker and reviewed ROCm 7.2.1 SDK. It was not admitted Rust source.
Request custody fields are explicitly synthetic, as in existing mechanism
fixtures. The [public fixture](../../tools/fe2o3-llvm-link-worker/tests/gfx950-artifact)
has C++/CMake sources matching the tested private bytes. All measured existing worker source
files also matched this checkout. SDK readback checked 2,517 files /
2,279,434,376 bytes before and after build. This is not transitive attestation.

The unchanged worker inspected gfx950:xnack-/COV6/Wave64, one selected
fe2o3_gfx950_observation_fixture kernel, kernarg264/align8, required group
[64,1,1], and zero group/private storage.

| Observed object | Bytes | SHA-256 |
| --- | ---: | --- |
| Fixed generated LLVM | 827 | 77aa8ca2316bcb5b329b926a29f4c61acc1206fbfd717920ac8476bd8ffd157e |
| Whole native HSACO | 5536 | d10b592732d91cf4c0d4289890fdd0a5328e84cb8208817eaabac4c62c1a34c6 |
| Rust observer executable | 26920176 | 991f6fec4d7ddc6466abeb7a7875615ff713e60257aca353d8ceb959e9bc6310 |
| Native fixture executable | 106477768 | 7adafdd8f81a0568bdcd21006a7160f00368ef0db770e8368b97eb51ca5c8e91 |

Recomputed worker derivation identity:
13164252a8472cbf8dcaebc62897e82ffca5fa5c83c8b46d2eed397800c0f9a4.

The Rust example independently inspected those bytes against actual topology
node 2 / GPU ID 39903 / unique ID 16366993098680759275. Device-profile digest:
6f859b0a67f8ee2497393206930ff35bf1a9ae69d33f172106a790a0c9226667.
Two revalidations, retained file comparison and final currentness succeeded.

| Case | Expected and observed result |
| --- | --- |
| Exact artifact/device/kernel; caller base 65536 | Observed; two revalidations |
| Changed hash | artifact_pin / sha256_mismatch |
| Changed byte count | artifact_file / expected_size_mismatch |
| Wrong node with same selected unique ID | device_selection / node_or_unique_id_mismatch |
| Missing kernel | companion_inspection / kernel_selection |
| Overflowing caller base | companion_inspection / code_binding |
| Separately pinned real gfx942 artifact | companion_inspection / artifact_target |

All six refusals exited 1 without a success observation; the positive exited 0.
An earlier-phase failure was never counted as a later negative. The gfx942
negative used historical ordered_repeat_u32 bytes: 6,224 bytes,
e9c97e7715d7ae0f9bd5a795fa099025199b7ab77619267100fa640cb4eee6c9.

Binding queries identity/XNACK/apertures and subscribes the existing reset-event
descriptor. It does not set XNACK, explicitly acquire a VM, allocate GPU memory,
create a queue, dispatch, attach or read registers. No reset mutation occurred.
This proves neither continuous currentness nor all-reset/ABA resistance.
Normal descriptor closure is not a measured fd census.

## Retained evidence

Receipt paths below are under the task root's logs/phase28-resume-r5- prefix.

| Receipt suffix | Bytes | SHA-256 |
| --- | ---: | --- |
| compiler-gfx950-companion-r1/receipt.json | 21532 | aae6a947fa9788b0dd01896c50da11ee99b706dafafc3cf4863b339f2d94d019 |
| compiler-gfx950-companion-example-r1/receipt.json (failed) | 12322 | ae0299cdd5b525876d90884dc7ebc87f831b7f3a8dc3c07c2b1c5761ad1efa73 |
| compiler-gfx950-companion-example-r2/receipt.json | 20681 | 079c3e4dc72b7e4d6dea21a6f8121da90fb9adc16670d5c6bfcfe649ceec7f8b |
| compiler-gfx950-native-fixture-build-r1/receipt.json | 96821 | 24b9e57c077d69b479f2a02277fadd45e09c3f9acc3b4daa086fe9375d9715e6 |
| compiler-gfx950-native-artifact-r1/receipt.json | 27862 | dcfba4e4d39dcd6c09a33e8649d255eeb2e7155f4e1861a92e27d117115193e4 |
| compiler-gfx950-companion-actual-r1/receipt.json | 28086 | 8077336cbbb2c41996f587eb19826e69a528fd2d2e8d0378103ae236524c3f36 |

Actual-device stdout: 1,716 bytes,
108026806db46689aa5415422bc1e1935271b821323d04ab7af63ec4cbd0fcc9.
Independent readers reviewed companion/currentness/binding/pins and the
native fixture's worker/result relations; reviews do not replace execution.
