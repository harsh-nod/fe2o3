# Direct-KFD opaque checkpoint V1

Status: bounded implementation for GitHub issue #215.

The direct-KFD debugger can suspend one queue already owned by its exact live
session and capture the ranges published in KFD's 40-byte context-save header.
It does not use HIP, HSA, ROCdbgapi, ROCgdb, or a private trap-handler layout.

## What is captured

For the admitted gfx942/KFD 1.18 profile, fe2o3 reads all eight context headers,
validates every control-stack and wave-state range within the published
`0x1621000`-byte XCC stride, and rejects overlaps or malformed pairs. It then:

1. Computes the complete required size before allocating or reading a segment.
2. Returns typed truncation with no retained prefix when the configured limit
   is too small.
3. Reads each non-empty segment twice and rejects content changes or partial
   reads.
4. Rereads all eight headers and rejects range/binding substitution.
5. Reobserves runtime, queue, device, and suspension ownership before returning.

The default content limit is 32 MiB. The hard limit is 185,630,720 bytes, the
complete eight-XCC context extent, with at most 16 non-empty segments.

## Privacy and agent surface

Raw segments stay in private `Zeroizing<Vec<u8>>` owners. Access requires the
explicit in-process `with_private_bytes` callback. Ordinary `Debug` output
prints `<private>`, and the Live GPU V3 JSON protocol never serializes bytes,
addresses, native IDs, descriptors, handles, or process IDs.

Agents receive one of three typed results:

- `complete`: checkpoint/content identities, exact artifact-binding identity,
  byte count, segment count, and `private_bytes_exposed: false`;
- `truncated`: required and configured byte counts, with no partial content;
- `unavailable`: an exact header, read, stability, or binding reason.

This lets an agent cite and compare captures without gaining attach, resume,
memory-read, or checkpoint-byte authority.

## What remains unavailable

The installed public Linux KFD UAPI describes the outer context header and its
ranges but does not describe the inner gfx942 wave, SGPR, VGPR, lane, or PC
records. fe2o3 therefore does not decode those records and continues to report
wave, lane, register, PC, source, and target-memory observations unavailable.
Using kernel-private trap-handler assembly as an ABI would make the trust claim
unsound and is explicitly excluded.

The MI300X live-validation lane currently proves queue suspend/resume and a
complete zero-byte idle checkpoint. A non-empty hardware-written checkpoint is
not yet qualified. Useful decoded stopped-state inspection requires a stable,
documented direct-KFD decoder interface or a separately versioned and reviewed
decoder with exact driver/firmware provenance; neither exists in the current
public interface set.
