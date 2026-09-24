# Complete-body renderer and native ABI — 2026-09-23

This qualifies an inert renderer and static LLVM/native mechanism, not source
admission, execution, protected publication or M2 completion. Accepted original
exits remain M1/V1/V2/U1/U2/U3 (6/18).

## Implemented mechanism

The checked gfx942 body model now renders bounded LLVM text and correspondence.
The model admits at most eight forward blocks and sixteen typed integer steps,
with explicit scratch/output/three-input VGPR intent, all-path initialization
and exact resource declarations. Renderer symbols use a closed ASCII grammar
with a 128-byte limit. Logical assembly/LLVM limits are 4 KiB/16 KiB; an explicit
work budget is charged before rendering. Allocator rounding is not an RSS bound.

LLVM still owns kernel ABI and index setup. The authored body is one
side-effecting inline-assembly unit with exact physical constraints and clobbers.
Uniform selector input is s22. Ordinal labels use LLVM unique-instance labels,
not caller-provided PCs. All terminal paths join the compiler-owned guarded
store/carry/EXEC-mask/wait/restore/end sequence.

The renderer returns inert text, not a source, canonical, launch, proof or
artifact owner. Nothing here bypasses the normal compiler transaction.
The standalone diagnostic example is not the complete-body source frontend.

## Executed gates

All commands ran on mi350, based on compiler main
8e3d0b8140b0541ee4deae44e70dd654a1f42b4d, with selected source/tool inputs
checked before and after by the bounded runner.

- Final model tests/doctests: 342 passed, 21 ignored; the separate ordinary
  generator example adds two passing tests. Combined gate: 344 passed,
  zero failed, 21 ignored. Strict all-target no-deps Clippy passed.
- Five actual generator calls produced LLVM for independent closed profiles.
- Ten real LLVM/LLD compilations passed: every profile at O0 and O3.
- The final matrix passed 352 negative checks. No GPU was executed.

The native checker uses the unchanged worker's decoder and independently
transcribed plans. It checks exact typed LLVM dataflow/constraints, complete
entry bytes and CFG, direct branch destinations, every authored step, the
compiler tail, actual metadata slots, descriptor capacities and ready scalar
load provenance from kernarg byte 28 to s22. Root separately rejoined all
365 exported instruction byte rows to the retained payload extents.

| Profile | Blocks / authored steps | O0 / O3 entry instructions | O0 / O3 HSACO bytes |
| --- | --- | --- | --- |
| one | 1 / 1 | 36 / 21 | 5808 / 5488 |
| output_diamond | 4 / 2 | 42 / 27 | 5872 / 5488 |
| scratch_diamond | 4 / 3 | 43 / 28 | 5872 / 5488 |
| two_terminals | 3 / 2 | 41 / 26 | 5872 / 5488 |
| maximum | 8 / 16 | 58 / 43 | 5936 / 5552 |

All cases observed six explicit argument slots: pointer 0/8 bytes, length 8/8,
a 16/4, b 20/4, c 24/4 and selector 28/4; kernarg total 288, alignment 8,
gfx942:xnack-/COV6/Wave64 and required group [64,1,1].
Metadata reported SGPRs 29 at O0 and 30 at O3; VGPRs 37 except maximum's 64.
All ten reported zero LDS/private bytes, AGPRs and spills. These are static
resource observations, not live occupancy or physical lifetime measurements.

| Refusal boundary | Checks |
| --- | ---: |
| LLVM parser or exact typed profile | 208 |
| Existing machine decoder | 48 |
| Complete-body native relation | 26 |
| Explicit/hidden metadata relation | 30 |
| Descriptor capacity relation | 20 |
| Selector native provenance | 20 |

Native controls mutate actual bytes, recompute the hash and rerun the decoder.
They include invalid/wrong branches, arithmetic register changes, wait drift,
s23 substitution, insufficient descriptor capacities, selector load-offset
changes and overwriting s22. Metadata controls independently reparse changed
payloads. Earlier decoder refusals are not counted as later relation refusals.

## Failures retained and corrected

The first native build failed strict signedness checking in the fixture's
capacity comparison. Its required-capacity expression now uses uint32_t and an
unsigned increment; warnings remain errors.

The first actual matrix passed one/O0, then refused one/O3's metadata.
The checker had incorrectly required the same nineteen hidden descriptors at
both optimization levels. A separately retained O3 artifact showed the same six
explicit slots and reserved 256-byte hidden extent, but only the first thirteen
hidden descriptors through hidden_grid_dims; six runtime-service entries were
absent. The checker now requires the exact optimization-specific roster, not an
arbitrary subset. All explicit-slot, extent, descriptor and selector checks
remain mandatory. The hidden-entry mutation uses an entry present in each
profile. Failure reproduction remains a failed gate, not additional success.

The fixture now retains exact emitted bytes before relation checks. A file can
therefore exist after refusal: only a successful final report qualifies a case.

## Exact artifact identities

| Profile / level | Whole HSACO SHA-256 |
| --- | --- |
| one / O0 | 7354cd2b168e6d5a3edc39c790f1befd0581d2d38a65d7b97a3ad1578b7e2c13 |
| one / O3 | fc3d5781e66a14b3183792290c65862d4be3419e1b59e29bc9fa690be496856d |
| output_diamond / O0 | 7d46d7772fa202289132f2980398f041978dcdbae5a1ceab1a4a2d7d2dc1dda7 |
| output_diamond / O3 | d32f56f910b3bafbb59317fca812eb8e7862962130f5c38e5e884fb90f8ad43f |
| scratch_diamond / O0 | 32d81cda07db42418811adaef03d5de5ad96111824065bd5b14f28b46d43a06c |
| scratch_diamond / O3 | 214c6a419d3b705af5c4ef270a459af9a1a2dca39c023ca03dbd1d2eec9cceeb |
| two_terminals / O0 | f34e7a9c85100757e0f750cf61aa3be2d56dd65ad7dd1677a6fef574ffda9d6e |
| two_terminals / O3 | b75dad34de81b20cf9fcb5047f6559233e8bd68469ef827efc48e5ca3e1b4c29 |
| maximum / O0 | 38185d185b37ec6a5c0d8a5a7931c0ce072f75481cfd962d2f54c27087e21df3 |
| maximum / O3 | 5624d9e2ffcb6e5041746c65b53b45107bb5572cfd0f31aed53e5798152560fe |

Final native fixture executable: 107,766,128 bytes,
4fe0c4ecbdeb73abb689487604a5e9a54da0c5f5e6fc3fb324e5f48cf33aafc1.
Generator executable: 21,008,080 bytes,
c37afabf2ad26d679b55f26e0312e94f65bd1ccae1985f685194c9f56ef89013.
Actual-case reports range from 29,685 to 62,330 bytes, below the separate
64 KiB per-report cap. Aggregate totals do not enlarge that cap.

The existing reviewed ROCm 7.2.1 SDK and actual worker were reused.
SDK readback checked 2,517 files / 2,279,434,376 bytes around builds.
The passing native/model source census was 6,801 files / 102,430,279 bytes,
945e35d2bfde4edb416333a66a859e4bab6ff146c2d780a4d79757d357bad618.
Later publication documentation is outside that census; it is not a commit hash.

## Retained receipts

Paths are under the task root's logs/phase28-resume-r5- prefix.

| Suffix | Bytes | SHA-256 |
| --- | ---: | --- |
| compiler-complete-body-native-build-r1/receipt.json (failed) | 55630 | 7a8232b177c38556f540c567769bc299af3469e776a98e975126bb5e705f4988 |
| compiler-complete-body-native-actual-r1/receipt.json (failed) | 56015 | 67d8641ef18f02983c01a5502b50462a4d6ab5ffb620671b357902100236eab8 |
| compiler-complete-body-native-diagnostic-r1/receipt.json (failed reproduction) | 55753 | 187863336fe5e5da6d35092967b1d32c237ad663d3bf99daaa25a8859e8ee4df |
| compiler-complete-body-native-build-r4/receipt.json | 91037 | 80afaf925854d6250e03bb237de9b34d443011d523f0305e27887739c50d8e3a |
| compiler-complete-body-native-actual-r2/receipt.json | 94551 | 721e82012a633ade7ab2c24069a50f91b24b2d945eb2a5c7a316ec173e46b35e |
| compiler-complete-body-renderer-final-r1/receipt.json | 20663 | fc22840a6859e9d04ff56d0e61a0ca9926b89557d1ce2e4d7e50080eb98ebe10 |

Passing native stdout: 6,581 bytes,
0f49a01fb1f5a4107b4044b62d54e22a0855842b42d0a35e3d53cc0947d2b7f8;
stderr empty. Earlier intermediate build receipts remain retained separately.

## Still required

Authenticated whole-body source collection, coordinated semantic/canonical
registration, real SSA/source-launch correspondence, ordinary checked lowering
and source re-entry must compose before M2 can close. Memory/synchronization
authoring, native functional reference checks, hardware qualification and
protected finalization remain their separate milestones. This fixture uses
explicitly synthetic worker custody fields and cannot mint their authority.

See the [fixture](../../tools/fe2o3-llvm-link-worker/tests/complete-body-abi)
and [contributor walkthrough](https://github.com/harsh-nod/fe2o3-kernels/blob/main/docs/complete-body-native-abi-v1.md).
