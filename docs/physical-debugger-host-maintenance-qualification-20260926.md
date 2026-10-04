# Disabled debugger host-entry maintenance source checkpoint

Status: source and CPU qualification only. Physical capture remains disabled;
accepted broad exits remain M1/V1/V2/U1/U2/U3 (6/18).

## Implementation

The separate GPL [physical-v3 package](../tools/rocgdb-one-stop-native-adapters-v1/physical-v3/README.md)
adds host-entry maintenance to the exact disabled physical-v2 parent. The parent
package is unchanged. Selection, capture and publication remain false, the V2
wire protocol is unchanged, and the public Rust controller retains PROFILE=None.

The adapter retains the actual native process and allowed top-target references
before the first host-entry commit. It arms maintenance only after the existing
commit and checked flush complete. A later repeated generic commit requires
the same live owner, exact pushed target stack, running host thread, unloaded
runtime, no GPU thread/callback/resume path, and exact roster. Guards bracket
the existing commit effect; maintenance adds no resume, event acknowledgement,
query, sample, output row or flush.

A new resume, MI input, callback, runtime acknowledgement, stop, disappearance,
rejection or unwind retires the epoch. References release normally only when
the same pushed stack and no in-flight effect make that safe. Exceptional
retirement may keep bounded references until debugger destruction and delay
target close; unchanged close timing is not claimed.

The two references and epoch fields belong to measured adapter storage.
The separate 256-byte maintenance scratch must be included in subsequent
actual layout qualification under the unchanged 65,536-byte native cap.
All native command, stop, API, read, work and lifetime limits remain unchanged.

## Reproducible disabled-source and CPU checks

The package includes four exact hook postimages, reversible transforms, patch
series, bounded source reader, placement controls and an inert CPU test using
the actual extracted helper bodies and reference-pointer implementation.
Mocked GDB objects do not establish real native dispatch or close behavior.

Both selected source stages have 63 files. The extended parent contains
2,177,182 bytes and the maintenance stage 2,183,132 bytes. The versioned v3
selected-source ceiling is 2,144 KiB (2,195,456 bytes); physical-v2 keeps its
2,112-KiB ceiling. The three extra lifetime-context headers are included in
the same sum. This is selected source payload accounting, not whole-process
I/O, memory or native authority.

The relocated public package passed 14 metadata controls and 25 placement
controls, both exact selected-stage verifiers and the pinned API-header check.
The unchanged parent separately passed 14 metadata and 20 placement controls,
its exact source verifier, and strict C++ publication/output/resource programs.
The newly built public maintenance CPU executable passed 100 groups and
481 checks. These are 73 Node controls plus the separate C++ checks; no
debugger or GPU was executed.

| Retained evidence | Bytes | SHA-256 |
| --- | --- | --- |
| Disabled source and CPU build | 431,152 | `e6a56b0bda7aaa389a895c647759f2803269556b1e26fe07f83de0320c441be4` |
| Public maintenance CPU probe | 432,572 | `67f1ec7a4d116d5e7b584cbbceb15e09d1d8dfcdd90d6ed07546e21863af2253` |
| CPU ELF | 81,232 | `d7def7145099b3488d0150b1603b498cb7470cfb862a43663581b4331b1add4a` |

## Native work remains separate

The retained native refusal (15) motivated this investigation, but does not
prove repeated maintenance commits caused that failure. The earlier attempt
still has no accepted capture; successful cleanup is not capture success.

A fresh private active debugger requires its own full build, measured layout,
artifact derivation, startup and observed loaded-file qualification, controller
and family binding, bounded replay, fresh hardware checks and one separately
coordinated attempt. The disabled public source checks do not qualify that
active executable or transfer an old startup receipt to a new ELF.

Visualizers must show unavailable data when no accepted same-stop sample
exists, never zero-filled registers, oracle-filled memory or a GPU-observed
badge. This checkpoint supplies no live visualization route or activation
command.
