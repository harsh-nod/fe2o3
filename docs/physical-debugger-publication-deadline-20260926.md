# Private debugger attempt after initial readiness — 2026-09-26

The bounded initial command-line readiness fix was qualified in the public
disabled controller and a freshly bound private controller/family. A new
single native attempt did **not** produce an accepted capture report.
Public activation stays disabled; broad accepted exits remain 6/18.

## Qualified prerequisites

The private controller passed 136 Rust tests, strict package Clippy and build.
The freshly bound family passed 32 Rust and 191 Node controls. Its deployment
build and read-only two-sweep replay passed. The replay charged 284,808,557
bytes and 4,929 bounded read calls, retaining two startup and two benign
observations without invoking the controller, debugger or target.

Before launch, root independently rehashed the 230 selected static inputs,
accounted for every predecessor role, reviewed the unchanged single-attempt
wrapper and rechecked the exact boot, loaded-module build ID, installed/DKMS
module digests and selected gfx950 topology. Workers and compiler gates were
quiet. Fresh coordination and one retained FD9 exclusive lock covered the
attempt; historical leases were not reused.

The trusted kernel/driver/firmware and owned-process lifetime assumptions remain
explicit. These checks do not prove resistance to privileged interference,
hardware TTMP readback, or absence of unrelated host GPU users.

## Actual failure and cleanup

The native gate exited with code 2 after 95,742 ms. The controller produced
zero stdout bytes. Its 332-byte stderr reports a raw-observation publication
refusal with `Deadline`; it retained debugger-child reaping, completed streams
and joined reader threads. It did not establish controller-side reaping of the
unadmitted inferior. The separate family owner adopted and reaped that child.

Because the raw report was absent, the outer parser refused its empty input.
This is not evidence that the earlier empty-command-line failure recurred,
nor evidence of a successful physical stop or capture. Native activity was
possible; dispatch and producer capture remain unknown, not fabricated false
observations of no activity.

Root independently rejoined request, manager generation, inner/outer cleanup
receipts and terminal acknowledgement. The scope was absent and all five
retained process IDs were absent. No cleanup deadline expired. All failed
artifacts remain preserved; no automatic retry was performed.

Native receipt: 232,123 bytes, SHA-256
`9663f1bad0e4ceff58d9c28fba39d5337756b4fb5ecfc77afb7dfce81e57eeb1`.
Terminal audit: 17,430 bytes, SHA-256
`2af21d7ed5984e5600ce554cb99f53954bc23a05c4e408b48a3c6415c8bf7d3d`.
Independent cleanup audit: 699 bytes, SHA-256
`f26693798f4ae476d4c85caf817c20d84df5acd615c181cbaab6671cbe99d2c7`.

The wrapper retained its original 300-second deadline, 512 MiB read limit and
40,000-call bound. Actual wrapper debit was 436,028,950 bytes / 7,565 calls.
Cleanup success is not physical-capture qualification or rollback.

## Qualified failure-only diagnostic

The disabled public controller now preserves a bounded diagnostic when normal
report publication fails. It records the original fixed refusal/status,
post-cleanup command/record counts, stream completion and bounded hexadecimal
suffixes of already-retained stdout, stderr and commands. It does not use an
unbounded debug rendering of a successful observation.

Formatting uses a fixed 4,096-byte stack buffer (at most 4,097 bytes including
the line ending), with at most 256 retained bytes from each stream/command
suffix. Checked growth and a fixed fallback cover formatting failure. Arbitrary
input bytes are hex-encoded, not interpreted as terminal control sequences.

This failure branch performs no new child reads, process queries, hashing,
parsing, debugger commands, spawning or teardown. The successful publication
path, original clocks, cleanup protocol and public-disabled state are unchanged.
Bounded content is not a guarantee of global stderr write latency.

Qualification passed 58 library and 83 controller Rust tests, 31 public package
checks and four private source-preservation controls, strict package Clippy,
build and diff checks. The receipt retained equal before/after source, input
and tool pins; root independently rehashed its inputs and streams.

CPU receipt: 100,865 bytes, SHA-256
`9d48d01aa078c9dd555163ac5ca14e66bc72b4f254d57f7672aba59b39ccd3e6`.
Its source snapshot contained 8,404 files / 120,110,614 bytes, SHA-256
`baddfdad48a2ce9d716c188029c3e641b2fa73bc95c02c3436f1860cd615a175`.
This documentation update follows that source qualification.

No new native attempt was made with this change. The original capture remains
unknown; cleanup success is still distinct from capture qualification. A fresh
private controller/family, reviewed binding and separately coordinated bounded
attempt are needed to observe the underlying failure. Milestone exits remain
6/18; this diagnostic neither accepts partial state nor enables public capture.
