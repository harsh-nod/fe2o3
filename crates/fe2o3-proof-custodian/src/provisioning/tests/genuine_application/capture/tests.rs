use super::*;
use std::fs::File;

#[test]
fn child_exit_does_not_wait_for_another_live_writer() {
    let (read, write) = rustix::pipe::pipe().unwrap();
    let mut reader = File::from(read);
    let writer = File::from(write);
    let mut holder = Service(
        Command::new("/bin/sleep")
            .arg("30")
            .stdout(writer.try_clone().unwrap())
            .spawn()
            .unwrap(),
    );
    let mut producer = Service(
        Command::new("/bin/sh")
            .args(["-c", "printf 'split'; printf ' record\\n'"])
            .stdout(writer)
            .spawn()
            .unwrap(),
    );
    let started = Instant::now();
    let output = collect(
        &mut producer.0,
        &mut reader,
        started + Duration::from_secs(3),
        OUTPUT_LIMIT,
    )
    .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"split record\n");
    assert!(started.elapsed() < Duration::from_secs(3));
    assert!(holder.0.try_wait().unwrap().is_none());
}

#[test]
fn exact_limit_succeeds_and_one_extra_byte_rejects() {
    for (count, success) in [(OUTPUT_LIMIT, true), (OUTPUT_LIMIT + 1, false)] {
        let mut command = Command::new("/usr/bin/head");
        command.args(["-c", &count.to_string(), "/dev/zero"]);
        let output = run(&mut command, Instant::now() + Duration::from_secs(5));
        if success {
            let output = output.unwrap();
            assert!(output.status.success());
            assert_eq!(output.stdout.len(), OUTPUT_LIMIT);
        } else {
            assert!(output.err().unwrap().contains("limit exceeded"));
        }
    }
}

#[test]
fn captured_output_does_not_hide_unsuccessful_child_status() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "printf 'output before failure\\n'; exit 7"]);
    let output = run(&mut command, Instant::now() + Duration::from_secs(3)).unwrap();
    assert_eq!(output.status.code(), Some(7));
    assert_eq!(output.stdout, b"output before failure\n");
}

#[test]
fn sleeping_and_continuously_writing_children_are_bounded_and_reaped() {
    for (program, args, expected) in [
        ("/bin/sleep", vec!["30"], "deadline exceeded"),
        ("/usr/bin/yes", vec![], "limit exceeded"),
    ] {
        let mut command = Command::new(program);
        command.args(args).stdout(Stdio::piped());
        let mut child = Service(command.spawn().unwrap());
        let mut stdout = child.0.stdout.take().unwrap();
        let result = collect(
            &mut child.0,
            &mut stdout,
            Instant::now()
                + if program == "/bin/sleep" {
                    Duration::from_millis(100)
                } else {
                    Duration::from_secs(5)
                },
            OUTPUT_LIMIT,
        );
        assert!(result.err().unwrap().contains(expected));
        child.0.kill().unwrap();
        assert!(!child.0.wait().unwrap().success());
        assert!(child.0.try_wait().unwrap().is_some());
    }
}

#[test]
fn every_read_attempt_including_eintr_consumes_the_poll_budget() {
    struct Busy {
        calls: usize,
        interrupted: bool,
    }
    impl Read for Busy {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            self.calls += 1;
            if self.interrupted {
                Err(io::ErrorKind::Interrupted.into())
            } else {
                out[0] = 42;
                Ok(1)
            }
        }
    }
    for interrupted in [false, true] {
        let mut busy = Busy {
            calls: 0,
            interrupted,
        };
        let mut bytes = Vec::new();
        assert!(!drain_batch(&mut busy, &mut bytes, OUTPUT_LIMIT).unwrap());
        assert_eq!(busy.calls, READS_PER_POLL);
        assert_eq!(bytes.len(), if interrupted { 0 } else { READS_PER_POLL });
    }
}
