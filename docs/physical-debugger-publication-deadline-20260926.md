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

## Next correction

Retain a small, bounded failure-only diagnostic when normal report publication
itself fails. It must expose the original error/stage without extending the
positive execution deadline, accepting partial state, enabling public capture
or automatically rerunning a target. Source review and deterministic controls
precede any separately coordinated future native attempt.
