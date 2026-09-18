# Single SDMA Probe Accounting Correction

The first native campaign for `a0ab05f3` rejected the new probe before SDMA
creation: its baseline incorrectly expected zero retained host-credit records.
Healthy live backing is retained by the credit account. The corrected oracle
requires retained records equal used allocation records, an exact single-SDMA
delta of 4096 bytes/one allocation/one retained record, then all-zero accounting
after successful teardown and after its rejected retry.

Six example tests passed on GNU and musl. The existing constructed Generic
release test now also checks the actual ledger before/after creation for all
three engine profiles, with and without dispatch; it passed on both targets
(one test function per target, 1,416 filtered). The existing independent final
all-resources-released oracle is preserved. Strict KFD Clippy, the static musl
example build, formatting, diff and unchanged 5,555-file source checks passed.
ELF SHA-256: `596aaed466822f7bf75e7b63f152d2c3c79c50bafb4a7e8ebe3e7ff297583330`.

`preliminary` preserves an earlier successful six-example-test campaign before
the real-ledger assertions were added. The complete campaign was repeated in
`raw` after that test-only change; the non-test ELF remained byte-identical.
`preformat` preserves the subsequent campaign, which passed both example and
ledger suites and Clippy but stopped at a rustfmt line-wrapping diagnostic in
the new test. No source or receipt from either earlier campaign was overwritten.
These are scoped CPU checks, not full KFD/runtime suites or native qualification.
The failed native attempt is retained separately, not relabeled as a pass.

`python3 -B verify.py` checks archive closure, exact final commands/chronology,
named complete harnesses, unchanged source, and the executable hash receipt;
it also checks preliminary commands/statuses/harnesses, the sole source-file
delta and preformat outcomes. Earlier-campaign chronology is not parsed.
`--live` additionally checks current
source and executable bytes; `--seal` is one-time prepublication only. The
hash-pinned parser is imported from the sibling native-wait CPU archive. No
native executable is run by the verifier, and no hermetic-build claim is made.
