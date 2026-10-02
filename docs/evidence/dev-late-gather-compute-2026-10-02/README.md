# Native Late Gathered Frame Consumers

Implementation baseline: `3c8afa96a2a9f0cb71da500b125054ce08153824`.

## Scope

The ordinary native multi-device path now has hardware qualification for a
compute consumer admitted after an earlier gather copy has published and while
its native ownership is retained. All source computes and ordered gather copies
are admitted first. The consumer depends only on the exact latest gather event.
After the oldest peer is seeded, only final-readback-stream progress drives the
compute, remaining gathers, return peer and host readback.

The new `--late-gather-compute` and `--late-gather-compute-overlap` modes preserve
the unchanged finite R57 qualification authority, allocation requirements and
full-byte oracle. The existing prequeued and direct late modes remain intact.
Only two example files change; production runtime, KFD and model sources do not.

Ordinary Context drain can finish a fast copy before returning. The seed uses
bounded positive deadlines but never treats timing or Pending alone as proof of
publication. Immediately before and after consumer admission, every gather peer
must remain publicly Pending, the retained native count must be one, and the
completed native count must be zero. A missed interval rejects the witness.
The ordered shared-destination roster identifies the oldest retained root;
the public counter does not directly inspect its two child reservation markers.
Actual production consumer admission authenticates that paired custody.

## Qualification

All 18 selected MI300X cases pass on the exact rebuilt witness:

- Eight new late cases: three and four GPUs, forward and reversed UID order,
  with disjoint and overlapping destination windows.
- Eight matching prequeued gathered-consumer controls.
- Two existing direct late-admission controls, with both device orders.

The independent Python oracle reconstructs complete source-D, gathered-C,
computed-D, returned-E and host buffers, including ordered overwrites and all
logical guard bytes. Every gather case matches its length-prefixed digest and
exact callback roster. Across the complete matrix, 136 successful callbacks and
60 completed logical native peer copies are checked. Retained native ownership
returns to zero, results release in reverse dependency order, and explicit owned
shutdown succeeds. Direct late controls retain their narrower source-preservation
scope; they do not independently inspect the full computed source afterward.

Fresh CPU checks pass all 2,198 runtime library tests, with 32 existing hardware
ignores, and all 13 witness tests. The latter include three new CLI, publication
gate and report/oracle regression tests. Strict witness Clippy, changed-source
formatting and whitespace checks pass. Captured test rosters match every test
outcome without filtering.

The witness uses `--no-default-features --features hardware-qualification`.
Its local, uploaded and final remote SHA-256 is
`a053fd742e3d33d016d83a76bac531ec9eb16afdc563ca48991e1c187ebed6b0`.
The source-bound campaign ran from 20:22:35 to 20:31:33 UTC on 2026-10-02.
There were no rejected native cases or retries in this campaign.

## Shared Host Safety

Cards 4 through 7 were checked for exact UID/BDF, idle use, baseline VRAM,
process roster and process-to-device attachments before and after every case.
Host available memory was checked as well. GPU 0 retained foreign work and was
not used. These are point observations, not an exclusive reservation or a
continuous occupancy census.

The owned executable process was confirmed absent before removing its exact
file and fresh scratch directory. Final hash, directory absence and restored
device/process observations are retained. No device reset, foreign process
termination or broad temporary-directory cleanup was performed.

## Evidence And Limits

`audit-01.json` verifies the selected CPU commands, complete 6,119-file source
inventory, witness identity, all native command/output hashes, chronological
admission and cleanup receipts, exact schemas, byte digests and totals.
`raw.tar.xz` retains those receipts, controllers, source inventories and the
candidate patch; `raw-manifest.json` hashes every member. `SHA256SUMS` binds the
README, manifest and archive. No executable is distributed in the packet.

Publication capture remains timing-dependent and fail-closed. Retained custody
does not establish that a GPU fence is still active or prove physical overlap.
This is one finite pipeline batch per process, not arbitrary graph support,
expired-deadline resumption, native fault isolation or general application-kernel
authority. Logical guards do not cover hidden physical pool padding. No solver
or matched performance campaign ran; no whole-adapter formal refinement or
HIP/HSA parity is claimed. Generated Context arguments remain CPU-qualified
only and are not used by these native witnesses.
