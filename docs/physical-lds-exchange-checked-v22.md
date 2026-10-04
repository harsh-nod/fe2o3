# Checked LDS source to inert LLVM/handoff (V22)

The normal continuation consumes the actual MIR39 source/materialization owner,
not diagnostic KIR bytes. It retains one immutable KIR22 executable, the complete
typed memory report, a conditional ranked safety projection, mandatory analysis
reports and source/ABI correspondence. The same canonical verification work
ledger is cumulative through emission and handoff. There is no V12 conversion
or decoded-file path that recreates compiler custody.

## Public normal-source command

With the pinned compiler/backend built as in the
[source lesson](physical-lds-exchange-source-v22.md):

```sh
node scripts/physical-lds-exchange-checked-v22.mjs one /absolute/new-lds-checked
node scripts/physical-lds-exchange-checked-v22.mjs registers /absolute/new-lds-register-checked
```

The same seventeen negative case names are supported. Each stage recompiles
current source through the ordinary wrapper: FE2O3_EXTRACT_GFX942_LLVM_PATH_V1
then FE2O3_EXTRACT_GFX942_COMPILER_HANDOFF_PATH_V1. The outputs are unchanged
canonical.ll and handoff-v2.bin, not a native artifact. The handoff's worker text
adds the existing descriptor data to the unchanged canonical executable text;
it does not insert executable setup or tail.

A closed diagnostic relation joins the emitted LLVM and handoff hashes and
declares the same canonical identity/descriptor digest, frame size 512 and
required 128 participants. It explicitly reports LDS execution false and all
authority flags false. Its parser rejects missing/extra/duplicate fields,
wrong versions, foreign output hashes and attempted authority promotion.
This is presentation evidence, not a wire decoder, compiler attestation or
runtime binding proof.

## Retained conditions

The combined typed report keeps all of these, not just global accesses:

- Four actual kernarg reads, their source/SSA identities, offsets 0/8/16/24,
  eight-byte widths and LGKM readiness; a live readable immutable 32-byte
  prefix aligned to 8, disjoint from output writes.
- The full-EXEC global input load, opaque data result and immediate VM wait.
  The first 128 u32 elements must be readable and initialized: 512 bytes even
  when output length is zero.
- The actual guarded output store of the ready peer LDS value, its VM wait and
  EXEC restoration. The conservative formal output minimum remains 512 writable
  bytes, not the short CPU mask length.
- Input/output nonaliasing and valid shared/exclusive source roles; distinct
  logical argument IDs alone do not establish distinct runtime allocations.
- The actual typed 512-byte frame, local write, write LGKM completion, LDS-only
  epoch-1 workgroup publication, xor-64 peer read and read LGKM completion.
  Exactly one complete workgroup of 128 invocations must participate.

These conditions survive normal ABI/descriptor preparation and inert handoff.
Clean mandatory reports do not discharge runtime pointer validity, allocation
lifetime, permissions, initialization, disjointness or device participation.
No host, protected-finalizer, semantic-handoff-V3 or launch owner is returned.

## Authored graph and ranked safety graph

The authored one-block SSA/CFG, instructions and output comparison remain in
the immutable executable. The ranked graph is a separate memory-safety
projection, not an optimized replacement or numerical equivalence theorem.

Under the retained conditional 512-byte input/output requirements, it uses
128-element prefix views. It does not assert the actual dynamic lengths equal
128 or replace the executable output mask. For the exact single workgroup,
global_id equals local_x; the projection retains that source-derived relation.
The xor-64 peer index is represented as (local_x + 64) % 128, equal for every
local_x in 0..128. Actual byte-address multiplication/SSA lineage remains checked.
The projection's barrier trace is workgroup-wide, not merely one active wave.

Global loaded data and the peer read are opaque values for safety analysis.
They are not constants or deterministic functions of their addresses. The full
typed source/formal report, ranked graph/text and middle-end evidence are replayed
against the same owner. Arbitrary unknown operations are not treated as safe;
generic memory extraction remains unmodeled for this physical profile.

The ledger accounts bounded logical added-owner/report/graph/text payload and
work; preexisting semantic/planner domains remain separately bounded. Fixed
stack scratch is structurally bounded and is not a claim of full stack, heap
allocator or RSS accounting.

## What this catches, and what it does not

Source and exact-profile validation reject wrong target/launch/frame, foreign
arguments or marker families, missing/incorrect waits or barrier placement,
wrong peer/address/carry lineage, stale or incorrect store results, and illegal
register/operand forms. CPU controls can additionally expose input bounds,
initialization, aliasing, mask and barrier-model errors.

This finite profile is not arbitrary assembly verification or hardware
conformance. It has no general loops, divergent barriers, multiple epochs,
atomics, dynamic LDS, independent outstanding memory scheduling or physical
GPU capture. Runtime and native qualification remain separate. See the
[dated record](physical-lds-exchange-qualification-20260924.md) for exactly what
has been run; no broader milestone is closed by these files.
