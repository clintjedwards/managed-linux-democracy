//! This simply just does busy work to take up CPU time.

const SCHED_EXT: i32 = 7;

fn main() {
    unsafe {
        let pid = libc::getpid();
        let param = libc::sched_param { sched_priority: 0 };
        if libc::sched_setscheduler(pid, SCHED_EXT, &param) != 0 {
            panic!("{:#?}", std::io::Error::last_os_error());
        }
    }

    loop {
        let _ = 1 + 1;
    }
}
