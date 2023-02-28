use rand::{Rng, SeedableRng};

/// The states in which a process can be.
#[derive(PartialEq, Copy, Clone, Debug)]
pub enum ProcessState {
    Ready,
    Running,
    IORequest,
    IOBlocked,
    Finished,
}

/// The possible instructions that a process can return to a scheduler.
#[derive(PartialEq, Copy, Clone)]
pub enum ProcessInstruction {
    /// The process has finished.
    Finished,
    /// The process has requested IO.
    IORequest,
    /// The process has not finished and has not requested IO.
    Continue,
}

/// An entry in the process table.
pub struct SchedulerProcessEntry {
    /// The remaining progress of the process (number of time slices it needs to finish).
    remaining_progress: i32,
    /// The state of the process.
    pub state: ProcessState,
    /// The probability that the process will request IO after finishing a time slice.
    io_probability: f32,
    /// The CPU time that the process has used
    pub time: i32,
}

impl SchedulerProcessEntry {
    /// Creates a new process entry.
    fn new(length: i32, io_probability: f32) -> Self {
        Self {
            remaining_progress: length,
            state: ProcessState::Ready,
            io_probability,
            time: 0,
        }
    }

    /// Tick the process. This should be called once per time slice.
    /// Returns the instruction that the scheduler should execute.
    /// It needs a reference to a rng, because it needs to generate a random number
    /// to determine whether the process requests IO.
    fn tick(&mut self, rng: &mut impl Rng) -> ProcessInstruction {
        match self.state {
            ProcessState::Running => {
                self.remaining_progress -= 1;
                self.time += 1;
                if self.remaining_progress <= 0 {
                    self.state = ProcessState::Finished;
                    ProcessInstruction::Finished
                } else if rng.gen::<f32>() < self.io_probability {
                    self.state = ProcessState::IORequest;
                    ProcessInstruction::IORequest
                } else {
                    ProcessInstruction::Continue
                }
            }
            _ => {
                panic!("Tried to tick a process that was not running.");
            }
        }
    }
}

#[derive(PartialEq, Copy, Clone, Debug)]
pub enum IODeviceState {
    Idle,
    Busy,
}

// Implements an IO device.
pub struct IODevice {
    /// The state of the IO device.
    pub state: IODeviceState,
    /// The number of time slices the IO device needs to finish.
    pub remaining_progress: i32,
}

impl IODevice {
    /// Creates a new IO device.
    fn new() -> Self {
        Self {
            state: IODeviceState::Idle,
            remaining_progress: 0,
        }
    }

    /// Ticks the IO device. This should be called once per time slice.
    /// Returns the old and new state of the IO device.
    fn tick(&mut self) -> (IODeviceState, IODeviceState) {
        let old_state = self.state;
        if self.state == IODeviceState::Busy {
            self.remaining_progress -= 1;
            if self.remaining_progress <= 0 {
                self.state = IODeviceState::Idle;
            }
        }
        (old_state, self.state)
    }

    /// Starts the IO device. This should be called when a process requests IO.
    fn start(&mut self, length: i32) {
        self.state = IODeviceState::Busy;
        self.remaining_progress = length;
    }
}

/// Possible outcomes of a scheduler tick.
#[derive(PartialEq, Copy, Clone, Debug)]
pub enum SchedulerTickOutcome {
    /// An IO operation has finished, and the process that requested it is now ready.
    IOFinished(usize),
    /// An IO request was handled, and the process that requested it is now IOBlocked.
    IORequestHandled(usize),
    /// A process has started running.
    ProcessResumed(usize),
    /// A process has continued running.
    ProcessContinued(usize),
    /// A process has finished.
    ProcessFinished(usize),
    /// There is nothing to do.
    Idle,
}

/// Trait for a scheduler. A scheduler is responsible for scheduling processes.
pub trait Scheduler {
    /// Registers a new process with the scheduler.
    fn register_process(&mut self, length: i32, io_probability: f32);
    /// Ticks the scheduler. This should be called once per time slice (or frame,
    /// when using a graphical interface).
    fn tick(&mut self) -> SchedulerTickOutcome;
    /// Create a new scheduler.
    fn new() -> Self;
}

/// Implements the first-come-first-serve scheduling algorithm.
/// This is the simplest scheduling algorithm. It finds a READY process and runs it
/// until it finishes or requests IO. If a process requests IO, it is moved to the
/// IOBlocked state, and the scheduler will try to find another process to run in
/// the meantime.
pub struct FirstComeFirstServeScheduler {
    /// The process table.
    pub processes: Vec<SchedulerProcessEntry>,
    /// The IO device.
    pub io_device: IODevice,
    /// The rng used to determine whether a process requests IO.
    rng: rand::rngs::StdRng,
}

impl Scheduler for FirstComeFirstServeScheduler {
    fn register_process(&mut self, length: i32, io_probability: f32) {
        self.processes
            .push(SchedulerProcessEntry::new(length, io_probability));
    }

    fn tick(&mut self) -> SchedulerTickOutcome {
        // Tick the IO device.
        let (old_io_device_state, new_io_device_state) = self.io_device.tick();

        // If the IO device was busy and is now idle, we need to find the process
        // that is IOBlocked and move it to the Ready state.
        if old_io_device_state == IODeviceState::Busy
            && new_io_device_state == IODeviceState::Idle
        {
            for (i, process) in &mut self.processes.iter_mut().enumerate() {
                if process.state == ProcessState::IOBlocked {
                    process.state = ProcessState::Ready;
                    return SchedulerTickOutcome::IOFinished(i);
                }
            }
        }

        if new_io_device_state == IODeviceState::Idle {
            // Now we find a process that has an IORequest, start the IO device and
            // move the process to the IOBlocked state.
            for (i, process) in &mut self.processes.iter_mut().enumerate() {
                if process.state == ProcessState::IORequest {
                    self.io_device.start(10);
                    process.state = ProcessState::IOBlocked;
                    return SchedulerTickOutcome::IORequestHandled(i);
                }
            }
        }

        // Now we find a process that is Running and tick it.
        for (i, process) in &mut self.processes.iter_mut().enumerate() {
            if process.state == ProcessState::Running {
                let instruction = process.tick(&mut self.rng);
                match instruction {
                    ProcessInstruction::Finished => {
                        process.state = ProcessState::Finished;
                        return SchedulerTickOutcome::ProcessFinished(i);
                    }
                    ProcessInstruction::IORequest => {
                        process.state = ProcessState::IORequest;
                        return SchedulerTickOutcome::ProcessContinued(i);
                    }
                    ProcessInstruction::Continue => {
                        return SchedulerTickOutcome::ProcessContinued(i);
                    }
                }
            }
        }

        // Now we find a process that is Ready and start it.
        for (i, process) in &mut self.processes.iter_mut().enumerate() {
            if process.state == ProcessState::Ready {
                process.state = ProcessState::Running;
                return SchedulerTickOutcome::ProcessResumed(i);
            }
        }

        // If we get here, there is nothing to do.
        SchedulerTickOutcome::Idle
    }

    fn new() -> Self {
        Self {
            processes: Vec::new(),
            io_device: IODevice::new(),
            rng: rand::rngs::StdRng::seed_from_u64(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_first_come_first_serve_scheduler() {
        let mut scheduler = FirstComeFirstServeScheduler::new();
        scheduler.register_process(4, 0.9);
        scheduler.register_process(3, 0.2);
        scheduler.register_process(2, 0.1);

        // Iterate it for some ticks and print the outcome of each tick.
        for _ in 0..100 {
            let outcome = scheduler.tick();
            println!("{:?}", outcome);
        }
    }
}

/// A struct that wraps a scheduler and provides an event queue of the events that
/// the scheduler generates.
pub struct SchedulerWrapper<S: Scheduler> {
    /// The scheduler.
    pub scheduler: S,
    /// The event queue.
    pub event_queue: Vec<SchedulerTickOutcome>,
}

impl<S: Scheduler> SchedulerWrapper<S> {
    /// Creates a new scheduler wrapper.
    pub fn new(scheduler: S) -> Self {
        Self {
            scheduler,
            event_queue: Vec::new(),
        }
    }

    /// Ticks the scheduler and adds the outcome to the event queue.
    pub fn tick(&mut self) {
        let outcome = self.scheduler.tick();
        self.event_queue.push(outcome);
    }
}
