use std::os::fd::OwnedFd;

pub(crate) fn await_test_control(control: &OwnedFd, expected: u8) {
    let mut descriptors = [rustix::event::PollFd::new(
        control,
        rustix::event::PollFlags::IN,
    )];
    let timeout = rustix::event::Timespec {
        tv_sec: 20,
        tv_nsec: 0,
    };
    assert_eq!(
        rustix::event::poll(&mut descriptors, Some(&timeout)).unwrap(),
        1
    );
    assert_eq!(descriptors[0].revents(), rustix::event::PollFlags::IN);
    let mut message = [0; 2];
    assert_eq!(rustix::io::read(control, &mut message).unwrap(), 1);
    assert_eq!(message[0], expected);
}
