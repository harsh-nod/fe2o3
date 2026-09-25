# Physical debugger V2 packages and static build — 2026-09-25

This checkpoint ships a disabled Rust controller and a separately licensed GPL
debugger producer. It qualifies their CPU/source behavior, not a live physical
sample or a production launch. Accepted broad exits remain
M1/V1/V2/U1/U2/U3 (6/18); V4 remains open.

## Implemented boundary

The [Rust controller](../tools/gfx950-one-stop-controller-v1/physical-v2/README.md)
consumes the closed V2 same-stop record. It keeps unsigned 64-bit selectors
lossless, joins the actual MI thread and frame PC with the producer tuple, and
checks either four register bytes plus 272 output/canary bytes or a typed
unavailable result. The full stop/resume protocol still has 17 commands and
the original 60-second controller deadline. A diagnostic sample is not a
compiler source owner, executable capability or reusable resume permission.

The [GPL producer](../tools/rocgdb-one-stop-native-adapters-v1/physical-v2/README.md)
contains two exact source patches and their standalone hooks/tests. Same-client
register/memory reads remain provisional until stop identity, state and
breakpoint-retirement checks pass. Publication checks the exact current MI
stdio chain, performs one submission and one flush, and poisons/revokes after
an error without retry or clearerr. A successful submission is not proof that
the consumer received the bytes.

Independent review found that MI's saved raw stdout member could be read before
initialization. The corrected source explicitly initializes it to nullptr;
the guard was not weakened. Actual stdio error tests include /dev/full.

The public Rust PROFILE is None and all runtime bindings are null. The public
GPL selection, capture and publication flags are false. No JSON, environment
variable or source-file hash activates either package. Old V1 source/parser
behavior remains intact; the V2 package is an explicit separate domain.

## Relocated controller qualification

The actual shipped-path gate ran the V2 source verifier and 22 Node controls,
20 old-package Node controls, 58 library plus 30 transport Rust tests, strict
all-target Clippy, and a binary build. A fresh compatibility test against the
retained V1 consumer passed: a full V2 transcript is not accepted as V1 success.
This is one additional test, not another 88-test run. The controller binary was
built but not executed.

The completed outer receipt is 209,257 bytes, SHA-256
233cea0510948c02e284b9d97df9d6a0d6eeda34b4e935b3cb2892d8b3e8cf17.
The gate used source census 8,094 files / 116,505,691 bytes /
791f0f8693d281eacad612b707ddb9f1468327710a9edd43705de88bc3189fc5,
based on main d6b5887f55a4b7592b6905f4415ec88b544a0191 plus this batch.
The verifier checks its selected source set, not the full dependency or loaded
library closure.

## Relocated producer qualification

A fresh independent copy of the exact R4 source, excluding only root .git,
received both published patches through apply_patch. All intended postimages
were checked before application and actual postimages checked afterward.
No newline repair, alternative patch tool, source replacement or retry was
used. The original source remained byte-identical.

The three checked selected-source stages contain 56, 58 and 60 files,
respectively: 2,135,571, 2,148,853 and 2,162,093 bytes. The V2-only static source
cap is 2,112 KiB (2,162,688 bytes), including all six added sink/context files;
595 bytes remain. This is an explicit selected-source envelope, not a native
runtime limit increase or a claim of full-checkout verification. V1's source
cap and all runtime limits are unchanged.

Fourteen metadata and 20 actual-source placement controls passed. Strict
C++17 builds and executions passed 16 formatter, 12 output and four resource
fixture groups. The API header was checked, not dynamically loaded. The
unchanged V1 verifier first accepted the copied baseline and then refused the
final V2 source; a broken baseline cannot masquerade as compatibility refusal.

The completed 16-child report is 57,366 bytes, SHA-256
968ad4f97d4678786f82de884203d2ed9d191399162c6719fb2fb82da4f64500.
Its completed outer receipt is 196,967 bytes, SHA-256
e759617b8fea9499892d79977bf08df4e2fe765515913a0980a29a86ff68f7f4.
The compiler source census matches the controller gate above. Final external
source census: 41,113 files / 386,558,761 bytes /
c92ce460b4d94711fdad4ed6aea48642ac81abbd238dcff6fd3fdf49bedda4e5
(using the retained directory/file/hash census convention).

This package gate built only the three CPU fixtures, not the debugger.
Direct-child close and both stream EOFs were observed; whole-family cleanup
and a complete system compiler/header/library closure are not claimed.

Publication formatting removes one trailing blank line from tests/output.cc;
the frozen donor and preceding receipt keep their original bytes. This changes
no production patch or source-stage digest. Unified patch context remains
byte-exact data, with whitespace attributes limited to the three new patch files.

## Distinct private R9 full build

The corrected private R9 source separately passed a full debugger build and
53 actual-source/static controls. Its source selection flag is true, but
capture and publication are still false, so it cannot supply a physical run.
This differs from the all-false public source projection above.

The full-build receipt is 142,824 bytes, SHA-256
e1547e9df1a5351a1d023bbbad4ffe2212b8022f6c7581a39c632df0cfcf46c8.
The actual static-control receipt is 164,405 bytes, SHA-256
f9dbc14246a3b9b4d4791d8369b9fd93f5d2b0da742842ab13db0838361619c3.
The produced debugger ELF is 199,423,104 bytes, SHA-256
196d4ca6cac607e0bb0ce2fae0a53492f9cbef55bae49bd6ff0c26f4cffcc5db.
It has not been executed.

Actual DWARF layout gives an 8,704-byte native adapter and a 14,208-byte
logical reservation including checkpoint/response/scratch allocations, within
the existing 65,536-byte cap. The added API budget remains 192; the checked
publication work bound is 129,240 of 131,072. Layout and these static bounds
are not measured live memory consumption or hardware correctness.

## Next live gate

A final enabled private source needs its own fresh build/static evidence,
new exact-build startup decoder, fresh MI2 no-inferior startup and independent
loaded-file review. The current old-build startup record cannot transfer to
the new ELF. A distinct V2 family-observation reader must preserve the same
owned parent/generation, transcript, EOF, reaping and cleanup relations.

Only then can a source-bound controller profile join the actual debugger,
target and family. Trap/CWSR/TTMP, sampling exclusion, attached-before-runtime
state and the isolated target's lifetime premises require separate current
evidence. None was established by the CPU/package/build gates above.
A typed unavailable outcome is useful diagnostics, not successful physical
byte capture. Hardware observation, general register access and protected
kernel publication remain unqualified.
