# Producer-Owner Stable Wrapper Qualification

This developer packet binds the two public stable-read wrappers to shared execution
bodies and independently executed historical models. It is not native GPU
qualification or full HIP/HSA parity. Gate 1 remains open.

The source-bound campaign records two whole-root positives around 18 scoped controls,
the pinned Verus distribution, source brackets, exact terminal process receipts,
CPU checks and a frozen-versus-shared benchmark. Probes, syntax errors, timeouts and
solver resource exhaustion are not accepted as qualification.

Acquisition preserves context/ID/roster/output precedence before combined budget and
stable capacity/epoch. Its raw domain conditions budget/storage requirements on the
checks that precede their use. Release directly delegates with the stable leaf's
unconditional count-storage requirement and no producer budget/count/epoch premise.
Both preserve producer reservation storage. Synchronous consumers remain admissible.

Eleven historical declarations are copied exactly from the unchanged lifecycle.
Frozen Rust outer methods come from `30aac7720c4a2eef34da3e3682ec5b74784ed61d`;
both variants invoke unchanged current stable/capacity leaves. Test-only opaque
reset/fault helpers do not add mutable production access. Synthetic proof witnesses
demonstrate behavior, not fallible constructor or settlement reachability.

The benchmark contains 64 cases, 896 ordered rows and 11 fields across seven
alternating rounds. O(k) reset, allocation and assertions are outside timing; each
batch's full post-state and storage are checked before reset. The budget fault is
deliberately raw malformed state; a separate CPU test covers legal exhaustion.
Non-exclusive instrumented CPU timing is not a GPU or production latency claim.

Replay from a checkout or Git object database:

```sh
python3 docs/evidence/dev-producer-stable-2026-09-22/audit.py \
  --repo . --packet docs/evidence/dev-producer-stable-2026-09-22 --selftest
```

After committing reviewed source, record proof before CPU checks, serially:

```sh
python3 docs/evidence/dev-producer-stable-2026-09-22/check.py \
  --repo . --verus "$VERUS" --output "$SCRATCH/proof"
python3 docs/evidence/dev-producer-stable-2026-09-22/cargo-checks.py \
  --repo . --output "$SCRATCH/cargo" --target "$SCRATCH/target"
```

Generate `SOURCE`, `RESULTS.md` and `SHA256SUMS` from the recorded campaign, audit,
remove only owned scratch after all children exit, then audit again. Never alter
captured transcripts or promote probes into qualified evidence.

Open boundaries include remaining lifecycle wrappers, physical storage/nonallocation,
allocator failure/unwind, construction/reachability and quiescence authenticity.
Native pending-consumer admission stays closed. No remote GPU is touched here.
