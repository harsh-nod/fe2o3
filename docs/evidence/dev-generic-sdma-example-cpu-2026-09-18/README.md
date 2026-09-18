# Single SDMA Native Probe: CPU Qualification

Five CLI tests passed on GNU and musl. Strict all-feature/all-target KFD Clippy,
formatting and source diff checks passed. The non-test musl example was built
as a static PIE with SHA-256
`9fa74e6b09b972546e63bf31e29be58f262b2eb54e80caa261c0088a7b2c8dcd`.
The 5,555-entry source inventory was unchanged across this campaign.

The new `--retained-release-sdma (generic|0|1) <unique-id>` mode checks public
single-SDMA creation and retained teardown, exact configured accounting refunds,
one-shot retry and completed custody Drop, without packets or MMIO stores.
The CPU tests exercise argument parsing, not these native assertions. This is
not a repeat of the full runtime/KFD suites or a formal or performance claim.
The production implementation's separate CPU campaign is retained in
`../dev-generic-sdma-release-cpu-2026-09-18`.

`python3 -B verify.py` checks sealed bytes, exact command receipts/chronology,
complete named CLI harnesses, equal source inventories and the executable hash
receipt. `--live` also checks current source and executable bytes. The checker
uses the hash-pinned strict parser from the sibling native-wait CPU archive;
this packet alone is not standalone. It does not attest a hermetic build or
execute native code. `--seal` is a one-time prepublication operation only.
