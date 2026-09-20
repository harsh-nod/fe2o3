# Source-fed guarded assembly body — 2026-09-20

Status: diagnostic source/CPU/native-compilation qualification, not production
admission, a general whole-assembly API or GPU execution. This is partial M2
evidence for #280 and does not close an original milestone.

## Supported path and ownership

The new `diagnostic_source_body_gfx942` example securely reads at most 64 KiB
of canonical V17, borrows the verified typed owner, checks the complete supported
eight-operation graph and emits at most 16 KiB of LLVM text. Every block,
operation, dataflow edge, branch and effect must belong to the recognized shape.
The existing exact-target lowerer supplies its normal owner/capability checks;
its LLVM output is not parsed or patched.

The profile is `gfx942:xnack-`, Wave64, required workgroup [64,1,1], dynamic
one-dimensional launch, a writable global u32 slice and three u32 arguments.
It accepts an authored ordered arithmetic program followed by exactly its
result stored at global-index when index < slice-length. Helpers, extra
operations/effects, uncovered blocks, different ABI and reserved-register
collisions refuse. Fifteen structural controls exercise the boundary.

This path does use LLVM IR: an ordinary kernel function owns argument loading,
system inputs and global-index calculation. One constrained assembly unit owns
arithmetic, address calculation, unsigned bounds masking, a global store, wait,
EXEC restoration and termination. This is not a naked entry and does not give
the caller control of every physical ABI register.

The five authored roles must avoid v0..v7. Diagnostic lowering uses s16 for
pointer-low, v4 for pointer-high, s18:s19 for length, v0:v1 for global index,
v2:v3 for the computed address, and s20:s21 to save EXEC. All net scratch,
VCC/SCC and memory clobbers are declared. EXEC is unconditionally restored
inside the single opaque unit even for an empty mask, so it has no net clobber.
LLVM materializes the typed v4 input; the body does not prescribe that prologue.

## Example source

The exact fixture is
[`source.rs`](../../crates/fe2o3-amdgcn-model/examples/diagnostic_source_body_gfx942/source.rs).
Its central code is:

```rust
use fe2o3_device::{DisjointSlice, amdgpu_ordered_program, kernel, thread};

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn ordered_u32_program(
    mut output: DisjointSlice<u32>,
    a: u32,
    b: u32,
    c: u32,
) {
    let value = amdgpu_ordered_program! {
        gfx942_xnack_off_wave64;
        scratch(32); out(33); in(34) = a; in(35) = b; in(36) = c;
        xor(scratch, input0, input1);
        and(scratch, scratch, input2);
        xor(out, input1, scratch);
    };
    if let Some(element) = output.get_mut(thread::index_1d()) {
        *element = value;
    }
}
```

Normal fresh-source export precedes the diagnostic. Given the actual supported
V17 file, the example command is:

```sh
cargo run --offline --locked -p fe2o3-amdgcn-model \
  --example diagnostic_source_body_gfx942 -- /absolute/path/source-body.kir
```

stdout is LLVM; stderr records separate raw/canonical/LLVM identity observations.
It does not issue an artifact, source-custody owner or continuation permission.
See the [native experiment instructions](../../tools/fe2o3-llvm-link-worker/tests/source-body-abi/README.md).
This diagnostic is not a replacement for the tutorials' existing public
instruction-edit/debugger workflow or its pinned compiler revision.

## Exact fresh-source qualification

All work ran on mi350-2. Evidence root:
`/home/harmenon/fe2o3-authoring-280-282.FEW3gj` (ROOT below); charged cache:
`ROOT/rebuildable-cache-phase10.fpFy5o` (CACHE).

Source run `CACHE/secondary/phase12-source-body-r5` passed 105 stages using
measured current primary host tools and a source-only checkout of
`75fc3304846e5ef6b4ebc094d90acc5024e3fe42` with one fixture-leaf replacement.
It is not clean-75fc host reproduction. Builds were
`phase9-binaries-r25` and `phase11-source-body-build-r6`; structural controls
`phase11-source-body-unit-r8` passed 15 tests and format r15 passed.

| Domain | SHA256 |
| --- | --- |
| Source receipt, 56,392 bytes | `4949870c6f63cb13c8496d7b32679181d57f35b8c674fae7f91b3cb91abb9a65` |
| Worker receipt, 1,129,676 bytes | `39f33bd5db36c4d3df804914eeba4527eb70f5e32f276dbb14b094a01424f2cc` |
| Fixture leaf, 910 bytes | `3e4e61dc53c57beb6bd7de7e626c03d7f8136d96dc0bb4df16d858f5d2f22958` |
| Raw V17, 1,017 bytes | `5985dff32489a54d14021062f58227499ee59611c70e7210849922e6da0a129d` |
| Typed canonical identity | `7fd87cfbfb229d3788378cd9786c800ea046a7c2e70ad9db930cd7cb352d98f9` |
| Actual LLVM, 2,180 bytes | `1524ac60f31913838a29f245459816c2e2051562589774964c725718816bf79f` |

The unchanged existing CPU simulator exercised the actual source-produced
owner. Six scalar triples, eight lengths (0,1,63,64,65,127,128,129), and two
rounded launches give 96 cases: 6,924 output words, 13,056 invocations, 52,992
backing bytes, including 25,296 unchanged bytes. An independent u32 oracle,
`(a & c) | (b & ~c)`, checks values, initializedness, inactive slots and canaries.
All 46 selected inputs, 105 stage stream pairs and 198 artifacts were rechecked
independently. The source run's full census was stable at 5,992 files,
91,738,782 bytes, SHA `4b516d5a9c98c6b3058a482df550d88f93c87692a543724151a5bc0e35371c1b`.
Later native-test/document changes do not inherit that historical census.

## Native compilation and independent decoding

Run `CACHE/secondary/phase12-body-native-r4` passed normal worker compilation,
LLD linking, post-link metadata and complete-entry decoding at O0 and O3.
The exact LLVM above is joined by bytes to the source run. The standalone
runner itself labels its source receipt as unauthenticated context and leaves
`exact_source_join_verified=false`; the separate root/independent retained-byte
audit establishes the observation join, not authenticated source/build custody.

Its 300,651-byte receipt has SHA
`9689a82b2be0d34c0fc0d3032fa5de7736e2e6de6f9c043505d837574d6844b5`.
The untruncated 53,113-byte native report has SHA
`9fe25e8b68758d3a3e447ebe60f117b9f412bd0b890171b0810ed95d41c3c97d`.

| Native observation | O0 | O3 |
| --- | --- | --- |
| Complete entry | 36 instructions / 176 bytes | 21 instructions / 100 bytes |
| Compiler-owned prologue | 24 instructions | 9 instructions |
| Assembly tail | 12 instructions | 12 instructions |
| Kernarg / hidden kernarg | 288 / 256 bytes | 288 / 256 bytes |
| Encoded VGPR capacity / required high-water | 40 / 37 | 40 / 37 |
| HSACO observation | 5,784 bytes | 5,464 bytes |

Both retained the exact three arithmetic encodings and nine-instruction
address/mask/store/wait/restore/end tail. The test verifies exact opcodes,
explicit definitions, explicit operands, sorted implicit reads/writes, store
width and exact global-store bytes `008070dc02217f00`. e32 VCC is implicit
where the MC descriptor says so, not incorrectly treated as an explicit operand.
Seventeen actual typed-LLVM substitutions refuse before compilation.

Metadata retains 28 explicit bytes aligned to 32 plus the unchanged 256 hidden
bytes; no LDS/private/dynamic stack is introduced. Capacity is not a lifetime
or occupancy proof. Full HSACO payloads are not retained by this experiment:
the report records payload hashes and complete entry bytes. Native execution,
hardware output, races, hazard freedom and general semantic refinement are
not established.

## Actual source refusals

Run `CACHE/secondary/phase12-source-body-negative-r1` passed 17 stages using
the current positive's selected Rust/profile/emitter/tool inputs. Each case
uses its own fresh source-only checkout and one exact mutation of the 910-byte
positive fixture. Both export successfully through the normal frontend,
produce distinct raw/canonical identities, then refuse at the diagnostic
profile with exit 1, empty LLVM stdout and the exact designated stderr.

- `scratch(32)` to `scratch(4)`: `program collides with diagnostic ABI scratch`.
- Add an unused u32 argument: `expected writable global u32 slice and three u32 arguments`.

The 48,483-byte receipt SHA is
`0a3385c73b2facb84897dd0ee92fddc5fae74fc27abf53690b0e68d08dd5e360`.
These are real source/profile refusals, not setup failures or a synthetic KIR
edit. No negative CPU/native executions were needed or claimed. The negative
run's own full census was stable at 91,739,395 bytes,
SHA `4b100da9c30888ba32dae8f6d9574a5540dfa445839212fbe7b61004707a2b14`;
it is deliberately distinct from the earlier positive census.

## Retained failures and narrow corrections

Source r1 stopped at the disk-reserve floor without a final receipt; an
interruption observation is retained without inventing child-exit/reap proof.
Source r2 reached the complete-shape checker and refused Rust's actual
Bool-to-I64 Option discriminant. The profile now admits only exact Boolean
zero-extension to I64/U64 with exact 0/1 control flow; narrower/wrong-source,
sign-extension and changed-arm controls remain rejecting. Earlier r3/r4
source passes belong to their previous flat-store emitters.

Native r1 refused LLVM22's standard intrinsic return attributes; the corrected
guard compares the complete attributes to `Intrinsic::getAttributes`, not
a broad allowlist. Native r2 exposed a real scalar constant-bus violation in
the carry instruction and a reserved EXEC-clobber warning. The v4 binding and
net-preserved EXEC contract above fix those causes. Native r3 then reached
the unchanged machine-effect classifier and refused FLAT_STORE_DWORD_vi.
The verified global pointer now uses the precise global-store instruction;
the classifier was not expanded. All failed receipts/logs remain retained.

## Limits and milestone boundaries

Input/output caps, bounded process supervision, two Cargo jobs, 20 GiB combined
charged storage, 40 GiB free disk and 64 GiB available RAM remain unchanged.
Native new-output cap is 256 MiB and report cap is 64 KiB. Selected file and
runtime-library measurements are not a complete compiler/runtime attestation.
Valid allocation, alignment, address representability and launch remain
external premises. No GPU dispatch, protected-finalizer bypass, public schema,
production source-promotion API or fixed-policy admission is introduced.

M1/V1/U1 remain the only qualified original exits; fifteen remain open.
M2 still needs its general authoring/materialization and broader
branches/helpers/resources obligations. The owner decisions recorded in the
[contract review](../assembly-authoring-contract-review-20260920.md) remain
outstanding; source/native diagnostic success cannot substitute for them.
