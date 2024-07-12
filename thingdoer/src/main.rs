//! This simply just does busy work to take up CPU time.

const SCHED_EXT: i32 = 7;

fn fibonacci(n: u64) -> u64 {
    if n <= 1 {
        return n;
    }
    fibonacci(n - 1) + fibonacci(n - 2)
}

fn main() {
    unsafe {
        let pid = libc::getpid();
        let param = libc::sched_param { sched_priority: 10 };
        if libc::sched_setscheduler(pid, SCHED_EXT, &param) != 0 {
            panic!("{:#?}", std::io::Error::last_os_error());
        }
    }

    let mut n = 0;
    loop {
        println!("Fibonacci {}: {}", n, fibonacci(n));
        n += 1;
    }
}
