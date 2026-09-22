# Qualified Begin Correspondence

Source: `4b0fc4d56c848b266ae7fbd7af3817a2ddf89755`.

- Verus: two whole-root runs, each 413 verified and zero errors at default solver limits.
- Twenty-four scoped controls: six projections, ten executable omissions/mutations, six contract sensitivities and two live-fixture sensitivities.
- CPU: 878 unit tests and 27 doctests passed; ten ignored tests in the normal run.
- Formatting, all-target Clippy with warnings denied and a release test build passed.
- No performance benchmarks, native GPU execution or HIP/HSA measurements were run.

The raw paired harness requires only represented pre-state and corresponding writer/roster
inputs. Exact mapped decisions and represented post-state follow from independently verified
actual-type and historical raw Begin executions. No successful-admission, canonicality, custody,
clean-scratch, distinct-member-slot or post-representation premise is added. Each execution
contract supplies its own exact rejection identity; equal Vec views do not establish it.

The guarded issued harness is deliberately hybrid: historical combined/stable unread guards
execute first. Rejection skips actual Begin and returns None for its optional result; passing
guards execute actual Begin and return Some(result), including Some(Err(...)). The paired
historical issued wrapper preserves its issued invariant and reader/reservation storage.
This is not execution correspondence for the actual reader guards or public wrapper.

Concrete witnesses cover distinct and aliased free slots, captured epochs, dirty scratch tails,
noncanonical precedence, replay rejection and empty-roster success outside constructor metadata.
A synthetic live Pending reservation protects allocation/member/writer slot zero while Begin
updates a distinct allocation. The witness checks reservation storage and Pending status,
then Some(Err(InvalidReference)) replay and guard-first AllocationBusy with an invalid writer.
These are explicit synthetic states, not production constructor/acquisition reachability.

The whole-root count includes inherited historical and shared-body obligations; 413 is not
a count of new obligations. Production runtime source is unchanged from 30bc6fdbd493a82bc699ef1af99adc907e99109b.
Paired execution and fixtures exist only in verification and add no runtime work.

Physical Vec capacity/address preservation, allocation failure, unwind, actual reader wrappers,
the public forwarding wrapper, universal represented-model existence and native producer
authority remain outside this packet. Native pending-consumer admission remains closed.
Full HIP/HSA behavioral/performance parity remains unproved.
