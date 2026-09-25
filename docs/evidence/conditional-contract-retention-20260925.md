# Conditional contract retention and runtime transport

Continuation of [generated-field projection](conditional-generated-fields-20260925.md)
for issue #272. This is implementation progress, not closure of M1 or a new
47-kernel qualification result.

## Production compiler change

The existing conditional target replay now encodes a bounded invocation contract
from its checked generated fields and reimported signed formula execution. It
retains the actual compiler kernel identity, exact graph and source subjects,
CPU reference subjects, theorem and staging identities, ordered typed roots,
canonical/source/generated argument coordinates, output, ordered read occurrences,
and runtime premises. The encoder validates the existing shared-IEEE theorem
preimage. It does not turn that implication into LLVM or ISA refinement.

The canonical bytes are copied inside the generated-field scope. That scope
returns owned data **unreserved**. The original conditional root reserves the
returned payload on its existing account before the enclosing CPU, verifier and
lowering postchecks. Only a successful replay installs it beside the retained
source arena, runtime lease and signed proof. Repeated replay keeps old and new
payloads charged simultaneously, requires equal bytes, and destroys the old
payload before releasing its reservation. Failure consumes the root; terminal
phase cleanup follows destruction of its owners and preserves work/denial history.

There is no public constructor for this retained owner, no copied work token,
replacement ledger, ordinary-proof conversion, or new launch authority.
`FE2O3-COND-FINALIZER-001` remains the production boundary.

## Host and runtime change

The generated-host preparation helpers compare every logical and physical ABI
field, including scalar fields outside the memory-premise roster. Conditional
preparation retains the original sealed packing plan with bounded deep-copy
accounting. Ordinary packing does not take that extra copy and keeps its existing
bytes and identities.

Runtime transport carries the mandatory conditional payload into the existing
KFD request. The unsafe authority interface must explicitly identify its
invocation family; missing or mismatched contract/premise bindings are rejected
before currentness callbacks, debug setup and native execution. KFD checks live
allocation spans, alignment, output separation, address arithmetic, and mapping
and publication generations before queue publication. Input/input aliasing is
not confused with forbidden output overlap.

These are inert transport and preparation components. The existing authenticated
host entry still refuses the conditional family. Connecting it requires retained
V4 finalizer, semantic and machine-proof evidence, publication currentness, and
the original account through preparation and completion. Public descriptor bytes,
matching hashes and prepared premises cannot supply that custody. See the
[host/runtime integration handoff](issue272-host-runtime-worker-20260925.md).

## Validation

The guarded local runner uses the pinned nightly, one Cargo job/test thread,
offline locked dependencies, hidden GPUs, disabled HIP, a 12 GiB virtual-memory
limit and a 1,200-second deadline. Source and tool hashes are compared before and
after every build. Reports and logs are in the issue-272 production-next evidence
directory; these runs have no protected-proof or hardware credit.

`conditional-contract-retention-r1` passed 83 tests with 11 ignored on the
compiler-only candidate. Source inventory: 8,124 files,
`baa43a7b1cb4869365a82eb08adb8e40e5be89b2e2ed97dad507fb5e8536ae4f`.
Log SHA256:
`799187f564f8038ca2f4d0b2131a23bfbb217dfe9674ecfbf5f9c71ed1c44f53`.
This includes 13 encoder/projection controls and nine retained-storage controls,
plus existing generated-field, original-phase and CPU/source replay regressions.

The protected actual-source Vecadd test now checks retained contract bytes
against the observed source fields and signed receipt, but was **not run** here.
The generic storage tests use arbitrary bytes only to test ownership/accounting;
they do not fabricate a successful source-bound proof.

## Remaining acceptance

- Run the exact integrated source through the protected actual-source proof test.
- Carry conditional evidence through target lowering, applicable machine
  refinement, V4 artifact finalization and independently authenticated safe-host
  admission, without weakening the current refusal gates.
- Exercise full host-entry substitution and cleanup negatives, then protected
  target-matched Vecadd hardware execution.
- Generalize and qualify the complete manifest, including synchronization,
  structured compute and advanced kernels. No new kernel receives end-to-end
  credit from this checkpoint alone.
