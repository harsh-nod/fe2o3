# GFX950 publication diagnostic: retained native failure (2026-09-26)

This is a failed, single-attempt debugger observation, not physical-capture
qualification. The bounded diagnostic added in `e934c1437457efc01c0c311e73026e862943041a`
was exercised on `mi350`. It preserved information that the earlier publication
deadline had hidden. Public debugger capability and the accepted broad milestone
count remain unchanged: **6/18**.

## Observed result

The outer command returned code 2 with no signal after 96,018 ms; the root wrapper
finished its own audit after about 90,984 ms. Controller stdout is empty.
Controller stderr contains:

- publication refusal `Deadline`;
- original protocol result `refused(Incomplete)`;
- retained transport counters: 14 commands consumed, 73 records, both streams at
  EOF and transport closed after cleanup;
- a 256-byte retained stdout suffix ending in the debugger MI log-stream message
  `fixed owned one-stop native relation refused (15)`;
- a 256-byte command suffix containing token 13 selecting the owned one-stop
  process and token 14 requesting continue.

These are bounded retained bytes, not a parsed full transcript. Command consumption
does not establish successful command completion. The suffixes do not prove which
source-level guard or protocol step caused the refusal, what every prior MI record contained, or whether a GPU
dispatch occurred. The stderr warning about the index-cache directory is retained
but is not established as the cause.

The outer family result rejected the empty controller report as `frame cap/LF`.
That is a downstream report failure, not proof that the report size limit should
be increased. No deadline, report cap, cleanup requirement, or acceptance check
was loosened. There was no automatic retry.

## Cleanup is separately established

The exact request, manager generation, inner receipt, owner receipt and terminal
acknowledgement joined successfully. Inner and outer cleanup completed; streams
reached EOF, the controller was reaped, and the owned cgroup was absent. The
controller reported that it did not itself prove cleanup of an unadmitted
inferior; that is not overwritten by the outer-family observation.

An independent root read-only check at `2026-09-26T11:43:34.598Z` found all five
recorded process IDs absent: `3908557`, `3908573`, `3910068`, `3910082`,
and `3910098`. The exact owned scope was absent too. These observations establish
cleanup of the selected attempt, not a machine-wide isolation or GPU rollback
claim. Physical capture and dispatch remain unknown; no successful capture is
counted.

## Exact retained evidence

Paths are relative to the retained MI350 task root
`/home/harmenon/fe2o3-authoring-280-282-mi350.4VZ42zNr`.

| Evidence | Bytes | SHA-256 |
| --- | ---: | --- |
| R23 request `site-final-r10-publication-diagnostic-one-stop-native-r1.json` | 79681 | `ce07a6811c2ca8bfde9cd48cdefe4176dccf1f901c864532362aeede61ff9997` |
| `logs/phase28-resume-r23-site-final-r10-publication-diagnostic-one-stop-native-r1/receipt.json` | 272124 | `81e881850e7843dc83ef04b530317c8fd4369a609a991343f06c0bc62d35ba1c` |
| `phase28-final-r10-publication-diagnostic-root-r24-r1/audit.json` | 17437 | `2f21c44121a459e8e54ab54f1ec1c8f9a90ae0b6635393c83075024384317a99` |
| `phase28-final-r10-publication-diagnostic-root-r24-r1/independent-cleanup.json` | 670 | `fa609a6bdb32972f5ba695b8b364edff570b063c32cc81f91d5c055522476fea` |
| Attempt `controller.stderr` | 2206 | `dd607bf9ca059ea92e633d563c6c48b5ab197a5bddf91737b05eb068ff00b121` |
| Attempt `controller.stdout` | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

The attempt directory is
`phase28-gfx950-one-stop-publication-diagnostic-scope-r24-r1/one-stop-45cf0c05e8483e6755d773668aea9e0a`.
The request lives in `phase28-resume-r23-runner/requests/`.
The failed runner intentionally has no successful post-command source/input
qualification; the wrapper completed its two selected-input identity/content
sweeps, and root independently rehashed the retained products. These are different
checks and are not interchangeable.

## Preconditions and limits

Root reviewed the running driver/hardware assumptions, rechecked the pinned
module and topology, confirmed relevant task processes were quiet, and confirmed
lock availability before creating fresh finite coordination. The wrapper retained
the same exclusive native lock FD 9 throughout. The coordination was one-attempt,
not reusable authorization.

The wrapper selected 276 fixed inputs and charged 449,235,518 of 536,870,912
read bytes and 7,925 of 40,000 read calls. Its original command bound remained
300,000 ms. Independent CPU/build/read-only replay receipts qualified the private
diagnostic deployment before this attempt; none of them constituted native
success.

Next work is to trace the retained native-relation refusal to the pinned adapter
and protocol path, add narrowly justified controls, and qualify any proposed fix.
The historical failure, consumed coordination and all retained products remain
unchanged. A later native attempt needs its own reviewed sources, inputs,
quiescence, coordination and one-attempt request.
