# Original-meter assertion helpers — 2026-09-26

This tested prerequisite for #280/#282 adds private resource helpers for the
existing assertion/range evaluator. No evaluator caller has been switched yet.
Broad accepted exits remain M1/V1/V2/U1/U2/U3 (6/18).

## Implemented boundary

Strict helpers use the original preparation adapter and work ledger. Opaque
sets, caches and FIFOs retain its exclusive-borrow lifetime and check the exact
adapter, budget slot and ledger before lookup or mutation. They expose no
budget alias, refund or transferable assertion authority.

Vector growth, container scans, model payload clones/comparisons and complete
typed value/result/error frames are charged before the corresponding work.
The strict FIFO retains its paid consumed prefix; cache replacement at the
existing limit remains legal. Sticky resource denial blocks even cache hits and
spare-capacity mutation. Existing logical assertion counters stay separate from
additional resource-accounting work. Legacy paths retain their standard
containers, counters, limits and refusal behavior.

Raw vectors and model payloads are not ownership-credit transfers. The future
analyzer owner must keep their credits live and drop all values before refund.
The illustrative rejected adapter-reborrow shape was source-reviewed, not
separately qualified with a compile-fail harness.

## Qualification

The first integrated gate, `compiler-bf16-assertion-resource-helpers-r1`, passed
331 model tests, 2,499 backend tests (189 intentionally ignored),
backend/extractor build and whitespace validation. All 18 new helper controls
passed: legacy parity, cache/FIFO behavior, cross-adapter refusal, sticky denial,
exact/one-short work and storage, partial failure/unwind cleanup, payload costs
and large generic values even with zero elements or spare capacity.

Receipt: 38,082 bytes, SHA-256
`3e37ac11c6295a491d944b7b1aa8b2c6be68d926304188a9dd758ee9ac867cdb`.
Source census: 8,393 files / 119,917,586 bytes, SHA-256
`302523b6c85db2635bd52b8146167346a94668071dd9755a7f045c96a633a161`.
The root independently verified unchanged source, inputs, tools, request and
retained streams. This CPU gate did not rerun the genuine-kernel ladder or
invoke a GPU/debugger.

Review corrected missing typed frames in two generic helpers and a fixture
variant typo before integration. Frozen source packets are retained; no failed
compiler run was hidden or replaced.

## Remaining connected work

Wire the full shared evaluator, preserving its real decision mask and legacy
semantics while borrowing the actual source CFG and rich tables. Then retain
that mask in a same-source immutable preparation scope on the original ledger,
including success/error/panic cleanup and genuine-source parity checks.

The existing bounds-check decision convention is not a standalone proof:
canonical bounds/access matching, root memory/effect projection, ranked
correspondence, formal/target/LLVM continuation and ordinary production
admission remain required. Public debugger activation is unchanged.
