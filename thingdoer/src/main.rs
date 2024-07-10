//! This simply just does busy work to take up CPU time.

fn fibonacci(n: u64) -> u64 {
    if n <= 1 {
        return n;
    }
    fibonacci(n - 1) + fibonacci(n - 2)
}

fn main() {
    let mut n = 0;
    loop {
        println!("Fibonacci {}: {}", n, fibonacci(n));
        n += 1;
    }
}
