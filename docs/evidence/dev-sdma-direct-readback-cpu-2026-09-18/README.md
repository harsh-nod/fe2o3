# Direct SDMA Readback CPU Binding

This fresh packet binds signed commit
`fd1cf3dd05e691797533b1beab1f015c4b2d8fad` to full GNU and musl runtime
library tests and their exact executables. It is preparation for native
correctness testing of direct destination readback, not native or performance
qualification itself.

The recorder, source inventory, executable inventory and sealer are reused
unchanged from the preceding late-selection CPU packet. `qualify.sh` captures
all selected source identities and executable hashes before and after both
runtime test runs. The inventory includes Cargo inputs, all crates and examples,
runtime benchmark sources, the unsafe inventory and the primary-release design
document. It excludes evidence and build output. The verifier checks every
selected file against the signed containing commit, not just changed files.

Builds are locked/offline (`--frozen`), all-feature, low-memory builds with one
test thread. Native tests remain ignored. This packet does not repeat or claim
the previous turn's full KFD suite, doctests, Clippy or other static checks as
new evidence. Toolchain and executables are identified, but this is not a
hermetic compiler/environment closure or a formal implementation proof.

Both suites passed 1,105 named tests with twenty ignored. Their complete outcome
rosters match, including both native cold-allocation tests remaining ignored.
All 5,553 selected source identities match the signed commit and current source;
GNU and musl executable identities are unchanged across execution. The verifier
enforces receipt ordering and joins each build-selected, executed and hashed
executable path. It uses the existing hash-pinned strict harness parser,
including the expected abort-child transcripts.

This is a live-worktree verifier: rerunning it requires the pinned source HEAD,
retained binaries and repository history. The sealed raw receipts remain the
historical evidence after subsequent source changes; the verifier is not a
standalone portable archive auditor. Two read-only reviews found and then
confirmed fixes to the initial verifier's roster, binary linkage, ordering and
source-roster checks before sealing. No test receipt was replaced.
