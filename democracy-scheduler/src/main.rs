mod bpf_skel;
pub use bpf_skel::*;
pub mod bpf_intf;

mod bpf;
use bpf::*;

use scx_utils::Topology;
use scx_utils::TopologyMap;
use scx_utils::UserExitInfo;

use std::thread;

use std::collections::BTreeSet;
use std::collections::HashMap;

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::SystemTime;

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use log::info;
use log::warn;

const SCHEDULER_NAME: &str = "democracy";

const VERSION: &str = env!("CARGO_PKG_VERSION");

// We could schedule this as a game which ever program gets to run the requisite amount of time is rewarded with the win
// The votes are submitted by rank choice by http/json
// We need to find some way to make the linux protections around the scheduler enable longer and we also need to make
// sure that we can do things like display to the screen and offer an http endpoint.
// We can do this somewhat easily by having the vote collector run in userspace and we can just simply scheudle everything
// as normal. The only things we don't scheudle is programs with a special name. OMG we can pit summer 1 vs summer 2
// against each other! WE need simplified rate-limiting to prevent people from calling curl a billion times to win.
// Upon a special timer finishing(we can literally just have a "last voted time in a file somewhere that we reference")
// The scheduler will specially allow that program to run for a certain amount of time.
// The scheudler can then also track how long that program has been running for in vruntime
// and print the winner via some other way(not sure on this yet) to make the scheduler spit out the winner in another way.
// We should also make a live graph of who is winning.

// Basic item stored in the task information map.
#[derive(Debug)]
struct TaskInfo {
    sum_exec_runtime: u64, // total cpu time used by the task
    vruntime: u64,         // total vruntime of the task
    avg_nvcsw: u64,        // average of voluntary context switches
    nvcsw: u64,            // total amount of voluntary context switches
    nvcsw_ts: u64,         // timestamp of the previous nvcsw update
}

// Task information map: store total execution time and vruntime of each task in the system.
//
// TaskInfo objects are stored in the HashMap and they are indexed by pid.
//
// Entries are removed when the corresponding task exits.
//
// This information is fetched from the BPF section (through the .exit_task() callback) and
// received by the user-space scheduler via self.bpf.dequeue_task(): a task with a negative .cpu
// value represents an exiting task, so in this case we can free the corresponding entry in
// TaskInfoMap (see also Scheduler::drain_queued_tasks()).
struct TaskInfoMap {
    tasks: HashMap<i32, TaskInfo>,
}

// TaskInfoMap implementation: provide methods to get items and update items by pid.
impl TaskInfoMap {
    fn new() -> Self {
        TaskInfoMap {
            tasks: HashMap::new(),
        }
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Clone)]
struct Task {
    qtask: QueuedTask,    // queued task
    vruntime: u64,        // total vruntime (that determines the order how tasks are dispatched)
    is_interactive: bool, // task can preempt other tasks
}

struct TaskTree {
    task_map: HashMap<i32, Task>, // Map from pid to task
}

// Main scheduler object
struct Scheduler<'a> {
    bpf: BpfScheduler<'a>, // BPF connector
    task_map: TaskInfoMap,
}

impl<'a> Scheduler<'a> {
    fn init() -> Result<Self> {
        // Initialize core mapping topology.
        let topo = Topology::new().expect("Failed to build host topology");

        // Scheduler task map to store tasks information.
        let task_map = TaskInfoMap::new();

        let nr_cpus = topo.nr_cpus_possible();

        // This function is doing a lot of heavy lifting, it is our interface into the sched_ext hooks such that we
        // can recieve and perform various scheudling events. Let's explain some of the parameters it takes in.
        // You can find a better explaination of these variables here: https://github.com/sched-ext/scx/blob/main/scheds/rust/scx_rustland/src/main.rs#L85-L161

        //TODO(turn partial back on when we have actual tasks to schedule)

        let bpf = BpfScheduler::init(
            1000000, // slice_us: How much time the task should be given to run in micro-seconds. 1000000 is 1s.
            topo.nr_cpus_possible() as i32, // nr_cpus_online: Tells the scheduler how many CPUs they are and how many
            false, // partial: Setting this to false tells BPF that we want to be responsible for how ALL tasks get scheudled, setting this to true says "only tasks that specifically set their scheduler to SCHED_EXT"
            0, // exit_dump_len: Exit debug dump buffer length. 0 indicates default. I'll be honest i'm not exactly sure what this means, but if I had to guess I assume that this is the number in bytes of hte debug buffer which will be printed in debug mode when the scheudler is exited.
            true, // full_user: Setting this to true tells BPF that we want all scheduler decisions to be made in user spaces, instead of it trying to optimize some of the decision making in kernel space
            false, // low_power: This enables a bunch of settings that cause the CPU to operate in a way that saves power.
            false, // fifo_sched: By default when there is low utilization the system will simply go into FIFO mode since that provides better performance. This turns that off since we want to control the scheduling.
            true, // debug: Simply prints all events that occurred to /sys/kernel/debug/tracing/trace_pipe
        )?;
        info!("{} scheduler attached - {} CPUs", SCHEDULER_NAME, nr_cpus);

        // Return scheduler object.
        Ok(Self { bpf, task_map })
    }

    // Return current timestamp in ns.
    fn now() -> u64 {
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();
        ts.as_nanos() as u64
    }

    fn schedule(&self) {
        println!("Would have scheduled something here!");
        // grab all available tasks
        // check if we're supposed to schedule them yet by checking the time that we scheduled the last program.
        // if it's time to schedule one then check who the last winner was by checking the file where the current winner is output
        // The winner will be updated second by second and the schedule will check who the winner is on any given second.
        // When the winner if found then we can schedule ONLY that task and update it's vruntime.
        // When we update the vruntime for a specific task then we write the current stats to a file that the frontend can handle
        // We then just keep repeating the process.

        // Yield to avoid using too much CPU form the scheduler itself.
        thread::yield_now();
    }

    fn run(&mut self, shutdown: Arc<AtomicBool>) -> Result<()> {
        while !shutdown.load(Ordering::Relaxed) && !self.bpf.exited() {
            // Call the main scheduler body.
            self.schedule();
        }

        self.bpf.shutdown_and_report()
    }
}

// Unregister the scheduler.
impl<'a> Drop for Scheduler<'a> {
    fn drop(&mut self) {
        info!("Unregister {} scheduler", SCHEDULER_NAME);
    }
}

fn main() -> Result<()> {
    println!("Managed Democracy scheduler is starting...");

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();
    ctrlc::set_handler(move || {
        shutdown_clone.store(true, Ordering::Relaxed);
    })
    .context("Error setting Ctrl-C handler")?;

    loop {
        let mut sched = Scheduler::init()?;
        // Start the scheduler.
        if let Err(e) = sched.run(shutdown.clone()) {
            eprint!("scheduler has shutdown; {:#?}", e);
            break;
        }
    }

    Ok(())
}
