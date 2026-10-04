# Opt-In Shared Full Currentness

`Gfx950EngineeringPeerGroupV1::configure_performance_v2(cache, operational, true)`
selects a bounded group full-currentness fence before user resources exist.
The existing `configure_performance` API and default group path retain their
per-rank full discovery and existing fence placement.

For each opted-in full group fence, the implementation:

1. Runs every participant's existing full pre-topology checks: retained process,
   reset event fence, KFD descriptor/sysfs, render descriptor, UAPI, DRM identity
   and VRAM-loss observation, XNACK, and complete process apertures.
2. Discovers one fresh, complete, generation-consistent gfx950 host topology and
   compares it exactly with every participant's retained snapshot.
3. Runs every participant's existing post-topology descriptor, process, XNACK,
   DRM and reset checks.
4. Rechecks the topology root directory identity and generation after the last
   participant, then checks every queue's idle state.

The snapshot is local to this call and cannot be reused by another fence or
returned as authority. Failure poisons every device and the enclosing group;
idle failure also poisons the group through its existing terminal transition.
All host access, mapping, release, dispatch, sequence and round boundaries stay
in place. Single-context allocation/free/queue lifecycle checks remain full.
Peer mapping ownership, read-only foreign arguments, argument/ABI validation,
queue capacity, publication, completion and failed teardown are unchanged.

The optimization removes repeated whole-host topology discovery within one
group fence (two/eight scans become one), not mutable participant checks. It
does not reduce TP1 operational dispatch checks or parent IPC, and makes no
all-reset, topology ABA, device arithmetic, or GPU performance claim. A topology
generation may not authenticate arbitrary privileged sysfs changes; the same
contracted kernel/sysfs observation assumptions as the existing full check
remain. Final generation rechecking detects generation drift after the shared
snapshot while later participants are checked.

Host fault tests exercise the actual group algorithm, all before/after failure
positions, retained-rank mismatch, snapshot failure, final generation failure,
topology change between ranks, terminal poison and no cross-call snapshot reuse.
Filesystem fixtures cover final root/path/inode/device/generation drift and a
missing generation file. These do not substitute for native retained-buffer,
dispatch/round, release/close and reset/error qualification on TP2/TP8.
