# Public Unknown Transition Records

Source: `710f4bfe94aef3077a9de9b7e4c3d1341864f45a`.

This developer record covers `mark_unknown` through the producer owner, stable
owner and journal adapter, down to the unchanged shared retained-chain executor.
It is not native admission qualification or HIP/HSA performance evidence.

## Results

- Raw actual-owner root: 193 verified, zero errors.
- Whole historical root: 728 verified, zero errors, twice at default solver limits.
- Nine scoped executable mutations rejected with logical verification errors.
- 935 unit tests and 27 doctests passed; 18 tests ignored in the normal run.
- Formatting, all-target Clippy with warnings denied, and release test build passed.
- Pinned Verus 0.2026.08.09.92f466f; Rust 1.97.1 for the recorded CPU checks.
- Verus distribution closure checked before and after: 190 files, 129019839 bytes.
- No GPU execution, elapsed-time benchmark, or HIP/HSA comparison was run.

## Scope

Raw execution is total over the actual declared owner types. It requires no
reader/producer count storage, shared budget, custody, issuer-ID, writer-kind or
physical-capacity premise. Every call validates the retained chain, including
already-Unknown calls. Rejection and idempotent success preserve exact normal-state
owner values. Pending success changes only the selected writer entry; all other
journal fields and all reader/producer custody fields remain unchanged.

Actual and historical execution run independently. Ten historical declarations
are projected unchanged from the frozen settlement source. Correspondence needs
only representation and matching writer references; invariant preservation is
conditional on an initially valid producer invariant. No issuance-history premise
is introduced. Chain-frame lemmas establish that marking the writer leaves its
retained chain unchanged.

Five synthetic fixture/witness functions cover acquired producer custody,
Pending-to-Unknown status, replay identity, stale-reference precedence, corrupted
already-Unknown chains, irrelevant malformed reader/budget state, and empty chains
with matching zero-ID synchronous writers. These are not Rust constructor or
settlement-reachability proofs.

Five new differential CPU tests compare all three frozen adapters from
`e3ea84c1f3140600e637e9d60e16d95a7be9fd6d` against shared execution. Frozen outer
adapters route through frozen inner adapters to the unchanged current retained
leaf. Tests compare results, full state, storage addresses/capacities and indexed
accesses before reset. Existing lifecycle tests retain all four neighboring
producer statuses. Traversal tests reach 4096 members and capacity 65536: Pending
success uses 2k+2 accesses, repeated Unknown 2k+1, and a late backlink failure
2k+1. These counts establish the tested traversal behavior, not elapsed performance.

The nine controls cover no-op adapters at all three layers, producer-field
corruption, skipped chain validation, skipped mutation, an invalid idempotence
shortcut, skipped actual execution and skipped historical execution. Diagnostics
are captured and checked for scoped logical failure; they are not compared with
precommitted exact diagnostic expectations.

## Reproduction

The source-bound runner is
`crates/fe2o3-runtime-model/verus/check-owner-unknown.py`. Run it from the recorded
source with `--repo`, `--verus`, `--output` and an owned `--target` directory.
It brackets unchanged inputs, verifies source objects and shared adapters, records
normal terminal process receipts, and removes each temporary proof-source tree.

Offline record consistency, including nine rehashed corruption selftests:

```sh
python3 docs/evidence/dev-owner-unknown-2026-09-22/audit.py --repo . --selftest
```

The auditor works with a checkout or bare Git object database. It validates source
identities, complete rosters, commands, proof results, closure transcripts, serial
receipts and CPU results. It does not rerun Verus or establish physical-storage,
allocator/unwind, quiescence-authenticity or native-backend guarantees.

Gate 1 remains open; native pending-consumer admission remains closed. Remaining
owner lifecycle bodies, native integration and matched performance qualification
are still required before broader parity claims.
