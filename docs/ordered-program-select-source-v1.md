# Choose a bounded instruction program at compile time

The experimental const_if spelling lets a Rust kernel select between two
complete flat instruction lists using a concrete boolean constant. Both lists
must be valid, including the one not selected. Selection produces the existing
single ordered-program marker; it adds no GPU branch or runtime predicate.

Qualification status: the revised hygienic implementation passed fresh
normal-source and CPU acceptance independently in both compiler forks:
four exports, four inspections, 120 simulations and eight exact refusals per
fork. See the [dated source evidence](evidence/authoring-select-source-20260923.md)
for source/tool/receipt pins, regression results and retained failed attempts.
This is not LLVM, native or hardware qualification for selection. The existing
[literal-repeat acceptance](ordered-repeat-source-acceptance-v1.md) and
[repeat-native observations](ordered-repeat-native-observation-v1.md) concern
different source/kernel inputs and cannot qualify this feature.

Implementation and acceptance inputs:

- [Selection helper](../crates/fe2o3-device/src/ordered_program_select_v1.rs)
  and [macro](../crates/fe2o3-device/src/ordered_program.rs).
- [Normal-source driver](../scripts/ordered-program-select-source-smoke.mjs)
  and [pure controls](../scripts/tests/ordered-program-select-source-smoke.test.mjs).
- [Complete registered source fixtures](../crates/fe2o3-device/tests/fixtures/ordered-select-v1/).

## Author two alternatives

This is the body of the reviewed const-true fixture, with its qualification
comment omitted for readability. It is a library body, not a standalone
Cargo project or an independently executed Markdown example.

~~~rust
#![no_std]
use fe2o3_device::{DisjointSlice, amdgpu_ordered_program, kernel, thread};

const SELECT: bool = 3_u32 < 4;

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn ordered_select_u32(
    mut output: DisjointSlice<u32>,
    a: u32,
    b: u32,
    c: u32,
) {
    let result = amdgpu_ordered_program! {
        gfx942_xnack_off_wave64;
        scratch(32); out(33); in(34) = a; in(35) = b; in(36) = c;
        const_if(SELECT) {
            mov(out, input0);
            add(out, out, input1);
        } else {
            mov(out, input0);
            add(out, out, input1);
            add(out, out, input1);
        }
    };
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = result;
    }
}
~~~

With SELECT true, the selected program is MOV followed by one ADD and computes
(a + b) modulo 2^32. Changing the constant to 4_u32 < 3 selects MOV followed by
two ADDs and computes (a + 2*b) modulo 2^32. For a=19 and b=23 the independent
expectations are 42 and 65. These numbers are not captured simulator results.

The three data expressions a, b and c evaluate once, in argument order, before
the one marker call. Selecting the false arm does not evaluate b twice.
Input2 and scratch remain declared roles even though this example does not use
their values. Calling the marker as an ordinary host function still panics;
there is no host execution fallback.

Use a bool literal, a named concrete bool const or a concrete Rust bool
constant expression. A runtime scalar such as a != 0 is not a predicate input
to this unit. Non-bool values and references to an outer generic parameter are
unsupported; this does not enable general const-generic kernel specialization
or change rustc's const-expression rules.

## Both arms are checked independently

Each arm is a complete, nonnested flat program containing 1 through 16
instructions. The total across both alternatives may reach 32, but the selected
program never exceeds 16. There is no shared init block, repeat block, nested
const_if, authored helper body, label or runtime branch.

Both arms start with only input0, input1 and input2 defined. Scratch and out
start undefined separately in each arm. Reads use the pre-instruction state,
only scratch/out are writable, and out must be defined on exit. An out
definition in the true arm cannot initialize a read in the false arm.

This is intentionally invalid even though the true arm is chosen:

~~~rust
const_if(true) {
    mov(out, input0);
} else {
    add(out, out, input1); // out has no earlier definition in this arm
}
~~~

This is an explanatory fragment, not another executed registered fixture.
Empty, malformed, unknown-opcode, oversized or undefined inactive arms are
not discarded before checking. The existing packer checks the true arm and
then the false arm before selecting count/packed words. On an invalid arm it
may refuse before reaching later checks; a valid selected arm never exempts
the other arm.

Existing opcode/arity/role checks remain MOV, wrapping ADD/SUB, AND/OR/XOR
over u32. Five physical bindings must be distinct literals in v0..v63.
The normal source owner still requires one unconditional, acyclic occurrence
in one direct kernel root, gfx942:xnack-, wave64 and required/maximum workgroup
64x1x1. This adds no authored memory/synchronization, SGPR/AGPR, EXEC writes,
carry, matrix operations, gfx950, partial waves or arbitrary control flow.

## Hygienic expansion and bounded local work

The revised selection arm introduces no named const items or local bindings
into the caller's scope. Each of the five const-generic arguments directly
calls the eager helper with the predicate and both descriptor arrays, then
projects the selected count or one packed word. Caller identifiers must not
accidentally bind to implementation item names.

Each helper call delegates to the old flat packer twice. Each arm's length is
checked before narrowing it to u8 and before its descriptor loop. A valid call
checks at most 32 descriptors and has two 32-byte packed results. Across the
five syntactic projections, that is at most 160 descriptor checks; rustc may
reuse evaluations. This is not a bound on predicate evaluation, macro tokens,
all rustc CTFE, stack layout, allocation, process RSS or total compilation I/O.
Keep normal compilation under independently budgeted supervision.

There is exactly one call to the existing terminal134 marker, with five typed
constants: one u8 count and four u64 packed source-descriptor words. Runtime
arguments remain three u32 data operands followed by five u8 role literals.
These words are not AMD machine instructions. The old flat/repeat macro arms,
terminal declaration and executable schemas are unchanged.

## Where this enters the compiler

~~~text
Rust + concrete const_if
  -> both flat arms checked; one count/four-word program selected
  -> existing actual terminal134/source-occurrence checks
  -> semantic MIR V32 / diagnostic canonical KIR V17
  -> existing ordered LLVM inline-assembly lowering
  -> separately qualified native compilation, if requested
~~~

Surrounding typed Rust, output indexing and the store still lower normally.
LLVM IR is not bypassed: the selected list becomes one constrained ordered
inline-assembly unit in the existing path, not separate LLVM branches for the
two alternatives. This describes the representation contract, not a fresh
LLVM observation for these inputs.

The frontend independently validates the actual marker instance, typed
constants, source placement, registers, target and launch. The macro helper
is not the sole validation layer. The driver does not construct executable
KIR, synthesize a replacement owner, edit canonical bytes or resume protected
production compilation.

The inspector reports declared roles/order. Their high-water 37 is not final
kernel VGPR usage, descriptor capacity, occupancy or a lifetime proof.
Existing debugger treatment remains one logical ordered-region operation:
this syntax supplies no newly captured per-instruction values, physical
registers, source-to-SSA links, selection microsteps or debugger UI.

## Exact source fixtures and identity rules

All eleven leaves below are complete registered kernels named
ordered_select_u32 in the directory linked above. They are unchanged between
the first and revised macro drafts; their original qualification comment is
retained in the bytes. Do not regenerate them from the displayed snippet or
change them to accommodate a failed capture.

| Fixture | Bytes | SHA-256 |
| --- | ---: | --- |
| source-literal-true.rs | 799 | `cccdb4076ed4404b315ea938d1f9c0feeb297414f36e5c4003f36c1ff852ebd6` |
| source-const-true.rs | 834 | `2a04e96a8f4b6f8e52ef6319545ae6f56b1ae3b985de9a2e2f77a9b605717f1a` |
| source-const-false.rs | 834 | `d3d882ac1669b114701f693451dab5b81182b360c2dee8c199a474eb58b03df6` |
| refuse-dynamic.rs | 801 | `b0207d3860a01fce94fcfd428513ff2eb96812df8deeb9c5bd951412038288aa` |
| refuse-non-bool.rs | 800 | `5bd209d21a2ba3464a656940606904767effe5f8bb253f8ce1e45c5fd030784b` |
| refuse-nested.rs | 832 | `01e8fdba11b2746572713e9950eaeaacffe7d2ae00e82e336e0cf51fd76b25f3` |
| refuse-missing-else.rs | 682 | `593cbfb364b58bf598b916efaa022f71baf294000160e0474f02d318cd06e29b` |
| refuse-unknown-opcode.rs | 764 | `ef2a3123ad66714b4d6492fae1a652cfbf8ebaeb213410c476848e68ec0796f8` |
| refuse-inactive-undefined.rs | 734 | `e40704524be650eaa15778b3c7f68c4a0c9688c08d527f3b33f0379c06e6102d` |
| refuse-inactive-seventeen.rs | 1289 | `31af8d48243315a0f10359e24fce04cbdb85639ae2e1aa4e0630ba7eba6401d6` |
| refuse-register-alias.rs | 799 | `6ecc77863c102959579fe3e8d709e5334aafc4abc7a701e5fe52dcb0330c9426` |

The driver copies these full sources into fresh task-owned library packages
using the existing assembly-authoring-v30 manifest/lock template. It changes
only the two template dependency paths to the chosen repository and retains
the lockfile bytes. Negative source exports use these attributed kernels,
not host-only snippets or hand-authored KIR.

The four positive exports are:

| Label | Predicate | Selected list | Count |
| --- | --- | --- | ---: |
| literal-true | true | MOV, ADD | 2 |
| const-true | SELECT = 3_u32 < 4 | MOV, ADD | 2 |
| const-false | SELECT = 4_u32 < 3 | MOV, ADD, ADD | 3 |
| repeat | exact const-false source again | MOV, ADD, ADD | 3 |

The final export uses identical const-false source/manifest paths with a new
extraction target/output. All four have the same role plan. Inspection must
expose descriptors 8/201 or 8/201/201, zero-padded to sixteen entries, and the
corresponding ordered instructions. These descriptors are not native encodings.

Keep raw source/file SHA-256 separate from domain-separated semantic,
preflight, inventory and canonical identities. Literal-true and const-true
have different source bytes but the same selected program/arithmetic; no
arbitrary cross-spelling semantic/KIR identity equality or inequality is
assumed. Const-true versus const-false must change the selected program and
observed canonical identity. The identical-source repeat must reproduce the
complete accepted export/inspection identity set, including the inventory.
A retained inventory is a root/contract census, not a body digest.

## Reproduce the qualified normal-source profile

These are command templates, not literal commands with usable paths.
The dated report records the actual qualified inputs and captures.
Replace every placeholder with reviewed matching inputs. Build the device crate/controls and fresh normal exporter, extractor,
inspector, simulator and compiler DSO from the same final source/provider
closure. Do not reuse an old DSO solely because its path exists.

~~~sh
/ABS_NODE /CANONICAL_REPO/scripts/tests/ordered-program-select-source-smoke.test.mjs

/ABS_NODE /CANONICAL_REPO/scripts/ordered-program-select-source-smoke.mjs \
  --repo /CANONICAL_REPO \
  --bin-dir /MATCHING_FRESH_NORMAL_TOOL_DIRECTORY \
  --cargo /EXACT_RUST_TOOLCHAIN/bin/cargo \
  --rustc /EXACT_RUST_TOOLCHAIN/bin/rustc \
  --output /CURRENT_SCOPED_ROOT/logs/NEW_ORDERED_SELECT_OUTPUT
~~~

The script must be the selected repository's own script. All paths are
canonical absolute paths. The output leaf must not exist; its parent must
exist and the output must be outside the selected source/tool inputs. Writes
are create-new and failed attempts remain retained. Do not transplant receipts
to another root or rewrite their paths.

The driver requests raw diagnostic canonical KIR V17, not Bundle V6.
Each ordinary positive export follows:

~~~text
fe2o3-export-sim --diagnostic-kir-v17
  --crate fe2o3_assembly_authoring_v30_fixture
  --output NEW_OUTPUT/<label>.kir --target gfx942
  --target-dir NEW_OUTPUT/<label>-extraction
  -- --manifest-path NEW_OUTPUT/<variant>-source/Cargo.toml --lib --offline
~~~

The exporter forces Cargo --locked and its existing target/build-std profile.
The driver requests offline behavior explicitly and through the environment,
uses jobs2/incremental0 and exact Cargo/rustc/tool loader paths, and removes
ambient extraction flags/wrappers. Selected pins are not a complete compiler
closure or sandbox; Cargo configuration/cache, runtime dependencies and
descendants still need outer supervision.

Normal inspection and simulation consume the real newly exported file:

~~~text
fe2o3-program-inspect NEW_OUTPUT/<label>.kir NEW_OUTPUT/<label>-inspect-request.json
fe2o3-kir-sim --diagnostic-kir-v17 NEW_OUTPUT/<label>.kir --request NEW_REQUEST.json
~~~

Function/block/operation coordinates come from the normal inspector, not
hard-coded ordinals. No debug session, lowerer, native worker, launcher or GPU
is invoked by this source ladder.

## Independent outputs and exact refusal causes

The qualified success ladder has 4 exports, 4 inspections, 120 CPU simulations
and 8 expected frontend refusals: 136 sequential stages per compiler fork. Each positive export gets five scalar triples, lengths 0/1/65 and two
replays. Lengths 0/1 launch one 64-lane workgroup; length 65 launches two.
The independent oracle is:

~~~js
const additions = selectedCondition ? 1n : 2n;
const expected = (BigInt(a) + additions * BigInt(b)) & 0xffffffffn;
~~~

It does not interpret descriptors or reuse simulator arithmetic. Inputs cover
zero/all-ones, wrapping addition, alternating bits, high-bit overflow and
19/23/42. Check every output word, backing byte, initialization/padding bit,
unchanged scalar argument and two four-byte canaries. Even empty output must
preserve all eight canary bytes and every initialization bit. Unknown fields
or fabricated authority must refuse rather than be silently dropped.

Each result must match its exact KIR/request/target and cooperative schedule.
Actual step count is bounded by the existing finite budget, not an invented
exact MIR count. No-race observations remain selected CPU-run observations,
not a universal race proof.

| Refusal fixture suffix | Cause checked by the normal-source acceptance |
| --- | --- |
| dynamic | runtime value is not a concrete bool constant |
| non-bool | predicate must have type bool |
| nested | only two flat instruction lists are admitted |
| missing-else | both alternatives are required |
| unknown-opcode | unselected instruction still needs an admitted opcode/arity |
| inactive-undefined | unselected out read has no prior definition |
| inactive-seventeen | unselected arm exceeds sixteen steps |
| register-alias | normal source owner rejects scratch33/out33 aliasing |

Inactive-arm conditions stay true and their true arm stays valid. Each refusal
must bind the exact generated crate/manifest/source and healthy normal export,
with no resulting KIR or unrelated error. Retain typed Cargo JSON diagnostics,
failed terminal record and exporter status/streams. Quoted text, warnings,
timeouts, ICEs, dependency/loader failures, stale providers and no-kernel errors
are not the requested cause.

The revised macro's host-only diagnostic preview observed exactly one primary
E0435 for dynamic input; five E0308 primaries for non-bool input; five null-code
unknown-opcode primaries; five E0080 primaries for each inactive undefined or
seventeen-step arm; and one syntax primary for each nested/missing-else case.
The preview used plain host functions containing the macro statements, without
kernel registration or the output store. Three positive macro snippets and the
alias snippet host-compiled; these are not complete normal frontend exports.
The alias still requires the existing normal source-owner refusal.

The normal acceptance must retain precisely the applicable observed primary
count and validate every primary's code/message, target, manifest/source and
primary-span presence. It cannot accept arbitrary additional errors or use a stale
one-error assumption for all five projections. Both fresh normal-source captures passed
that exact diagnostic contract; the dated report retains their own evidence. Do not weaken validation to
“any failure” or silently rewrite fixtures.
Generic-dependent predicates remain a separate Rust compile-fail control,
not an invented registered generic-kernel route.

## Resource limits and evidence boundaries

The driver retains the existing bounded process helpers:

- At most 136 sequential stages; Cargo jobs2; export timeout 300s and query
  timeout 60s; cooperative deadline 19min with a separate hard outer deadline
  and stop/retention margin.
- Source/raw KIR at most 64KiB each; parsed JSON, each stdout/stderr and receipt
  at most 1MiB; at most 256 Cargo JSON records per negative export.
- At most 512 selected pins, each at most 512MiB and aggregate unique selected
  bytes at most 2GiB. Repeated executable rehashing is separate I/O.
- Existing 40GiB disk-free and 64GiB available-RAM reserve checks; these do not
  bound the entire task directory, compiler scratch, parsing or process RSS.

Fresh per-export Cargo targets can be much larger than the selected files.
The outer supervisor must budget the complete task root, scratch, child groups,
runtime inputs and failure retention. A 64KiB source limit is not a build-storage
limit. Cooperative checks are not hard real-time guarantees.

A source receipt records selected paths/identities, results and original stream
pins. It does not authenticate its own bytes; independently pin the receipt.
It grants no source authentication, compiler-closure attestation, protected
proof/production resume, native qualification, physical-register lifetime,
GPU execution or performance claim.

Pure/device controls and actual source captures are separate evidence. Fresh
LLVM/native qualification needs new selection-specific observations, not a
renamed repeat receipt or inferred match from an identical short sequence.
This advances bounded compile-time authoring, not general M2, U2 or any other
broad milestone closure.
