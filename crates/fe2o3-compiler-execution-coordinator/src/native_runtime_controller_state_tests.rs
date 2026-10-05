use super::*;
use std::{cell::RefCell, rc::Rc};

fn task(pid: i32, birth: u64) -> TaskKey {
    TaskKey { pid, birth }
}

struct Image(u8, Rc<RefCell<Vec<u8>>>);
impl Drop for Image {
    fn drop(&mut self) {
        self.1.borrow_mut().push(self.0);
    }
}

#[test]
fn inherited_kernel_image_survives_parent_exec_and_terminal_until_last_child() {
    let dropped = Rc::new(RefCell::new(Vec::new()));
    let root = task(10, 1);
    let child = task(11, 3);
    let mut images = TaskImages::new(root);
    assert!(images.inherit(root, child).is_err());
    images
        .replace_exec(root, Image(1, dropped.clone()))
        .unwrap();
    images.inherit(root, child).unwrap();
    images
        .replace_exec(root, Image(2, dropped.clone()))
        .unwrap();
    assert!(dropped.borrow().is_empty());
    assert_eq!(images.image(child).unwrap().0, 1);
    assert_eq!(images.image(root).unwrap().0, 2);
    images.remove_terminal(root).unwrap();
    assert_eq!(&*dropped.borrow(), &[2]);
    images.remove_terminal(child).unwrap();
    assert_eq!(&*dropped.borrow(), &[2, 1]);
    assert_eq!(images.count(), 0);
}

#[test]
fn task_pid_reuse_requires_old_terminal_and_new_birth_generation() {
    let root = task(1, 1);
    let old = task(2, 3);
    let new = task(2, 9);
    let mut images = TaskImages::new(root);
    images.replace_exec(root, 1).unwrap();
    images.inherit(root, old).unwrap();
    assert!(images.inherit(root, new).is_err());
    assert!(images.remove_terminal(new).is_err());
    images.remove_terminal(old).unwrap();
    images.inherit(root, new).unwrap();
    assert!(images.image(old).is_err());
    assert!(images.remove_terminal(old).is_err());
    assert_eq!(images.image(new), Ok(&1));
    assert!(images.inherit(root, task(3, 1)).is_err());
}

#[test]
fn fixed_image_roster_supports_all_distinct_execs_and_refuses_overflow() {
    let root = task(1, 1);
    let mut images = TaskImages::new(root);
    images.replace_exec(root, 1).unwrap();
    for index in 1..CAPACITY {
        images
            .inherit(root, task(index as i32 + 1, index as u64 + 2))
            .unwrap();
    }
    assert_eq!(images.count(), CAPACITY);
    assert!(images.inherit(root, task(999, 999)).is_err());
    for index in 1..CAPACITY {
        let key = task(index as i32 + 1, index as u64 + 2);
        images.replace_exec(key, index + 1).unwrap();
        assert_eq!(images.image(key), Ok(&(index + 1)));
    }
    images.replace_exec(root, 100).unwrap();
    assert_eq!(images.image(root), Ok(&100));
    for index in 1..CAPACITY {
        images
            .remove_terminal(task(index as i32 + 1, index as u64 + 2))
            .unwrap();
    }
    assert_eq!(images.count(), 1);
}

#[test]
fn syscall_exit_binding_rejects_reused_pid_other_task_and_stale_observation() {
    let key = task(1, 2);
    require_exit_binding(key, key, 3, 4).unwrap();
    for (actual, generation) in [(task(2, 2), 4), (task(1, 3), 4), (key, 3), (key, 2)] {
        assert!(require_exit_binding(key, actual, 3, generation).is_err());
    }
}

#[test]
fn checkpoint_dispatch_is_closed_and_open_confinement_cannot_be_bypassed() {
    for number in [9, 10, 25, 216, 329] {
        assert_eq!(entry_kind(number, [0; 6], 1), Ok(EntryKind::Memory));
    }
    for number in [16, 47, 299] {
        assert_eq!(entry_kind(number, [0; 6], 1), Ok(EntryKind::Descriptor));
    }
    for number in [2, 257] {
        assert_eq!(entry_kind(number, [0; 6], 1), Ok(EntryKind::Open));
    }
    for number in [58, 435, 317, 437, 438, 999, u64::MAX] {
        assert!(entry_kind(number, [0; 6], 1).is_err());
    }
    assert_eq!(entry_kind(60, [0; 6], 32), Ok(EntryKind::Exit));
    assert_eq!(entry_kind(231, [0; 6], 1), Ok(EntryKind::Exit));
    assert!(entry_kind(231, [0; 6], 2).is_err());
    assert_eq!(
        entry_kind(157, [15, 0x1000, 0, 0, 0, 0], 1),
        Ok(EntryKind::ThreadName)
    );
    for arguments in [[15, 0, 0, 0, 0, 0], [22, 1, 0, 0, 0, 0], [4, 1, 0, 0, 0, 0]] {
        assert!(entry_kind(157, arguments, 1).is_err());
    }
}

#[test]
fn clone_policy_accepts_native_threads_and_fork_but_refuses_unowned_effects() {
    for flags in [17, 0x1200011, 0x3d0f00] {
        assert_eq!(
            entry_kind(56, [flags, 0, 0, 0, 0, 0], 1),
            Ok(EntryKind::Birth)
        );
    }
    for flags in [
        0x800011,
        0x4011,
        0x8011,
        0x1011,
        0x1000_0011,
        0x4000_0011,
        1 << 32,
        0x10011,
        0x10000,
        9,
        u64::MAX,
    ] {
        assert!(
            entry_kind(56, [flags, 0, 0, 0, 0, 0], 1).is_err(),
            "{flags:x}"
        );
    }
    assert_eq!(entry_kind(57, [0; 6], 1), Ok(EntryKind::Birth));
}

#[test]
fn held_root_exit_requires_same_task_epoch_every_argument_and_sole_remaining_task() {
    let original = HeldRootExit {
        task: task(41, 3),
        generation: 7,
        number: 231,
        arguments: [0, 2, 3, 4, 5, 6],
    };
    original.validate(original, 1).unwrap();
    for tasks in [0, 2, CAPACITY] {
        assert!(original.validate(original, tasks).is_err());
    }
    for changed in [
        HeldRootExit {
            task: task(42, 3),
            ..original
        },
        HeldRootExit {
            task: task(41, 4),
            ..original
        },
        HeldRootExit {
            generation: 6,
            ..original
        },
        HeldRootExit {
            generation: 8,
            ..original
        },
        HeldRootExit {
            number: 60,
            ..original
        },
    ] {
        assert!(original.validate(changed, 1).is_err());
    }
    for index in 0..6 {
        let mut changed = original;
        changed.arguments[index] ^= 1;
        assert!(original.validate(changed, 1).is_err());
    }
}

#[test]
fn held_root_exit_is_not_a_generic_checkpoint_or_stale_identity_match() {
    for number in [60, 231] {
        let original = HeldRootExit {
            task: task(7, 9),
            generation: 9,
            number,
            arguments: [0; 6],
        };
        original.validate(original, 1).unwrap();
        let stale = HeldRootExit {
            generation: 8,
            ..original
        };
        assert!(stale.validate(stale, 1).is_err());
    }
    for number in [0, 2, 9, 56, 257, u64::MAX] {
        let other = HeldRootExit {
            task: task(7, 9),
            generation: 9,
            number,
            arguments: [0; 6],
        };
        assert!(other.validate(other, 1).is_err());
    }
}
