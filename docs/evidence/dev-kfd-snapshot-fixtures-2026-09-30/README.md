# KFD Snapshot Test-Fixture Evidence

This compact development packet supports the test-only canonical snapshot change
signed as `1cac2b791822d2c86869ddb5e0e625b83ddf3ad7`, based on
`5cf8266b6620591790e154fdf75110300a583539`. It does not establish native runtime
performance, HIP/HSA parity, formal verification, or full KFD test qualification.

## Source And Scope

Four Rust files change only CPU test fixtures. Canonical snapshots retain the
original byte length, sorted nonzero chunk positions, and every byte of each
nonzero chunk. Zero chunks are omitted only after a full comparison. This is
lossless full-content equality, not a sampled or hash-only approximation.
Production mapping allocation, initialization, and ownership are unchanged.

The packet includes those four signed files, the three original baseline files,
and five unchanged matrix/CWSR/telemetry source files. Complete selected-source
hash inventories and the signed byte-binding receipt cover 6,449 inputs; their
full source bytes are deliberately not copied here. The matching Git checkout,
toolchain, dependencies, and omitted retained test ELFs are separately required
for reconstruction or execution. This is not a standalone or hermetic rebuild
packet, and old absolute invocation paths are historical records.

The signed candidate was detached and is not made reachable by the main branch's
cherry-pick. `packaging/candidate.bundle` therefore supplies its exact signed
commit and four changed source blobs, plus the connecting trees. The bundle is
not a full clone: it requires existing base commit
`5cf8266b6620591790e154fdf75110300a583539` and that commit's source history.
The initial pushes were blocked when this packet was prepared. On 2026-09-30,
both remotes' `codex/r65-runtime-drain-versions` refs were confirmed at
`44c9266dfef479804cc222c3c9a528e42a788841`; the base is an ancestor of that commit.
This publishes the required branch history, not a main merge or a new test run.
With that history available in a checkout, the recovery commands are:

```sh
git bundle verify /path/to/packet/packaging/candidate.bundle
git fetch /path/to/packet/packaging/candidate.bundle HEAD
git -c gpg.ssh.allowedSignersFile=/path/to/trusted-public-signers verify-commit FETCH_HEAD
git worktree add --detach /path/to/candidate-checkout FETCH_HEAD
```

The advertised bundle HEAD must equal `1cac2b791822d2c86869ddb5e0e625b83ddf3ad7`.
The expected signer is `harmenon@amd.com`, public-key fingerprint
`SHA256:q8oGVYZ11904aFzlMkSiEwyeSP+6hbuiZGbNVGRZVCg`. Establish trust in that public
signer independently; no private key is supplied. `bundle-metadata.json` records
the exact bundle SHA-256, base prerequisite, source object roster, and local Git
verification. Preparation checks the pack checksum and restricts its new blobs
to the four signed source files; it contains no ELF/build artifacts.

## Observed Results

- One baseline-first diagnostic pair executed the same complete 24-profile,
  436-case matrix: baseline 305.88 seconds, compact snapshot 103.49 seconds
  (libtest times; build times excluded). There was one pair, no alternating
  samples, no statistical confidence analysis, and no RSS measurement.
- The pre-final-format candidate ran all 1,820 KFD tests without exclusions:
  1,819 passed and one failed in 744.38 seconds. All 1,304 affected cases passed;
  those are a subset of the 1,820, not additional passes. Strict all-feature,
  all-target KFD Clippy passed. The full suite remains rejected.
- The unchanged credential-bound telemetry test failed with `SocketAdmission`
  at `target_debug_telemetry_v2.rs:1173:80`. Separate exact retained baseline and
  candidate ELF observations both reproduced that failure, each with status 101.
  This does not identify the underlying syscall or errno. Diagnostic completion
  is not a passing test, and no socket checks were bypassed or tests ignored.
- After two formatting reorders and a comment correction, a fresh build and
  1,820-name listing passed, followed by five snapshot oracles and one cleanup
  oracle, strict Clippy, four-path rustfmt checks, and whitespace checks. The new
  ELF `61418f89a2acc5f30b68e33bd517278091ea40e2342232145ffce9f25883e4f6`
  differs from the timed/full-suite ELF
  `09c8ce301908b08701a52ed2c13f1eda2799164259460b88dcf2919b4cd0dc7c`.
  There was no final-format full-suite or timing rerun. Exact final CPU-tested
  bytes were then signed; CPU execution was reused before signing, not rerun
  on a signed checkout.

## Preserved History

All seven K attempt directories are included: both formatting attempts, the
diagnostic pair, rejected full suite, rejected first socket recorder, accepted
second socket diagnostic, and final focused CPU/static attempt. Unexecuted draft
recorders and the initial failing synthetic socket control are also retained.
The first compact packaging preparation is retained as an unexecuted predecessor;
it did not yet include the detached candidate bundle and created no real archive.
The first socket recorder rejected the measured standard backtrace hint; its
candidate observation was not launched. The second recorder accepts only that
exact optional hint and leaves the original rejection unchanged.

`historical-timeout/` separately preserves the earlier 1,800-second KFD attempt:
1,813 tests announced, 731 `ok` lines, no final footer, and a timed-out child.
It is incomplete, not the later completed 1,819/1 result. Original planning prose
is retained as history; actual commands, transcripts, and versioned result
receipts determine what was executed.

## Integrity And Limits

`packet-files.json` gives every payload member's exact source path, size, mode,
and SHA-256; `packet.sha256` provides the same content roster. Both controls are
also archive members, with their hashes recorded externally by `packaging.json`.
The packager requires exact membership and ordinary files, rechecks all source
bytes before and after packaging, rereads every archive member, and publishes
without overwriting an existing output. Rejected partial output is retained.

Every selected payload byte is screened for private-key blocks, recognizable
credential tokens, credential-bearing URLs, bearer credentials, and secret-like
values in JSON environment maps. This bounded pattern scan is not proof that
arbitrary secrets are absent. Build/target trees, temporary directories, and all
retained executable bytes are excluded; their recorded identities remain.
No private signing key is included.

Packaging only checks artifacts. Historical process closures are preserved as
the original recorder's observations in its own live namespace; they are not
new process observations or host-wide absence claims. No compiler, solver,
test executable, native GPU operation, or historical PID probe is performed by
the packager. Archive size and final membership are measured only when the
separately authorized packaging command runs.
