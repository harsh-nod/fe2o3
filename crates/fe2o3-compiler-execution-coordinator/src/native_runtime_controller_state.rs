//! Fixed-capacity bookkeeping only, never task acquisition or image authority.
pub(super) const CAPACITY: usize = 32;
type Result<T> = std::result::Result<T, &'static str>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct TaskKey {
    pub(super) pid: i32,
    pub(super) birth: u64,
}

#[derive(Clone, Copy)]
struct Task {
    key: TaskKey,
    image: Option<usize>,
}

pub(super) struct TaskImages<I> {
    tasks: [Option<Task>; CAPACITY],
    images: [Option<I>; CAPACITY],
}

impl<I> TaskImages<I> {
    pub(super) fn new(root: TaskKey) -> Self {
        let mut tasks = [None; CAPACITY];
        tasks[0] = Some(Task {
            key: root,
            image: None,
        });
        Self {
            tasks,
            images: std::array::from_fn(|_| None),
        }
    }

    pub(super) fn count(&self) -> usize {
        self.tasks.iter().flatten().count()
    }

    pub(super) fn key(&self, pid: i32) -> Result<TaskKey> {
        self.tasks
            .iter()
            .flatten()
            .find(|task| task.key.pid == pid)
            .map(|task| task.key)
            .ok_or("runtime task is absent from original image census")
    }

    fn task(&self, key: TaskKey) -> Result<&Task> {
        self.tasks
            .iter()
            .flatten()
            .find(|task| task.key == key)
            .ok_or("runtime task generation is absent from original image census")
    }

    pub(super) fn image(&self, key: TaskKey) -> Result<&I> {
        self.task(key)?
            .image
            .and_then(|index| self.images[index].as_ref())
            .ok_or("runtime task has no captured executable kernel image")
    }

    pub(super) fn inherit(&mut self, parent: TaskKey, child: TaskKey) -> Result<()> {
        let image = self
            .task(parent)?
            .image
            .ok_or("runtime birth parent has no captured kernel image")?;
        if child.birth <= parent.birth || self.key(child.pid).is_ok() {
            return Err("runtime birth reuses a retained identity or stale generation");
        }
        let slot = self
            .tasks
            .iter_mut()
            .find(|task| task.is_none())
            .ok_or("runtime image census exceeds fixed task capacity")?;
        *slot = Some(Task {
            key: child,
            image: Some(image),
        });
        Ok(())
    }

    pub(super) fn replace_exec(&mut self, key: TaskKey, image: I) -> Result<()> {
        let slot = self
            .tasks
            .iter()
            .position(|task| task.is_some_and(|task| task.key == key))
            .ok_or("runtime exec is not an originally retained task generation")?;
        let previous = self.tasks[slot]
            .as_mut()
            .expect("located task")
            .image
            .take();
        self.release_unused(previous);
        // At most one image per live task. Removing this task's old reference
        // guarantees a free slot even when every other task has exec'd apart.
        let index = self
            .images
            .iter()
            .position(Option::is_none)
            .ok_or("runtime image slots exceed their live-task bound")?;
        self.images[index] = Some(image);
        self.tasks[slot].as_mut().expect("located task").image = Some(index);
        Ok(())
    }

    pub(super) fn remove_terminal(&mut self, key: TaskKey) -> Result<()> {
        let slot = self
            .tasks
            .iter()
            .position(|task| task.is_some_and(|task| task.key == key))
            .ok_or("runtime terminal does not match its retained task generation")?;
        let removed = self.tasks[slot].take().expect("located task");
        self.release_unused(removed.image);
        Ok(())
    }

    fn release_unused(&mut self, index: Option<usize>) {
        if let Some(index) = index
            && !self
                .tasks
                .iter()
                .flatten()
                .any(|task| task.image == Some(index))
        {
            self.images[index] = None;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum EntryKind {
    Memory,
    Descriptor,
    Open,
    Birth,
    Exit,
    ThreadName,
}

pub(super) fn entry_kind(number: u64, arguments: [u64; 6], tasks: usize) -> Result<EntryKind> {
    match number {
        9 | 10 | 25 | 216 | 329 => Ok(EntryKind::Memory),
        16 | 47 | 299 => Ok(EntryKind::Descriptor),
        // Classification only. The controller still requires the non-forgeable
        // original Attempt receipt and live kernel confinement before stepping.
        2 | 257 => Ok(EntryKind::Open),
        56 => {
            // No namespace escape, untraced child, external parent, PIDFD import
            // or vfork dependency. Standard legacy pthread/fork flags remain.
            const ALLOWED: u64 = 0x100
                | 0x200
                | 0x400
                | 0x800
                | 0x1_0000
                | 0x4_0000
                | 0x8_0000
                | 0x10_0000
                | 0x20_0000
                | 0x100_0000
                | 0xff;
            let flags = arguments[0];
            if flags & !ALLOWED != 0
                || !matches!(flags & 0xff, 0 | 17)
                || (flags & 0x1_0000 != 0 && (flags & 0x900 != 0x900 || flags & 0xff != 0))
            {
                return Err("native clone flags exceed the supported retained lifecycle");
            }
            Ok(EntryKind::Birth)
        }
        57 => Ok(EntryKind::Birth),
        58 => Err("native vfork requires separately qualified dependent-task scheduling"),
        60 => Ok(EntryKind::Exit),
        231 if tasks == 1 => Ok(EntryKind::Exit),
        231 => Err("native exit_group with live siblings requires coordinated terminal scheduling"),
        157 if arguments[0] == 15 && arguments[1] != 0 => Ok(EntryKind::ThreadName),
        _ => Err("syscall is outside the native checkpoint policy"),
    }
}

pub(super) fn require_exit_binding(
    expected: TaskKey,
    actual: TaskKey,
    entry_generation: u64,
    exit_generation: u64,
) -> Result<()> {
    if expected == actual && exit_generation > entry_generation {
        Ok(())
    } else {
        Err("native syscall exit differs from its original task or entry generation")
    }
}

/// Inert copies only. The controller obtains both observations from its same
/// privately retained owner; this value cannot stop, select or resume a task.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct HeldRootExit {
    pub(super) task: TaskKey,
    pub(super) generation: u64,
    pub(super) number: u64,
    pub(super) arguments: [u64; 6],
}

impl HeldRootExit {
    pub(super) fn validate(&self, actual: Self, tasks: usize) -> Result<()> {
        if tasks == 1
            && matches!(self.number, 60 | 231)
            && self.generation >= self.task.birth
            && *self == actual
        {
            Ok(())
        } else {
            Err("original held root exit task, generation or registers changed")
        }
    }
}

#[cfg(test)]
#[path = "native_runtime_controller_state_tests.rs"]
mod tests;
