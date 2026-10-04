# Bounded repetition and ordinary-source fault replay — 2026-09-22

**Final functional qualification recorded.** The [final publication qualification](#final-publication-qualification) below records fresh builds, regressions, source captures, LLVM observations and fault/replay captures for the final implementation on both forks. Earlier sections retain the initial qualification and failures as historical evidence; they are not relabeled as results for later source bytes. Strict device Clippy did **not** pass: two unchanged ABI-arity warnings remain recorded, and mirror strict Clippy was not run.

M1/V1/U1 remain accepted: **3/18 broad milestones**. These bounded increments do not close M2, V2, U2 or another broad exit. Final documentation/publication censuses, policy results and main readback are recorded separately by the integrator; see the issue publication updates.

All receipt paths below are relative to `logs/` under the retained task root
`/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr` on `mi350`.
They are evidence locations, not portable repository paths or downloadable URLs.
“Supervisor receipt” and “actual driver receipt” name different records.

## Bounded source/API increment

The additive device spelling permits one nonempty initializer and one
literal-count repetition block, expanded at compile time into the existing flat
ordered-program marker. Counts are 1..15; the complete expansion contains at most
16 instructions, at least two in this form. Checked arithmetic, descriptor/role
checks and definite initialization remain mandatory for the entire sequence.
There is no nested repetition, dynamic count, new runtime loop or schedule.

The old flat spelling and terminal 134's five typed const parameters and eight
runtime operands are unchanged. Data operands are evaluated once in order.
The marker does not silently execute on the host. Seven new external API tests
are nested in `tests/ordered_program/repeat_v1.rs`, imported by the existing
`ordered_program_api` target; no automatic Cargo target is added.

Both actual normal-source ladders admitted repeat1, repeat2 and repeat15 plus
a fresh identical-source repeat15. Each retained one MOV followed by N ADDs,
the complete padded descriptor array and fixed bindings [32,33,34,35,36].
The independent whole-buffer oracle is a + N*b modulo 2^32, including output
initializedness and unchanged canaries. The third scalar is a real declared
input but does not contribute to this finite oracle.

## Initial reviewed provider closure refresh

Adding device source changes its entire reviewed closure. The refresh replaces
the canonical and Cargo-vendor pins plus directly dependent known answers and
source-count assertions. It adds no old/new compatibility exception and changes
no source-authentication predicate, terminal mapping, effect/ABI/schema owner or
hash algorithm. The roster is 28 regular src files, with unchanged canonical
Cargo.toml and no build.rs.

- Initial canonical pin: `8931e46d7cd42cec7469af30be82da7cd031d919d8a3dfd617b484ff1fff8d11`.
- Initial Cargo-vendor pin: `8ad658d614a1aca9bc6ee7b3830008a33e692c5664e688bbace2ea1d6bbfec3d`.

These initial pins are historical. The final two accepted pins are listed in the
[final qualification](#final-publication-qualification); no old/new compatibility
entry was added.

The algorithm still hashes its exact domain followed by lexicographically sorted
relative paths and raw file bytes, each length-framed as u64 little endian.
Canonical and vendor manifests remain distinct exact materializations. There is
no runtime normalization, Cargo.toml.orig substitution or semantic-equivalence
exception. Historical capture identities were not rewritten.

The mirror additionally adopts the canonical fork's already-public 1,922-byte
Cargo-produced vendor fixture and Cargo metadata target-roster regression.
Its older 1,770-byte fixture omitted the two existing ordered_program_api and
ordered_region_api stanzas. The adopted manifest SHA-256 is
`a5505445b6b63f1e46b7fca58aa25450de19f3cec1c8d44ce444e64d441a22b4`;
historical producer revision
`c4c5cdd0f69f3844386440a5addb4d4c3dce0e4b` and pinned nightly
2026-04-03 provenance are retained. **No fresh Cargo vendor-producer run is
claimed.** Nested repeat tests add no generated-manifest stanza.

Both full backend regressions passed the actual Cargo target-roster check,
canonical/vendor materialization checks, exact-source authentication and source
closure checks, including manifest/source mutation, missing input, out-of-root,
symlink and nonregular-file refusals. This is the existing reviewed provider
boundary, not transitive compiler-build attestation.

## Device, compiler and regression gates

Every successful supervisor receipt in this table records command-passed,
exit 0 and no recorded signal/transport failure.

| Gate | Supervisor receipt | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| Canonical device R2 | phase22-device-repeat-r2/receipt.json | 23214 | 777a29471ae5c52daf8f179f964cea73d63f73be02a17331a94c2685644e2fc5 |
| Mirror device R1 | phase22-mirror-device-repeat-r1/receipt.json | 23258 | 73bdd68762e774ee49d8bc56ab4eda9adf0da0b8086e736dc71c83d7abe84ef3 |
| Canonical compiler build | phase22-compiler-build-r1/receipt.json | 26605 | 8f2e5880a17d886d05e1c63844557c3b088f8e0780d91f7bde66a85e1eea2cc1 |
| Mirror compiler build | phase22-mirror-build-r1/receipt.json | 27387 | ada81f3a5fdaf0e6552e3c7b66bae2a39949492d863fbc50a0ffd4b0333f70fb |
| Canonical backend regression | phase22w2-compiler-regression-r1/receipt.json | 36682 | 3a5708f517ee14decb6b912822831d1873bc9cc426bb7d402e6c84f6bc9438c6 |
| Mirror backend regression R2 | phase22w2-mirror-compiler-regression-r2/receipt.json | 37079 | b273672ec92835dcda87c06e1e141e7b857c80f897cb417409999d7d1a895ac0 |

Each device gate passed 23 Node source-acceptance control groups, 12 ordered_program
and 2 ordered_region tests in **each** of debug and release, the no_std check, and
22 discovered doctests: 7 ordinary and 15 compile-fail. Those groups had no
failed/ignored cases.

Each compiler build passed 173 Node groups and 164 authoring Rust tests
(134 library plus 6/16/1/7 CLI groups), built the current exporter, extractor,
author, inspector, debugger and simulator, and built the existing
`lower_diagnostic_ordered_program_v17` example and backend test harness.
The 173 Node groups include the 23 repeat-source and 22 LLVM-observation groups;
do not add those again as disjoint tests.

The full backend harness independently passed **1711 tests, 0 failed,
110 ignored** in each fork: canonical 180.88 seconds; mirror 181.50 seconds.
Ignored tests are not passes. These are not whole-workspace strict Clippy results.

The build bases were canonical
`f4f091b77d8a7f4b374878fd436aa6da4c1d7a3a` and mirror
`4ffc0bf0c867c26d5eebe6f041f0be3c6c5d4e75`.
Each listed source census was unchanged across its own gate:

| Gate snapshot | Files | Bytes | Census SHA-256 |
| --- | ---: | ---: | --- |
| Canonical compiler build | 6587 | 100090316 | 04c30a97c39b73a387d6b3db283a0701a4e52478721649f8ddcf084bb68e320f |
| Mirror compiler build | 6580 | 100029191 | 37f4638fd9f030a645e79e562bdf02beb70e302c73bd886a945c53210e984568 |
| Canonical backend regression | 6590 | 100187072 | b939c409003a7ac4154fb6110c7b1ef69dcea2ae28beeeec2d96b0cba8dcb34c |
| Mirror backend regression | 6583 | 100127930 | 977831b9fd88b7151bb5e5dcb74e2c8535f1766d4915bc5303723544495dc987 |

Later reviewed harness/document additions have their own censuses. These
historical gate snapshots are not a claim that final publication already
occurred or that every later documentation byte was part of the compiler build.

## Fresh repeat-source qualification

[scripts/ordered-repeat-source-smoke.mjs](../../scripts/ordered-repeat-source-smoke.mjs)
uses normal source export, inspection and CPU simulation; it does not author KIR
or assume old function ordinals. Each fork's actual receipt reports four
successful exports, four inspections, **120 fresh simulations**, eight exact
frontend refusals, 136 stages and 406 retained file pins.

| Fork | Actual driver receipt | Bytes | SHA-256 | Retained pin bytes |
| --- | --- | ---: | --- | ---: |
| Canonical | phase22-repeat-source-actual-r1/receipt.json | 347492 | 920d20b2f1d3f7654d8d6187a827827ddc580e3395983b3db554bfd8fed24967 | 433130812 |
| Mirror | phase22w2-mirror-repeat-source-actual-r1/receipt.json | 354721 | b2e2208333f21c029306630b14ebc901a59bc21d3c4f84265b11071603c4c5cc | 433146135 |

The 120 simulations are four variants × five scalar input triples × lengths
0/1/65 × two replays. Every full output, initialization mask, unchanged input and
eight-byte output canary is checked against the independent finite oracle.
The reported instruction counts are 2/3/16/16: descriptor MOV=8, each ADD=201,
zero padding to all 16 slots. Scratch/output/input bindings are 32/33/34/35/36;
the declared binding extent/high-water count is 37, not an observed hardware
register allocation.

All eight refusal attempts contain real kernel source and the offending
spelling/resource choice: zero count, 16, oversized usize literal, expanded
17-instruction total, dynamic count, invalid initializer, nested repetition and
physical binding alias. The driver requires the intended frontend failure, not
timeout, missing kernel, unrelated compiler failure or transport failure.

Different repeat counts change body-sensitive preflight/semantic/KIR identities.
Identical repeat15 source preserves those identities within each fork despite
fresh output/target paths. The retained source inventory remains the same
root/function/contract census within each fork; it is **not an opcode digest**.
Canonical inventory is
`10d88d41e0467453ca48a356f89b377bcbd09223c13c2564df46a6fb23b98c09`;
mirror inventory is
`a6a7cca1996a0c8de19fad8186db3bf04a814d4da7f5b31288a26d71edd4d731`.
No cross-fork bundle, source-map or LLVM identity equality is asserted.

These are the existing ordered-program V17 source-export observations, not a
new protected Bundle V6 publication route or production worker admission.

## Separate ordinary LLVM observation

[scripts/ordered-repeat-llvm-observation.mjs](../../scripts/ordered-repeat-llvm-observation.mjs)
consumes the exact successful source receipt with externally supplied
size/SHA pins. Both forks passed its 22 pure controls and four fresh calls to the
unchanged lowerer example. Each actual receipt revalidates 120 retained raw
simulation results and eight retained refusal records, but runs **zero new
simulations**. It retains 425 selected pins.

| Fork | Actual driver receipt | Bytes | SHA-256 | Selected pin bytes |
| --- | --- | ---: | --- | ---: |
| Canonical | phase22w2-repeat-llvm-actual-r1/receipt.json | 204041 | 1e60856f08c8e0b0b4ab11718dcf93994baeff16615a97bc1b896399cbf91b7c | 464864019 |
| Mirror | phase22w2-mirror-repeat-llvm-actual-r1/receipt.json | 207963 | 2cb13500b08b86739c711cf5a5852c93e6bc5fdb75af34781685420540893b8d | 464886571 |

Each call joins actual raw/canonical KIR to the lowerer's complete report and
retained LLVM bytes, including all 16 descriptor slots and five fixed bindings.
Exactly one `asm sideeffect` contains MOV then N ADDs, with constraints
`=&{v33},{v34},{v35},{v36},~{v32}` and direct inputs %arg1/%arg2/%arg3.
The observed result %v11 appears in the ordinary
`store i32 %v11, ptr addrspace(1) %v16, align 4`.
The two repeat15 outputs are byte-identical within each fork.

| Variant | LLVM bytes | Canonical LLVM SHA-256 | Mirror LLVM SHA-256 |
| --- | ---: | --- | --- |
| repeat1 | 1973 | b5b41d4a0f94739019f21eaccbcda41dbcc9e2132eda933940f2bfdb795080da | b0a80708634109e182e6f3b47bdca2b740f2818f44f118d4a4e27514405f2d76 |
| repeat2 | 2003 | 5af2b3378d667fcffde0bb90ad05828c820d018017eca9d75bf45079aba1a61f | 26a278a5b86bb948ffc0643e37d24b0425a8073b6a6104c742421618954172bc |
| repeat15 and fresh repeat15 | 2393 | a9664b8e13d3db3f76bf410659f723d1c397521886157fbdbf86b2e978309e7a | f9681a2eb226e124b0dca8c96b40a19442a85aac6546d979b27884f43e15cfaf |

This is bounded existing-emitter text observation, not a new LLVM verifier,
general output-pointer/dataflow proof, O0/O3 qualification or post-link machine
observation. No existing native observer admits this repeated profile merely
because its instruction count matches. HSACO, native resources, physical
register lifetime and GPU results remain outside this gate.

## Ordinary-source fault and prior checkpoint

[scripts/resource-source-fault-replay-v2-smoke.mjs](../../scripts/resource-source-fault-replay-v2-smoke.mjs)
normally exports unchanged `examples/vecadd` Rust to Bundle V6.
Both corrected R3 captures passed: five recorded stages plus a separate live
debugger protocol session with 44 complete request/response pairs. Each fork
also passed 21 pure control groups. The five stage count is not a claim that
the live debugger is absent or that only five child processes ran.

| Fork | Actual driver receipt | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| Canonical R3 | phase22w2-fault-source-actual-r3/receipt.json | 19683 | c60587e9d5d47d689efecb020133b4d670da2f620780e37644fd9738c6af9488 |
| Mirror R3 | phase22w2-mirror-fault-source-actual-r3/receipt.json | 19928 | be44e6bd0d38c91a5e3fdbfb93af5e7a3887e71465ec286e08045088fb6f4026 |

The actual vecadd workgroup is [256,1,1], with the finite request grid [4,1,1].
Two explicit CPU input models differ only in A's first initializedness bit
(mask ffff to feff); source and data bytes are unchanged. The positive oracle
checks four exact f32 results [1.5,3.5,6.5,11.5], unchanged inputs, full output
initialization and all eight canary bytes. This is a simulator input-model test,
not undefined host-memory access or hardware execution.

The failing standalone run reports typed `execution_uninitialized_read` at
execution stage. A **separate** debugger execution uses the same pinned
bundle/request and retains its own raw JSONL. Diagnostic prose is not converted
into an invented structured fault range. The standalone raw block ID is not
the debugger's canonical block ordinal.

The actual debugger sequence in both forks is:

- Fault/Failed terminal event 22 at revisions 2, 4 and 6; source queries report
  `checkpoint_not_captured`. Terminal values are not fabricated.
- Reverse navigation reaches prior event 21 at revisions 3 and 5. This is the
  before-load checkpoint, not the terminal fault snapshot.
- Each prior checkpoint supplies one frame (frame 1, next operation 2), 13 SSA
  values and complete windows for three global allocations of 16/16/24 bytes.
  Their generations are all 0; no allocation reuse is observed.
- Source variables are queried at **two checkpoints**. Each returns six source
  rows: two captured pointer bindings a/b with source-binding generation 1 and
  four not-represented rows. Source-binding generation is not allocation
  generation or evidence of reuse. The source frame refines the retained
  checkpoint; it does not invent dynamic activation or source-to-SSA links.
- Revisited checkpoint contents agree while revisions/anchors remain distinct.
  Stale-revision queries are rejected. The final continue result is
  Completed/**Failed**, not successful kernel completion.

The normal inspection identifies a load at canonical function 0/block ordinal
6/operation 2, with the retained source span lib.rs bytes 265..325,
line 13/column 5. That exact observed ordinal is evidence, not a hardcoded
selector required for arbitrary future kernels. Prior memory preserves the
uninitialized input mask and the not-yet-written output plus canaries.
The logical debug wave width 32 and active mask 15 are simulator observations,
not a physical GPU wave or hardware register state.

| Retained identity | Canonical | Mirror |
| --- | --- | --- |
| Bundle identity | ad162014861826e1df4d508fcf44a0ca97a73e9d87e02fb542209471d3cd87cf | 051acf38ce5538fd04181b11f462d8acd78db0e465801c3a4500e2a9d598e7c7 |
| Canonical KIR | 43f82fba02952908a7ce39f4b642cc4ce835d0167010f9158e649b97402d9ff8 | a2cc17bd1452ef68ed023b69f9644da970584963a6a7ffabe6ee29afc8c437b2 |
| Source map | 94f6f4e5d53778d1b923e5079c3777fe0179bc733116dc76924164f893dcb7ff | 0c792bfe3bc218bad10ff72caa7796262fa2f1126d9217b9de5fe53d6e008153 |

| Raw retained evidence | Bytes | SHA-256 |
| --- | ---: | --- |
| Canonical requests JSONL | 10184 | 0f9ad2fdd8b084504a022ce859e308d9e6579be03ad12b1eecf01f78a0668e1e |
| Canonical responses JSONL | 69406 | 9c569caee68dd88d42aa12f36c5ea8a011cf5a4d034d8d6640addc423cdd5c22 |
| Mirror requests JSONL | 10184 | 467d136f97c6c494610528b27f634df98557d5a768a8980ae94769e26017d7dc |
| Mirror responses JSONL | 69401 | f9009122c27b76b3d4666bbc084246bf125bcbb3f1ccd657ef191c9010c19075 |

The actual receipts bind these raw files and their derived observations.
Terminal capture/structured-fault and allocation-reuse gaps remain separate
#215/#216 owner handoffs. This slice does not independently close broad V2.

## Failed attempts remain part of the evidence

| Attempt | Supervisor receipt | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| Device R1, missing runtime library search path | phase22-device-repeat-r1/receipt.json | 13815 | 9ed25c1f43a3c06714f82a32490133ce7341df437cb11f9dc21183cd22d1b17f |
| Fault R1, nonsibling output/target paths | phase22w2-fault-source-r1/receipt.json | 19939 | d2b6ec5c074ca2c27e9411f8ccf18fcc241fe321ab1b62e2bb724df9424ef08f |
| Fault R2, wrong requested workgroup | phase22w2-fault-source-r2/receipt.json | 20129 | 6e8a0910e05913a60103d40d8ab6356a786b533c34e0ad069cdd5f4ac99f3c53 |
| Canonical strict device Clippy | phase22w2-device-lint-r1/receipt.json | 13776 | eb97ea9133d8ddebcc22b2e552fc2b65829d64d1fafda80eae0b0b0964f78cb3 |

Device R1 exited 101 before its intended Rust gates: rustc could not locate
libLLVM.so.22.1-rust-1.96.0-nightly. The runtime search path was corrected for
separately named device R2; the failed record was preserved.

Fault R1 rejected the root caller's output/target directories because they were
not exclusive siblings under one supervisor. Fault R2 then exported successfully
but failed positive preflight with `preflight_workgroup_mismatch`: requested
[64,1,1], actual KIR [256,1,1]. Neither failure is an intended uninitialized-read
refusal. R3 corrected the request/control contract to the unchanged kernel's
actual shape; the old source and failed receipts were not relabeled or erased.

Strict device Clippy used -D warnings for the library and existing two external
test targets. It failed only the two pre-existing eight-argument ABI functions
`__amdgpu_ordered_xor_add_e32_v1` and
`__amdgpu_ordered_program_e32_v1` (too_many_arguments, 8/7).
Their unchanged `src/diagnostics.rs` is 6519 bytes, SHA-256
`46a6a137b1e86c4c80379abf2478fb8d801c298e781fd2312ff55334877df2ab`
in both forks. No lint allowance/suppression or ABI edit was introduced.
There is **no global strict-lint pass**; the declared mirror lint gate was unrun.

## Bounds, remaining gates and milestone interpretation

The root's separately recorded Phase22 window-2 continuation followed a network
interruption, with a prospective deadline of 2026-09-23 00:00 UTC and the same
116 GiB task-root storage bound. It did not retroactively change old supervisors
or receipts. The mirror backend gate sampled a maximum task-root size of
123888236197 bytes, below 124554051584 bytes (116 GiB); this is a sampled
observation, not a hard whole-process memory/storage proof or new budget grant.

The repeat driver has a 19-minute cooperative deadline, 136-stage/512-pin
limits, 2 GiB selected-pin aggregate bound and two Cargo jobs; extraction scratch
is separately covered by the root supervisor. The LLVM observer has four calls,
a 270-second cooperative deadline, 64 MiB output limit and selected input pins.
The fault driver has a 300-second deadline, at most 128 protocol pairs,
256 KiB requests, 8 MiB responses, 64 KiB lines and 64 MiB observation output;
compiler scratch remains separate. Existing outer timeouts, stream/resource
guards and input/source readbacks remain required. Sampled guards do not prove
descendant quiescence, hard RSS limits or transitive compiler closure.

The tutorial change is documentation-only: no new UI/importer/browser capability
or fresh browser qualification is claimed by this report. Root has integrated
and reviewed the compiler guides, tutorial pages and cross-links. Documentation
policy checks, final source censuses, publication review and main readback for
harsh-nod/fe2o3, powderluv/fe2o3 and the applicable tutorial repository are still
root-owned publication steps. See the final issue readback on
[#280](https://github.com/harsh-nod/fe2o3/issues/280),
[#281](https://github.com/harsh-nod/fe2o3/issues/281) and
[#282](https://github.com/harsh-nod/fe2o3/issues/282) for published
commit identities and policy/readback results; those are separate from functional
qualification and cannot be encoded as a self-referential document hash.

M2 gains a qualified bounded source authoring/control form. It does not gain a
runtime loop, scheduler or general control-flow implementation. U2 remains tied
to its one supported promoted region and actual fresh analysis/capture
invalidation obligations, not all of M2. Repetition and an ordinary-source
prior-checkpoint observation do not supply protected proof recomputation,
fixed production-policy scheduling, general kernel raising/lowering, physical
ABI/clobber/lifetime guarantees or hardware execution. Source authentication,
compiler-closure attestation, production resume, protected-proof authority,
native/hardware qualification and broad milestone completion remain unclaimed.
The broad count stays **3/18** until the separate exits are actually accepted.

## Final publication qualification

Both forks' final window-4 functional gates **passed** for the final fixture
bytes, reviewed provider pins and integrated tutorial-manifest inputs below.
This section supersedes the earlier provisional reuse plan and identifies the
current qualification; earlier receipts remain unchanged historical evidence.
Publication policy, final documentation census, commit and main readback are
tracked separately, avoiding a self-referential report hash. The accepted broad
milestone count remains **3/18**; no new M2/V2/U2 exit is claimed.

### Final inputs and historical corrections

The staged whitespace gate stopped before publication: eleven new Rust
fixtures had an extra terminal blank line. Removing that extra newline changed
the three positive-source pins, which were updated exactly in the driver,
synthetic source constructor and guide. No old/new compatibility exception,
normalization, oracle change or relaxed frontend predicate was added.

| Final positive fixture | Bytes | SHA-256 |
| --- | ---: | --- |
| source-1.rs | 662 | fa7634a5a1bc841db4b2a8ed5240b0184dfce8c79259fad6e04e60c66f5d08af |
| source-2.rs | 662 | a8bd4ddbb76a6e59871f06ce4b3ee958b7b24b6a7cfd95ca0e804c094e3351f1 |
| source-15.rs | 663 | d1d3f3812f459be5583c377c2a1d1690a85b24abb483555ed53b6150453f55c3 |

The existing hygiene policy then required narrow justification immediately at
two explicit checked-arithmetic panic sites. Two comments explain the unchanged
fail-closed const-source overflow refusals; no runtime fallback, semantic change,
global policy exemption or Clippy-warning suppression was introduced.
Because comments are real bytes in the exact provider closure, the current
reviewed pins are:

- Canonical: `9bb9a616c53766b493970e39f4e294785337c3a038645848f313e5a3038229f5`.
- Cargo-vendor: `539ddb6cf0632c936ea168f7556795b61dac309806177b6c5d502e661b5ba20e`.

The preceding 8931e46d... / 8ad658d6... pins describe pre-comment history, not
additional accepted entries. Root's independent refresh reproduced those old
pins after removing the two exact comments under the unchanged hash algorithm.
The roster remains 28; canonical manifest, historical vendor-producer provenance,
terminal, ABI and semantic owners remain unchanged. Authentication still hashes
raw source bytes, not comment-stripped or normalized text.

The peer main changes were preserved: canonical
`27017f75f27c33a92600f5ca0daaacdd09a4b849`, mirror
`5c86122c09fa9e7d433da75edcf4cd36873512fc`. Their nine tutorial
config/documentation/Python/ci-local paths change no Rust compiler/device/model
source or Cargo manifest. The tutorial manifest is nevertheless embedded by
existing test code. The final **freshly rebuilt** backend harnesses include that
merged manifest, resolving the earlier provisional test-harness reuse caveat.
No old binary is relabeled as rebuilt.

Window-3 canonical composite qualification genuinely passed 194 Node groups,
151 Python tests, shell syntax and four exports/120 simulations/eight refusals
before the later provider-comment change. Its retained supervisor receipt is
`phase22w3-compiler-composite-source-r1/receipt.json`, 39247 bytes,
SHA-256 `fb01d2cce726283cd10cc1514cd7a434ce96b3688e3d50ff3ede46005ea1e278`;
its actual source receipt is
`phase22w3-compiler-repeat-source-actual-r1/receipt.json`, 354958 bytes,
SHA-256 `55f3becec0435ea86aa843b017ee368ec593727578418b96032a1d340a3172b6`.
These are historical passes, not current-provider qualification. Mirror window 3
was unrun. The malformed prospective mirror window-4 source R1 command was caught
before execution; it is an unrun declaration, not a failed gate. Actual mirror
source R2 below is separately named.

All earlier failed attempts remain as recorded above: missing runtime-library
path, invalid output/target layout, wrong requested workgroup and strict device
Clippy. Strict Clippy still fails the two unchanged eight-argument ABI warnings;
the panic-policy comments do not fix or suppress them. Mirror strict Clippy was
not run. Historical receipt/raw-output bytes are retained unchanged, while old
selected-input ledgers are not claimed to match today's changed candidate paths.

### Final builds and regressions

New targets `target-milestones-phase22-final-r1` and
`target-milestones-phase22-final-mirror-r1` contain freshly built matching
normal tools, compiler DSOs, backend harnesses and the unchanged
`lower_diagnostic_ordered_program_v17` example. Each build passed formatting,
**194 Node groups and 164 authoring Rust tests** (134 library + 6/16/1/7 CLI)
using locked/offline dependencies. The 194 groups include 23 repeat-source,
22 LLVM-observation and 21 fault controls; these are not extra kernel executions.

Each subsequent full regression passed **1711 backend tests, 0 failed,
110 ignored**: canonical 180.60 seconds, mirror 180.91 seconds. Each also passed
12 ordered-program + 2 ordered-region tests in both debug and release, no_std,
22 doctests (7 ordinary + 15 compile-fail), 151 Python tests
(73/12/44/10/12) and shell syntax. Ignored cases are not passes, and these gates
are not a whole-workspace strict-Clippy qualification.

Each build and regression preserved its own current source census:

| Fork | Files | Bytes | Census SHA-256 |
| --- | ---: | ---: | --- |
| Canonical | 6594 | 100258052 | 17a64d611a825a79baef17fca79594a0e235a822829c8dcf7b198d194a1ddd10 |
| Mirror | 6587 | 100196927 | f778454a50491b0fa631592e792fc08c7611e80a4a480a93f3ccd6f72867c895 |

These are functional-gate censuses, not the later documentation/publication
census. Root's final artifact pins are distinct from source hashes:

| Artifact | Canonical SHA-256 | Mirror SHA-256 |
| --- | --- | --- |
| Compiler DSO | aeb2d996de934a0cc178c0f0eda669aa2da205501a2bab5d9b4dc4f34b4db4e2 | 3998ecbb957ba5b018a84713385480cd05bd742c8cc546f327fd6e25854180b9 |
| Backend harness | a8b51bb756ef830a15da8c70357f755c683f8bcbae51a7fafba5b6e21f3bdb89 | c2c52cee4c8de29df26ce8f6d14d37f2022caea870639593389440efb6c98dee |

### Current closed receipt set

Paths remain relative to the retained task root's `logs/`. All eight supervisor
records report command-passed; all six actual-driver records report passed.
Supervisor records and actual-driver records are separate evidence, not
additional kernel executions.

| Record | Bytes | SHA-256 |
| --- | ---: | --- |
| phase22w4-compiler-final-build-r1/receipt.json | 26551 | 7909c112d0f78fe06ecfe6c615b8fea75092683cc05b3e502f52904028f54648 |
| phase22w4-mirror-final-build-r1/receipt.json | 26748 | 0692131afde1dcc356ff78afc976cc175c3134cb832cef26134bb47088dc74f3 |
| phase22w4-compiler-final-regression-r1/receipt.json | 39667 | 24485e0509733d2734613bc58db29c019b947b29eb849fe4f1890e7a501f1425 |
| phase22w4-mirror-final-regression-r1/receipt.json | 39991 | 6b2db6b785a4d8544b11e80096a70779be52edc67f9996549250c4fd8c3a7f1a |
| phase22w4-compiler-final-source-r1/receipt.json | 36032 | b073469a4ee3eafaf41db60d00ec1458a0293c205b1a02271a56db8b2a1b5ab4 |
| phase22w4-mirror-final-source-r2/receipt.json | 36361 | 0134a1c9ec76729f18fa28c336ac6f5af56862ba99181a0415691720ed029786 |
| phase22w4-compiler-repeat-source-actual-r1/receipt.json | 355805 | 9edfa2fddcbf220191d6fb18feac00d3cfc6107dd8444aa8397d0a7b92265d07 |
| phase22w4-mirror-repeat-source-actual-r2/receipt.json | 355569 | c5780c3a28c6d84fe0992c39c14c5d61db5b907a532b378c8ddaebfe0deb3880 |
| phase22w4-compiler-final-llvm-fault-r1/receipt.json | 36496 | 2cd1aa5ad1a7f30aab729baf61f9df6321895b580aad87aaaf146fcf3d8072cf |
| phase22w4-mirror-final-llvm-fault-r1/receipt.json | 36823 | 7c814d44b5f5644da23f9ef05d46c168806d64d977f2784c588589c2162fe483 |
| phase22w4-compiler-repeat-llvm-actual-r1/receipt.json | 208610 | 8186ab5c539c20953e0b2b30ed32ac31fb769dec4f78d6821a454ff3fd63ccf0 |
| phase22w4-mirror-repeat-llvm-actual-r1/receipt.json | 208029 | f666a968f4e5c95c94f8a199233dd2c4450a81736c4146c8b59d60e46ee4c7d9 |
| phase22w4-compiler-fault-source-actual-r1/receipt.json | 19855 | 3879d619ad47c9e10cbc77f7aa4b16f1a61533e87b84e37a2a31b9075a239749 |
| phase22w4-mirror-fault-source-actual-r1/receipt.json | 19982 | e6dd69c8127b8b78d53a467bb13541fdf98c8a792f613ce8605eaa18983fd660 |

### Actual final source, LLVM and fault observations

Each final source capture contains four exports and inspections, **120 fresh
whole-buffer CPU simulations**, eight exact frontend refusals and 136 stages.
Each retains 406 pins: canonical 433140798 selected bytes, mirror 433146346.
The complete finite a + N*b wrapping oracle, initialization/canaries, one MOV
followed by N ADDs, fixed declared roles, original bytes and identical-source
repeat checks remain unchanged. New source/semantic/preflight/KIR identities
are actual observations, not copied from prior receipts. Inventory remains a
root/contract census, not an opcode digest.

Each final LLVM capture consumes only its matching **new** source receipt.
It makes four fresh lowerer calls, rechecks 120 retained CPU results and eight
refusals, and runs **zero new simulations**. Each retains 425 pins:
canonical 464882318 selected bytes, mirror 464887630.
Complete descriptors, bindings, one side-effecting MOV/ADD asm unit,
constraints/direct inputs/result use and within-capture repeat15 byte equality
are checked. Earlier LLVM hashes remain historical; no cross-epoch or cross-fork
equality is inferred. This is ordinary LLVM-text observation, not native
O0/O3, HSACO, physical register lifetime or GPU qualification.

Each final fault capture records five stages plus its separate live debugger
session, **44 raw protocol pairs**, four independently checked positive output
words, eight canary bytes and unchanged input bytes/initializedness. Each
queries source variables at two prior checkpoints, not “two source variables.”
The typed standalone uninitialized-read failure and debugger replay are separate
executions of the same pinned bundle/request. Terminal values and allocation
reuse remain explicitly unobserved; earlier checkpoint values are never renamed
as terminal fault snapshots. Raw transcripts are retained without normalization:

| Final raw transcript | Bytes | SHA-256 |
| --- | ---: | --- |
| Canonical debug-requests.jsonl | 10184 | a57ca819e6f00c499047636cbd61b75f53ead4c5cb421de55798203f7005c05e |
| Canonical debug-responses.jsonl | 69396 | 211a78c90b47fd7d6e1542cc89dc4e97b708a07f878211b352e8203880b9b142 |
| Mirror debug-requests.jsonl | 10184 | 0ba596a437faa9aed84103b90162763426265824032cbb2a255baf98f63dca6d |
| Mirror debug-responses.jsonl | 69401 | 6a71e4fd293a7d8d459fa2cb688d0b6476330cb3a862e017aa22a630b4072745 |

The current configuration identities are canonical
`0ebdef9342e325d2a3c846b341750f85b9c190563fefa5b3c6ae31495bad8fa8`
and mirror
`fb19f3cc935e963d47bef9799c6447f7a9e53858635c5aeb0a6ba9afb5ba792a`.
The receipts bind their own bundle, canonical KIR, source map, observations and
raw files; no historical fault identity is substituted.

### Scope and publication boundary

Window 2 recorded 116 GiB; window 3 prospectively allowed 124 GiB through
2026-09-23 00:00 UTC; window 4 prospectively allowed 140 GiB through
2026-09-23 00:30 UTC with the other guards retained. Window-4 notices are on
[#271](https://github.com/harsh-nod/fe2o3/issues/271#issuecomment-5785991657),
[#272](https://github.com/harsh-nod/fe2o3/issues/272#issuecomment-5785991939) and
[#280](https://github.com/harsh-nod/fe2o3/issues/280#issuecomment-5785992170).
These scopes do not retroactively change earlier supervisors or receipts.
New builds and captures use exclusive target/output leaves. Existing finite
stage/pin/stream/output/deadline bounds and sampled-resource limitations remain;
this document grants no new resource authority or compiler-closure attestation.

All final functional gates above are closed. Final documentation-policy checks,
commit identities and main readback are separately recorded publication work,
not missing functional qualification or a circular prerequisite for this text.
The tutorials remain documentation-only, with no new UI/browser result.
Source/proof authentication, protected production resume, runtime scheduling,
native/hardware execution, physical lifetime guarantees and broad milestone
completion remain unclaimed; **M1/V1/U1 remain the accepted 3/18 exits**.
