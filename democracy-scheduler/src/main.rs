mod bpf_skel;
pub use bpf_skel::*;
pub mod bpf_intf;

mod bpf;
use bpf::*;

use scx_utils::Topology;
use scx_utils::TopologyMap;
use scx_utils::UserExitInfo;

use std::{io::Stdout, thread};

use std::collections::BTreeSet;
use std::collections::HashMap;

use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::SystemTime;

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use nix::sys::signal::{kill, Signal};
use nix::unistd::Pid;
use serde::Deserialize;
use tracing::{debug, error, info, warn};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};

const SCHEDULER_NAME: &str = "democracy";

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Hash, Eq, PartialEq)]
enum Competitors {
    Summer1,
    Summer2,
}

impl Competitors {
    fn from_str(input: &str) -> Result<Competitors> {
        match input.to_lowercase().as_str() {
            "summer1" => Ok(Competitors::Summer1),
            "summer2" => Ok(Competitors::Summer2),
            _ => bail!("Unknown competitor"),
        }
    }
}

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

#[derive(Debug, Clone)]
struct Task {
    pub vruntime: u64,
    pub queued_task: QueuedTask,
}

// Main scheduler object
struct Scheduler<'a> {
    bpf: BpfScheduler<'a>,                // BPF connector
    task_map: HashMap<u32, Option<Task>>, // pid to task
    owner_map: HashMap<Competitors, u32>, // pid to binary owner
}

impl<'a> Scheduler<'a> {
    fn init() -> Result<Self> {
        // Initialize core mapping topology.
        let topo = Topology::new().expect("Failed to build host topology");

        // Scheduler task map to store tasks information.
        let task_map = HashMap::new();
        let owner_map = HashMap::new();

        let nr_cpus = topo.nr_cpus_possible();

        // This function is doing a lot of heavy lifting, it is our interface into the sched_ext hooks such that we
        // can recieve and perform various scheudling events. Let's explain some of the parameters it takes in.
        // You can find a better explaination of these variables here: https://github.com/sched-ext/scx/blob/main/scheds/rust/scx_rustland/src/main.rs#L85-L161

        let bpf = BpfScheduler::init(
            1000000, // slice_us: How much time the task should be given to run in micro-seconds. 1000000 is 1s.
            topo.nr_cpus_possible() as i32, // nr_cpus_online: Tells the scheduler how many CPUs they are and how many
            true, // partial: Setting this to false tells BPF that we want to be responsible for how ALL tasks get scheudled, setting this to true says "only tasks that specifically set their scheduler to SCHED_EXT"
            0, // exit_dump_len: Exit debug dump buffer length. 0 indicates default. I'll be honest i'm not exactly sure what this means, but if I had to guess I assume that this is the number in bytes of hte debug buffer which will be printed in debug mode when the scheudler is exited.
            true, // full_user: Setting this to true tells BPF that we want all scheduler decisions to be made in user spaces, instead of it trying to optimize some of the decision making in kernel space
            false, // low_power: This enables a bunch of settings that cause the CPU to operate in a way that saves power.
            false, // fifo_sched: By default when there is low utilization the system will simply go into FIFO mode since that provides better performance. This turns that off since we want to control the scheduling.
            true, // debug: Simply prints all events that occurred to /sys/kernel/debug/tracing/trace_pipe
        )?;

        info!(name = SCHEDULER_NAME, cpus = nr_cpus, "scheduler attached");

        Ok(Self {
            bpf,
            task_map,
            owner_map,
        })
    }

    fn schedule(&mut self) {
        // First lets drain all the tasks from the queue and only keep track of the ones that we want to focus on
        // scheduling.
        loop {
            match self.bpf.dequeue_task() {
                // We were able to get a new task to schedule.
                Ok(Some(task)) => {
                    // check if the pid is one we care about
                    if !self.task_map.contains_key(&(task.pid as u32)) {
                        continue;
                    }

                    // If it does grab it and stick it in the map
                    self.task_map.insert(
                        task.pid as u32,
                        Some(Task {
                            queued_task: task,
                            vruntime: 0,
                        }),
                    );
                    continue;
                }

                // The queue is empty.
                Ok(None) => {
                    self.bpf.update_tasks(Some(0), Some(0));
                    break;
                }

                // Some error occurred.
                Err(err) => {
                    error!(err = %err, "Encountered error while draining tasks");
                    continue;
                }
            }
        }

        let winner = match get_current_winner() {
            Ok(winner) => winner,
            Err(e) => {
                error!(err = %e, "There was no winner when we checked");
                std::thread::sleep(std::time::Duration::from_secs(1));
                return;
            }
        };

        let winner_pid = self.owner_map.get(&winner).unwrap();
        let winner_task = self.task_map.get(winner_pid).unwrap();
        let winner_task = match winner_task.clone() {
            Some(task) => task,
            None => return,
        };

        let mut dispatched_task = DispatchedTask::new(&winner_task.queued_task);
        dispatched_task.set_slice_ns(100000000);

        match self.bpf.dispatch_task(&dispatched_task) {
            Ok(_) => {
                info!(pid =  winner_pid, owner = ?winner, "Task successfully scheduled");
            }
            Err(e) => {
                error!(pid = winner_pid, owner = ?winner, error = %e, "Could not schedule task");
                // If there is an error here in a real scheudler we would attempt to schedule
                // the task again, but here we just error and continue.
            }
        }

        // this is a hack so the other scheduler doesn't try to get to it before we do.
        // let pidkill = Pid::from_raw(*winner_pid as i32);
        // kill(pidkill, Signal::SIGCONT).unwrap();

        let mut winner_task = winner_task.clone();
        winner_task.vruntime += 1000000000;

        self.task_map.insert(*winner_pid, Some(winner_task.clone()));

        std::thread::sleep(std::time::Duration::from_millis(500));

        // Yield to avoid using too much CPU from the scheduler itself.
        // thread::yield_now();
    }

    fn run(&mut self, shutdown: Arc<AtomicBool>) -> Result<()> {
        while !shutdown.load(Ordering::Relaxed) {
            // Call the main scheduler body.
            self.schedule();
        }

        Ok(())
    }
}

// Unregister the scheduler.
// impl<'a> Drop for Scheduler<'a> {
//     fn drop(&mut self) {
//         info!("Unregister {} scheduler", SCHEDULER_NAME);
//     }
// }

fn main() -> Result<()> {
    init_logger().unwrap();

    info!("Managed Democracy scheduler is starting...");

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();
    ctrlc::set_handler(move || {
        shutdown_clone.store(true, Ordering::Relaxed);
    })
    .context("Error setting Ctrl-C handler")?;

    let mut sched = Scheduler::init()?;

    let summer_1_pid = launch_process("thingdoer", "summer1");
    let summer_2_pid = launch_process("thingdoer", "summer2");

    sched.task_map.insert(summer_1_pid, None);
    sched.task_map.insert(summer_2_pid, None);

    sched.owner_map.insert(Competitors::Summer1, summer_1_pid);
    sched.owner_map.insert(Competitors::Summer2, summer_2_pid);

    loop {
        // Start the scheduler.
        if let Err(e) = sched.run(shutdown.clone()) {
            eprint!("scheduler has shutdown; {:#?}", e);
            break;
        }
    }

    Ok(())
}

fn init_logger() -> Result<()> {
    let filter = EnvFilter::from_default_env()
        // These directives filter out debug information that is too numerous and we generally don't need during
        // development.
        .add_directive("sqlx=off".parse().expect("Invalid directive"))
        .add_directive("h2=off".parse().expect("Invalid directive"))
        .add_directive("hyper=off".parse().expect("Invalid directive"))
        .add_directive("rustls=off".parse().expect("Invalid directive"))
        .add_directive("bollard=off".parse().expect("Invalid directive"))
        .add_directive("reqwest=off".parse().expect("Invalid directive"))
        .add_directive("tungstenite=off".parse().expect("Invalid directive"))
        .add_directive("scx_utils=off".parse().expect("Invalid directive"))
        .add_directive(LevelFilter::DEBUG.into()); // Accept debug level logs and above for everything else

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .compact()
        .init();

    Ok(())
}

// Launches a process and returns the PID.
fn launch_process(bin_name: &str, name: &str) -> u32 {
    // Launch the process
    let mut command = std::process::Command::new(bin_name);
    command.arg(name);

    let child = command
        .stdout(std::process::Stdio::null()) // Don't overwhelm with stdout logs
        .spawn()
        .expect("Failed to start process");

    // Get the PID of the launched process
    let pid = child.id();
    info!(pid = pid, bin_name = bin_name, "Launched process");

    pid
}

#[derive(Debug, Deserialize)]
struct CurrentWinnerResponse {
    current_tally: Vec<(String, u32)>,
}

fn get_current_winner() -> Result<Competitors> {
    let url = "http://localhost:8080/api/votes";

    let winner = reqwest::blocking::Client::new()
        .get(url)
        .header("User-Agent", "scheduler")
        .send()?;

    let tallys: CurrentWinnerResponse = winner.json()?;

    let mut winner = (String::from("_votes"), 0);

    for tally in tallys.current_tally {
        if tally.1 > winner.1 {
            winner = tally
        }
    }

    Competitors::from_str(winner.0.strip_suffix("_votes").unwrap())
}
