//! Inert send-gated transport: future-command responses do not exist until send.
//! No GDB/process/queue or claimed native observation is involved.
use super::*;

fn gpu_stop() -> Vec<u8> {
    b(
        "*stopped,reason=\"signal-received\",signal-name=\"SIGTRAP\",signal-meaning=\"Trace/breakpoint trap\",frame={addr=\"0x200050\",func=\"fe2o3_gfx950_one_stop_fixture\",args=[]},thread-id=\"2\",lane-id=\"0\",stopped-threads=\"all\"\n",
    )
}
fn batches() -> Vec<Vec<Vec<u8>>> {
    let transcript = script();
    let mut starts = vec![0];
    for token in 1..=17 {
        let first = match token {
            12 => b("=thread-group-started,id=\"i1\",pid=\"4242\"\n"),
            13 => row(1, 1),
            15 => row(8, 5),
            16 => row(12, 9),
            _ => b(&format!("{token}^")),
        };
        let index = transcript
            .iter()
            .position(|x| x.starts_with(&first))
            .unwrap();
        assert!(index > *starts.last().unwrap());
        starts.push(index);
    }
    starts.push(transcript.len());
    starts
        .windows(2)
        .map(|r| transcript[r[0]..r[1]].to_vec())
        .collect()
}
struct Gated {
    peer: Fake,
    unreleased: VecDeque<Vec<Vec<u8>>>,
}
impl Gated {
    fn new(batches: Vec<Vec<Vec<u8>>>) -> Self {
        assert_eq!(batches.len(), 18);
        let mut batches: VecDeque<_> = batches.into();
        Self {
            peer: Fake::new(batches.pop_front().unwrap()),
            unreleased: batches,
        }
    }
}
impl Peer for Gated {
    fn send(&mut self, t: u64, c: &str) -> Result<(), Refusal> {
        self.peer.send(t, c)?;
        self.peer
            .lines
            .extend(self.unreleased.pop_front().ok_or(Refusal::State)?);
        Ok(())
    }
    fn next(&mut self) -> Result<Option<Vec<u8>>, Refusal> {
        self.peer.next()
    }
    fn observe_child(&mut self, p: u32) -> Result<(), Refusal> {
        self.peer.observe_child(p)
    }
    fn entry(&mut self) -> Result<(), Refusal> {
        self.peer.entry()
    }
    fn current(&mut self) -> Result<(), Refusal> {
        self.peer.current()
    }
    fn elapsed_ns(&mut self) -> Result<u64, Refusal> {
        self.peer.elapsed_ns()
    }
    fn finish(&mut self) -> Result<Cleanup, Refusal> {
        if !self.unreleased.is_empty() {
            return Err(Refusal::Incomplete);
        }
        self.peer.finish()
    }
    fn cleanup(&mut self) -> Cleanup {
        self.peer.cleanup()
    }
}
fn positive(input: Vec<Vec<Vec<u8>>>) -> Gated {
    let mut p = Gated::new(input);
    let r = observe(&mut p).unwrap();
    assert_eq!(p.peer.sent, expected_commands());
    assert_eq!((p.peer.finish, p.peer.cleanup), (1, 0));
    assert!(p.peer.lines.is_empty() && p.unreleased.is_empty());
    assert_eq!(r.producer_rows, 15);
    assert!(!r.operational_qualification && !r.independent_native_tuple_replay);
    p
}
fn refuse_before_next(input: Vec<Vec<Vec<u8>>>, last: usize) {
    let mut p = Gated::new(input);
    assert!(observe(&mut p).is_err());
    assert_eq!(p.peer.sent, expected_commands()[..last]);
    assert_eq!((p.peer.finish, p.peer.cleanup), (0, 1));
    assert_eq!(p.unreleased.len(), 17 - last);
}
fn remove(batch: &mut Vec<Vec<u8>>, line: &[u8]) {
    let index = batch.iter().position(|x| x == line).unwrap();
    batch.remove(index);
}
#[test]
fn async_positive_has_only_startup_and_command_result_prompts() {
    let input = batches();
    assert_eq!(
        input
            .iter()
            .flatten()
            .filter(|x| x.as_slice() == b"(gdb)\n")
            .count(),
        17
    );
    for token in [12, 14, 15, 16] {
        assert_eq!(
            input[token]
                .iter()
                .filter(|x| x.as_slice() == b"(gdb)\n")
                .count(),
            1
        );
        assert_ne!(input[token].last().unwrap().as_slice(), b"(gdb)\n");
    }
    positive(input);
}
#[test]
fn stops_and_retirement_may_arrive_before_the_command_result_prompt() {
    positive(batches());
    for token in [12, 14, 15, 16] {
        let mut input = batches();
        remove(&mut input[token], b"(gdb)\n");
        input[token].push(b("(gdb)\n"));
        positive(input);
    }
}
#[test]
fn missing_stop_deletion_or_final_stop_phase_never_sends_next_command() {
    positive(batches());
    for (token, line) in [
        (12, host_stop(ENTRY, "1")),
        (12, b("=breakpoint-deleted,id=\"1\"\n")),
        (14, host_stop(CHECKPOINT, "-2")),
        (14, row(6, 3)),
        (14, row(7, 4)),
        (15, gpu_stop()),
        (15, row(10, 7)),
        (15, row(11, 8)),
    ] {
        let mut input = batches();
        remove(&mut input[token], &line);
        refuse_before_next(input, token);
    }
}
#[test]
fn each_terminal_fact_is_required_before_gdb_exit() {
    positive(batches());
    for line in [
        target(),
        b("=thread-exited,id=\"2\",group-id=\"i1\"\n"),
        b("=thread-exited,id=\"1\",group-id=\"i1\"\n"),
        b("=thread-group-exited,id=\"i1\",exit-code=\"0\"\n"),
        b("*stopped,reason=\"exited-normally\"\n"),
        row(14, 10),
        row(15, 11),
    ] {
        let mut input = batches();
        remove(&mut input[16], &line);
        refuse_before_next(input, 16);
    }
}
#[test]
fn startup_and_every_command_result_prompt_remain_mandatory() {
    positive(batches());
    for token in 0..=16 {
        let mut input = batches();
        remove(&mut input[token], b"(gdb)\n");
        refuse_before_next(input, token);
    }
}
#[test]
fn extra_prompts_cannot_replace_any_stop_or_terminal_witness() {
    positive(batches());
    for (token, line) in [
        (12, b("=breakpoint-deleted,id=\"1\"\n")),
        (14, row(7, 4)),
        (15, row(11, 8)),
        (16, row(15, 11)),
    ] {
        let mut input = batches();
        remove(&mut input[token], &line);
        input[token].push(b("(gdb)\n"));
        input[token].push(b("(gdb)\n"));
        refuse_before_next(input, token);
    }
}
#[test]
fn late_real_stop_observation_never_authorizes_next_continue() {
    positive(batches());
    for (token, line) in [
        (12, b("=breakpoint-deleted,id=\"1\"\n")),
        (14, row(7, 4)),
        (15, row(11, 8)),
        (16, row(15, 11)),
    ] {
        let input = batches();
        let index = input.iter().flatten().position(|x| *x == line).unwrap();
        let mut p = Gated::new(input);
        p.peer.late_read = Some(index + 1);
        assert_eq!(observe(&mut p).unwrap_err().refusal, Refusal::Deadline);
        assert_eq!(p.peer.sent, expected_commands()[..token]);
        assert_eq!((p.peer.finish, p.peer.cleanup), (0, 1));
    }
}
#[test]
fn stop_records_do_not_replace_correlated_running_results() {
    positive(batches());
    for token in [12, 14, 15, 16] {
        let mut input = batches();
        remove(&mut input[token], format!("{token}^running\n").as_bytes());
        refuse_before_next(input, token);
    }
}
