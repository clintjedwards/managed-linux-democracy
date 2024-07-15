//! This simply just does busy work to take up CPU time.

const SCHED_EXT: i32 = 7;
use colored::Colorize;
use rand::prelude::SliceRandom;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let word_choice = vec![
        "is the best ever",
        "are all flawless programmers",
        "are 100x engineers",
        "are 10x engineers",
        "are the absolute kindest",
        "are all future presidents",
        "are truly inspiring",
        "make the world a better place",
        "are incredibly talented",
        "are the heart and soul of the team",
        "have the best ideas",
        "are unstoppable",
        "bring joy to everyone around them",
        "are the definition of excellence",
        "make magic happen",
        "are shining examples of greatness",
        "are a dream team",
        "are heroes in disguise",
        "have a bright future ahead",
        "are simply the best",
    ];

    let name = &args[1];

    unsafe {
        let pid = libc::getpid();
        let param = libc::sched_param { sched_priority: 0 };
        if libc::sched_setscheduler(pid, SCHED_EXT, &param) != 0 {
            panic!("{:#?}", std::io::Error::last_os_error());
        }
    }

    loop {
        let mut rng = rand::thread_rng();
        let choice = word_choice.choose(&mut rng).unwrap();
        println!("{} {}!", name.yellow(), choice);
        std::thread::sleep(std::time::Duration::from_secs(1));
        unsafe {
            if libc::sched_yield() != 0 {
                panic!("{:#?}", std::io::Error::last_os_error());
            }
        }
    }
}
