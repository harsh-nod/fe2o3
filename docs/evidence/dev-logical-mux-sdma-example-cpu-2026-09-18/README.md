# LogicalMux public teardown probe: CPU qualification

This packet qualifies the public `kfd-compute-aql-queue` example's new
`--retained-release-logical-mux-sdma (2|4|8|14|16) <unique-id>` mode. It is not
native evidence. The mode creates exactly two physical SDMA queues, targeting
engines 0 and 1, for each admitted logical-lane count. Logical lanes do not have
independent HIP stream scheduling semantics.

The source cohort starts at signed commit
`95ed0301cb532dd4bd762ec4410f0bd9a61814cc`. Of all 5,558 inventoried inputs,
only the example differs from the preceding LogicalMux retained-release CPU
packet. That packet qualified the production implementation with 136 KFD tests
per GNU/musl target; this packet does not rerun that suite or broaden its claims.

## Coverage

- 19 example tests on GNU and musl, without ignored or filtered tests.
- Every admitted lane count accepts decimal/hex explicit device IDs. Unsupported
  counts, noncanonical counts, missing arguments, extra arguments, invalid IDs,
  and `--all` are rejected.
- The pure observation oracle checks exact requested/observed lane equality,
  exactly two native queues, distinct non-primary IDs, engines 0/1, 4096-byte
  rings and the admitted in-flight capacity. Mutation tests cover both owners,
  malformed rosters and admitted-but-wrong observed lane counts.
- The release probe checks a physical host-account increase of 8192 bytes and
  two records, unchanged device-account state, default SDMA pool state, retained
  selector and preflight admission, 11 returned resources, full account refunds,
  inert one-shot retry rejection and completed public-root drop.
- Strict KFD all-target/all-feature Clippy, a non-test static musl ELF build,
  formatting, whitespace, and unchanged complete source inventory.
- Five verifier calibration tests, including 12 malformed test transcripts,
  missing/matching/mismatched/existing archive seals, and hostile file membership.

One preliminary GNU developer run passed all 19 tests before the independently
reviewed admitted-lane-mismatch and boundary-17 cases were added. The recorded
qualification runs contain the final source. No production source changes are
included in this packet.

After the initial four calibration tests, independent review found that the
inherited manifest helper excluded nested files named `SHA256SUMS`. This packet
now constructs its own exact manifest, excluding only the archive-root seal.
The original receipts are preserved unchanged. The later `manifest-calibration`
receipt reruns those four tests plus the new closure test, rejecting nested
seals, extra files, missing files and symlinks. Its command was appended to
`qualify.sh` after the initial run finished and executed separately via
`record.sh`; replay includes both calibration commands. No source-cohort input
changed during this evidence-verifier correction.

## Replay and identity

`qualify.sh` records each command, timestamps, output and exit status. It refuses
to overwrite prior receipts. Replay requires a fresh unsealed archive, the
recorded Rust toolchain, both Rust targets, dependencies and a writable target
directory. The shared local target symlink is a build-cache convenience, not
evidence of executable identity.

Run `python3 -B docs/evidence/dev-logical-mux-sdma-example-cpu-2026-09-18/verify.py`
for a read-only historical audit. It checks receipt commands and chronology,
exact passing test identities, source continuity, binary observations and the
sealed file set without executing the recorded qualification commands.
`--live` additionally executes the SHA-pinned source selector and hashes the
current musl executable. `--seal` exclusively creates `SHA256SUMS` once; even
`--allow-unsealed` must verify an existing seal.

The receipt helper, strict test parser, source selector and preceding
production CPU seal are SHA-pinned in `verify.py`. The executable digest is in
`raw/binary.log`; source snapshots are in `raw/source-{before,after}.log`.

## Limits

No GPU was used for these receipts. CPU oracle tests do not prove that public
native creation or teardown succeeds. Native qualification must use the exact
recorded ELF and the signed containing source commit, with fresh shared-host
idle admission and cleanup evidence. The cursor marker is explicitly
source-qualified: no public cursor observation exists, and this probe publishes
no work. This packet does not establish logical scheduling, cursor advancement,
submitted-work drain, native failure retention, aggregate residency, formal Rust
refinement, performance or HIP/HSA parity. R126/A1/A2 and issue #182 remain open.
