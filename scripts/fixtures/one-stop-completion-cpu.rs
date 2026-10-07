//! CPU-only exact-source polling regression with a mocked backend and clock.
//! Does not allocate native resources, construct an owner, or execute a GPU.
#![allow(dead_code)]
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum E { Phase, ProcessChanged, Contract(&'static str), Native(String) }
mod fe2o3_aql { include!("extracted/constants.rs"); }
mod resources {
    use crate::{E, fe2o3_aql};
    include!("extracted/completion.rs");
    include!("extracted/completion_poll.rs");
}

#[derive(Clone, Copy)]
struct Sample { counters: (u64,u64), signal: (i64,i64), exception: i64, after: Option<u64> }
const LAG: Sample = Sample { counters:(1,0), signal:(1,0), exception:0, after:None };
const DONE: Sample = Sample { counters:(1,1), ..LAG };
struct State { now:u64, rows:Vec<Sample>, index:usize, calls:Vec<&'static str>, sleeps:usize, current_failure:bool }
impl Default for State {
    fn default()->Self { Self { now:0,rows:vec![LAG],index:0,calls:Vec::new(),sleeps:0,current_failure:false } }
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn reset(rows:Vec<Sample>) { STATE.with(|s| *s.borrow_mut()=State { rows, ..State::default() }); }
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct ClockInstant(u64);
impl ClockInstant { fn now()->Self { STATE.with(|s| Self(s.borrow().now)) } }
struct Ring(u64);
impl Ring { fn write(&self)->u64 { self.0 } }
struct Allocation { mapping: () }
struct Context { queue_epoch:u64,ring:Ring,completed_write:u64,last_observed_read:u64,internal:[Allocation;2] }
struct Resources { context:Context }
impl Resources { fn get_mut(&mut self)->&mut Self { self } }
struct Cold { resources:Resources }
struct Inner { cold:Cold }
fn base()->Inner { Inner { cold:Cold { resources:Resources { context:Context {
    queue_epoch:0,ring:Ring(1),completed_write:0,last_observed_read:0,
    internal:[Allocation{mapping:()},Allocation{mapping:()}]
} } } } }
struct Backend;
impl Backend {
    fn observe_completion_signal_state_acquire(_: &mut (), _:usize, _:u32)->Result<(i64,i64), &'static str> {
        STATE.with(|s| { let mut s=s.borrow_mut();s.calls.push("signal");Ok(s.rows[s.index.min(s.rows.len()-1)].signal) })
    }
    fn observe_aql_counters(_: &mut (), _:usize)->Result<(u64,u64), &'static str> {
        STATE.with(|s| { let mut s=s.borrow_mut();s.calls.push("counters");Ok(s.rows[s.index.min(s.rows.len()-1)].counters) })
    }
    fn observe_i64_acquire(_: &mut (), _:usize, _:usize)->Result<i64, &'static str> {
        STATE.with(|s| { let mut s=s.borrow_mut();s.calls.push("exception");let r=s.rows[s.index.min(s.rows.len()-1)];s.index+=1;if let Some(n)=r.after{s.now=n;}Ok(r.exception) })
    }
}
mod native {
    use crate::{Backend, ClockInstant as Instant, E, Inner, STATE, resources};
    use ::std::time::Duration;
    const CONTROL:usize=0;
    const SIGNAL:usize=1;
    const PAGE_BYTES:usize=4096;
    // Only external dependencies are mocked. Extracted source text is unchanged.
    mod std { pub mod thread {
        pub fn sleep(d: ::std::time::Duration) {
            assert_eq!(d,::std::time::Duration::from_micros(50));
            crate::STATE.with(|s| { let mut s=s.borrow_mut();s.now+=50;s.sleeps+=1; });
        }
    } }
    fn current(_: &mut Inner)->Result<(),E> {
        STATE.with(|s| { let mut s=s.borrow_mut();s.calls.push("current");if s.current_failure{Err(E::ProcessChanged)}else{Ok(())} })
    }
    include!("extracted/deadline.rs");
    pub(super) fn poll(base: &mut Inner, completed: &mut bool, deadline: &mut Option<Instant>)->Result<(),E> {
        include!("extracted/observe-expression.rs");
        Ok(())
    }
}
fn run(rows:Vec<Sample>,end:u64)->(Result<(),E>,Inner,bool) {
    reset(rows);let mut b=base();let mut done=false;let result=native::poll(&mut b,&mut done,&mut Some(ClockInstant(end)));(result,b,done)
}
fn uncompleted(b:&Inner,done:bool) { assert!(!done);assert_eq!(b.cold.resources.context.completed_write,0);assert_eq!(b.cold.resources.context.last_observed_read,0); }

include!("public-tests.rs");

#[test]
fn extracted_loop_lag_then_frontier_one_completes_once() {
    let (r,b,done)=run(vec![LAG,DONE],1000);assert_eq!(r,Ok(()));assert!(done);
    assert_eq!(b.cold.resources.context.completed_write,1);assert_eq!(b.cold.resources.context.last_observed_read,1);
    STATE.with(|s| {let s=s.borrow();assert_eq!(s.sleeps,1);assert_eq!(s.index,2);assert_eq!(s.calls,["current","signal","counters","exception","current","signal","counters","exception"]);});
}
#[test]
fn extracted_loop_forever_lag_stops_at_original_deadline() {
    let (r,b,done)=run(vec![LAG],150);assert_eq!(r,Err(E::Contract("one-stop original deadline")));uncompleted(&b,done);
    STATE.with(|s| {let s=s.borrow();assert_eq!(s.now,150);assert_eq!(s.sleeps,3);assert_eq!(s.index,3);});
}
#[test]
fn extracted_loop_positive_at_original_deadline_still_refuses() {
    let (r,b,done)=run(vec![Sample{after:Some(150),..DONE}],150);
    assert_eq!(r,Err(E::Contract("one-stop original deadline")));uncompleted(&b,done);
}
#[test]
fn extracted_loop_expired_before_read_has_no_backend_calls() {
    let (r,b,done)=run(vec![DONE],0);assert_eq!(r,Err(E::Contract("one-stop original deadline")));uncompleted(&b,done);
    STATE.with(|s|assert!(s.borrow().calls.is_empty()));
}
#[test]
fn extracted_loop_invalid_observations_keep_strict_error() {
    for bad in [Sample{counters:(0,0),..LAG},Sample{counters:(2,1),..LAG},Sample{counters:(1,2),..LAG},Sample{signal:(0,0),..LAG},Sample{signal:(1,-1),..LAG},Sample{signal:(1,2),..LAG},Sample{exception:1,..LAG}] {
        let expected=resources::completion(bad.counters,bad.signal,bad.exception).unwrap_err();let(r,b,done)=run(vec![bad],150);assert_eq!(r,Err(expected));uncompleted(&b,done);STATE.with(|s|assert_eq!(s.borrow().sleeps,0));
    }
}
#[test]
fn extracted_loop_does_not_reenter_completed_phase() {
    reset(vec![DONE]);let mut b=base();let mut done=true;assert_eq!(native::poll(&mut b,&mut done,&mut Some(ClockInstant(150))),Err(E::Phase));STATE.with(|s|assert!(s.borrow().calls.is_empty()));
}
#[test]
fn extracted_loop_owner_frontier_refuses_before_observation() {
    reset(vec![DONE]);let mut b=base();b.cold.resources.context.queue_epoch=1;let mut done=false;
    assert_eq!(native::poll(&mut b,&mut done,&mut Some(ClockInstant(150))),Err(E::Contract("one-stop in-flight owner frontier")));uncompleted(&b,done);STATE.with(|s|assert_eq!(s.borrow().calls,["current"]));
}
#[test]
fn extracted_loop_currentness_refusal_is_not_polled() {
    reset(vec![DONE]);STATE.with(|s|s.borrow_mut().current_failure=true);let mut b=base();let mut done=false;
    assert_eq!(native::poll(&mut b,&mut done,&mut Some(ClockInstant(150))),Err(E::ProcessChanged));uncompleted(&b,done);STATE.with(|s|assert_eq!(s.borrow().calls,["current"]));
}
