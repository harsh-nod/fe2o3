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
   reads. `process_vm_readv` remains the primary path. Only `EFAULT` may enter
   the `/proc/<pid>/mem` fallback, which is opened after ptrace/pidfd custody
   and prior local suspension ownership are established. It reads only the
   already validated header ranges through kernel-mediated procfs on the
   qualified host.
4. Rereads all eight headers and rejects range/binding substitution.
5. Reobserves the queue and device through direct KFD before returning.

The local live-session state must still retain the authority produced by the
prior successful suspend, but capture does not reobserve runtime or physical
suspension state. Reads are sequential and non-atomic. Each adjacent pair is
checked for equality, but changes outside that pair cannot be detected.
`complete` therefore means every announced extent was captured, not that all
segments represent one coherent hardware instant.

The default content limit is 32 MiB. The hard limit is 185,630,720 bytes, the
complete eight-XCC context extent, with at most 16 non-empty segments.

## Privacy and agent surface

Raw segments stay in private `Zeroizing<Vec<u8>>` owners. Access requires the
explicit in-process `with_private_bytes` callback. Ordinary `Debug` output
prints `<private>`, and the Live GPU V3 JSON protocol never serializes bytes,
addresses, native IDs, descriptors, handles, or process IDs.

Agents receive one of three typed results:

- `complete`: checkpoint/content correlation identities, byte count, segment
  count, and `private_bytes_exposed: false`;
- `truncated`: required and configured byte counts, with no partial content;
- `unavailable`: an exact header, read, stability, or binding reason.

The outer Live GPU session separately reports its declared artifact binding.
The checkpoint does not claim that artifact was loaded or executed. This lets
an agent cite and compare captures without gaining attach, resume, memory-read,
or checkpoint-byte authority; the hashes provide correlation, not authentication.

## What remains unavailable

The installed public Linux KFD UAPI describes the outer context header and its
ranges but does not describe the inner gfx942 wave, SGPR, VGPR, lane, or PC
records. fe2o3 therefore does not decode those records and continues to report
wave, lane, register, PC, source, and target-memory observations unavailable.
Using kernel-private trap-handler assembly as an ABI would make the trust claim
unsound and is explicitly excluded.

The MI300X live-validation lane retains its earlier queue suspend/resume and
complete zero-byte idle checkpoint. The 2026-09-03 active qualification adds a
finite one-Wave64 dispatch. One public KFD header reported a 20-byte control
stack and 2,304-byte wave-state range; the other seven XCCs reported empty
ranges. `process_vm_readv` returned `EFAULT` for the AMDGPU BO range, and the
ptrace-authorized `/proc/<pid>/mem` fallback captured all 2,324 bytes twice.
The exact queue was resumed, the target verified all 64 outputs, disabled its
runtime, released the queue resources, and exited before the debugger session
finished.

The target emitted adjacent sequential declarations naming the checked
artifact digest, dispatch identity, native queue, GPU, and packet. The debugger
joined those records to the one exact KFD queue that it queried, suspended,
captured, and resumed. This is same-queue runtime-observation evidence. It is
not a coherent-interval proof and does not independently authenticate which
code-object bytes were physically loaded or executed.

Useful decoded stopped-state inspection still requires a stable, documented
direct-KFD decoder interface or a separately versioned and reviewed decoder
with exact driver/firmware provenance; neither exists in the current public
interface set.
