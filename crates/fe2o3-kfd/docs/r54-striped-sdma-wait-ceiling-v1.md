# R54 gfx942 striped-SDMA wait ceiling

R54 changes only the host pause schedule used by the blocking striped-tail
wait. The first 64 pauses still spin and the next 16 still yield. Later pauses
request sleeps of at most 25 us rather than following the shared default
exponential backoff to a 1 ms request ceiling. The operating system may wake
the thread later than the requested duration.

## Basis

An R53 engineering screen on MI300X GPU 2 at commit
`add91030ea5defc40bb8315010d8999543e7746b` ran a 1 MiB, depth-112,
16-logical-queue striped copy with 10 warmups and 30 samples. Every measured
H2D and D2H wait used 64 spin pauses, 16 yield pauses, and 7 sleep pauses that
requested 2.575 ms in total. Median tail-scan time was 3.047 ms H2D and
3.042 ms D2H, while the median final audit was 4.236 us and 4.066 us.
All 16 tails first appeared ready in the same final scan round.

The host had unrelated CPU activity, so that screen is diagnostic rather than
qualification evidence. It identifies coarse host sleep cadence as a concrete
latency suspect; it does not establish the device completion instant or prove
causality.

## Preserved boundaries

The optimized path retains the same first observation, shared monotonic
deadline, exact bound tail roster, timeout behavior, final full ordered audit,
currentness checks, retirement authorization, panic handling, and move-only
custody. A shorter pause ceiling can increase host wakeups. It cannot turn a
pending observation into completion evidence or extend the caller's deadline.

## Claim limits

This source change alone is not a parity or speedup result. Hardware
qualification must compare the exact committed product against matched HIP and
HSA workloads. Latency evidence must be accompanied by a separate host CPU-cost
measurement before making a net performance claim; pause counts and requested
sleep duration do not measure scheduler wake latency or host CPU consumption.
