# Topology Prechecked Reads V1

Development implementation of one path inspection per IO/P2P link-properties
read during each fresh topology discovery. The matched pair-currentness campaign
in `docs/evidence/dev-xgmi-pair-comparison-mi300x-2026-09-18` found currentness
dominated the measured candidate host intervals. That campaign predates this
revision and establishes no performance result for this change.

## Observation Boundary

Previously, link discovery inspected a `properties` path, validated its regular
file type, then immediately repeated that inspection inside the bounded reader.
The private `RegularFileObservation` now binds a borrowed path and its exact
`FileIdentity`. It is not `Copy` or `Clone`; the bounded reader consumes it.
It grants no execution, device, mapping, or queue authority.

The read order is:

1. Inspect the path with `symlink_metadata`; reject a final symlink or nonregular
   type, and capture device, inode, mode, size, mtime and ctime.
2. Open read-only with `O_NOFOLLOW | O_NONBLOCK`; inspect the opened descriptor
   and require exact equality with the captured identity before reading.
3. Read at most the existing byte limit plus one. Oversize rejection retains
   precedence over the post-read descriptor check.
4. Inspect the descriptor again and require the same identity, then validate
   UTF-8 and parse properties using the unchanged parser and error ordering.

Ordinary topology text reads acquire their own observation using the same
reader. Link discovery instead passes the observation it just acquired. No
metadata or file contents are cached across reads, nodes, directions, full
discoveries, or currentness boundaries. Optional-module-field inspection,
directory membership/identity checks, link counts, generation observations,
boot/kernel/module observations and render correlation are unchanged.

The common open is additionally hardened against final-component symlink
replacement and blocking FIFO replacement. An open-time `ELOOP`, including an
intermediate-component loop, is now classified as `TopologyError::Symlink`.
Other open errors retain `Io("open")`; disappearance after preinspection remains
an error, never an absent optional field or successful observation.

These are descriptor-identity checks, not an atomic kernel snapshot or proof
that the pathname still names that descriptor after opening. `O_NOFOLLOW` does
not authenticate intermediate path components. Existing topology directory and
generation envelopes remain unchanged, as do their documented race/ABA limits.
`O_NONBLOCK` behavior on the real sysfs/proc providers needs native qualification;
a provider error must propagate rather than being converted to a successful read.

## Verification Scope

Fourteen focused CPU tests exercise the production path, including regular
replacement, same-inode truncation/growth, final symlink replacement whose target
retains every captured identity field, FIFO replacement, removal, post-read
mutation, size/UTF-8 failures and parser/error precedence. Stable malformed link
properties are tested through both IO and P2P discovery, not only reader helpers.

Test-only thread-local observation hooks count calls to the private `inspect`
helper and inject deterministic post-read changes. A three-GPU fixture contains
15 link-properties paths. Every discovery inspects each exactly once; a second
discovery independently observes changed IO and P2P weights. This is a helper
call-count regression oracle, not a native syscall trace or latency measurement.
The hooks are absent from non-test builds and do not replace production reads.

GNU/musl qualification, strict clippy, feature configurations, unsafe inventory
and source continuity belong to the accompanying CPU packet. Native provider
compatibility, a fresh source-to-source performance comparison, matched HIP/HSA
acceptance and machine-code/formal refinement remain separate requirements.
