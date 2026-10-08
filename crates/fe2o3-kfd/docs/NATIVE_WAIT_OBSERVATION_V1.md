# Native Wait Observation

`engineering-native-wait-diagnostics` is a default-off engineering build
feature. It emits one `Fe2o3NativeWaitObservationV1` JSON record on stderr after
each native token-program execution returns, including failed executions.
It does not change the wire protocol, 50 microsecond requested sleep, signal
acquire operations, currentness checks, retirement checks or failure poisoning.
Feature-off builds perform no new diagnostic clock reads or output.

`execution_return_ns` is sampled before JSON serialization and stderr output;
it excludes that diagnostic output overhead.

All timestamps are CPU monotonic nanoseconds relative to native execution entry,
before staging/publication. They are not GPU timestamps, calibrated shader time,
or completion/resource-release authority. Instrumented samples must never be
pooled with uninstrumented latency samples.

- `last_pending_read_ns` and `first_completed_read_ns` bracket the respective
  acquire operations. The first completed window is frozen.
- `completion_bracket_ns` bounds the raw signal transition using the last
  pending read's start and first completed read's end. Without a pending read,
  the lower bound is execution entry, not publication return. A signal may
  complete before publication returns.
- `max_inter_read_ns` includes both neighboring read intervals. It bounds
  observation uncertainty; it is not a measured GPU idle interval.
- `pause_elapsed_ns` measures actual CPU sleep calls, including oversleep.
- `post_read_ns` includes queue/exception validation and currentness checking.
  `currentness_nested_ns` is a subset and must not be added again.
- `poll_ready_ns` is the return of successful poll validation, not a separate
  ring-consumption frontier.
- `retirement_signals_ns` measures validation of all retained signals, not
  allocation release or the entire return path.

Only records with both `execution_succeeded` and `observation_valid` true may
be used for successful-execution attribution. Failed executions retain partial
observations; an error can leave timing buckets unfinished. Missing records or
unfinished buckets must not be interpreted as zero-cost work. No observation
can replace the existing completion or resource-retirement checks.

Existing Ferric native latency/counter harnesses intentionally reject this
stderr stream. Use a separately bounded diagnostic capture and bind its source,
worker ELF, exact command roster and lifecycle. A fixed128/128 request has131
native program executions; three singleton head dispatches are outside this
instrumentation.

The deterministic accumulator fixtures and serializer smoke test can be tested
without a GPU. They do not qualify the complete worker, device timing, or any
performance improvement.
