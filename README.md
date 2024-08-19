# Rusty Recurse Linux Process Scheduler <3

This past summer, I did a batch at [Recurse](https://recurse.com), which I will cherish for the rest of my life.
Recurse is a type of hacker school where you join a group of people (called a batch) and work on whatever you want
for nothing more than the love of computing.

During my time there, I decided to dive into lower-level systems and set my sights on learning more about operating
systems. This was purposeful because, although I've been a systems engineer my entire career, I've often shied away
from truly low-level code and systems (e.g., how do I write a bootloader?), preferring to stay towards the more popular
topics in cloud computing. So, I knew that taking a look under the hood of operating systems would reveal a whole new
world of abstractions.

The first half of my batch (the first 6 weeks) was dedicated to reading through
[Operating Systems: Three Easy Pieces](https://pages.cs.wisc.edu/~remzi/OSTEP/), and to add a practical component to
that, I also followed along with building an operating system in Rust using
[Philipp Oppermann's blog: Writing an OS in Rust](https://os.phil-opp.com/). This was very enlightening and extremely
challenging to wrap my head around everything all at once. I learned so much.

The second half of my batch (the last 6 weeks) was dedicated to doing a "final" project—something practical and fun.
I knew that finishing up writing an entire operating system was a bit out of scope for just 6 weeks, so instead,
I chose a specific feature of operating systems and ran with that.

I decided, in honor of it being an election year, that I would hold an election for which Linux process to run. So,
I dove into the best way to create a new Linux scheduler and surfaced with many paradigms to explore,
namely: `eBPF`, `sched_ext`, and more!

This repo represents the code I wrote for my final project. It provides:
* An eBPF Linux scheduler that queries a REST API web service to determine which Recurse "batch" has the most votes
(voting is simply done by curling a specific endpoint).
* The Linux scheduler then maps that vote back to a hardcoded process ID and schedules that process to run.
* It works in "partial" mode, so it only attempts to schedule processes within a specific process scheduling group.
* All other processes work as normal.

It's a bit challenging to run since `sched_ext` support for `eBPF` is still in the process of being merged into the
kernel, but running this on an operating system with early `sched_ext` support (like [CachyOS](https://cachyos.org/))
allows it to run like any other eBPF scheduler.

My perfectionism requires me to say that I had to learn and complete the task in a very short amount of time,
so the code isn't perfect by any means, and there is one gnarly segfault that still plagues it, but it works!
