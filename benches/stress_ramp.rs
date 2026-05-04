#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use scsp::timer::TimerWheel;
use std::time::{Duration, Instant};

struct VirtualConn {
    // minimal footprint connection to simulate scale
    last_activity: u64, // epoch secs
}

impl VirtualConn {
    fn new() -> Self {
        Self { last_activity: 0 }
    }

    fn touch(&mut self, now: u64) {
        self.last_activity = now;
    }
}

fn main() {
    let _profiler = dhat::Profiler::builder().testing().build();

    const TARGET: usize = 1_000_000;
    println!("Starting ramp-up to {} virtual connections", TARGET);

    let t0 = Instant::now();

    // Ramp-up in linear batches to avoid long single allocation pause
    let mut conns: Vec<VirtualConn> = Vec::with_capacity(TARGET);
    let mut wheel = TimerWheel::new(256);

    let batch = 50_000usize;
    let mut created = 0usize;
    while created < TARGET {
        let to_create = std::cmp::min(batch, TARGET - created);
        for i in 0..to_create {
            conns.push(VirtualConn::new());
            // schedule timer for this connection
            wheel.add(created + i, 30);
        }
        created += to_create;
        let elapsed = t0.elapsed();
        println!("Created {}/{} conns in {:?}", created, TARGET, elapsed);
    }

    let create_dur = t0.elapsed();
    println!("Total created {} conns in {:?}", TARGET, create_dur);

    // Warm tick loop: run a number of ticks and update last_activity for expired ids
    let tick_start = Instant::now();
    for _round in 0..60 {
        let expired = wheel.tick();
        let now = current_secs();
        for id in expired {
            // In this simulation most ids will refer to valid indices
            if id < conns.len() {
                conns[id].touch(now);
                // re-arm timer a few times to simulate activity
                wheel.add(id, 30);
            }
        }
    }
    let tick_dur = tick_start.elapsed();
    println!("Processed 60 ticks in {:?}", tick_dur);

    // Simulate a rapid burst of messages across all connections
    let burst_start = Instant::now();
    for i in 0..conns.len() {
        // touch half of them to simulate activity
        if i % 2 == 0 {
            conns[i].touch(current_secs());
        }
    }
    let burst_dur = burst_start.elapsed();
    println!(
        "Simulated burst update across {} conns in {:?}",
        conns.len(),
        burst_dur
    );

    // Collect allocation stats from dhat
    let stats = dhat::HeapStats::get();
    println!("Dhat total allocations: {}", stats.total_blocks);
    println!("Dhat total bytes allocated: {}", stats.total_bytes);
    println!("Dhat max bytes in memory: {}", stats.max_bytes);

    println!("Stress ramp completed.");
}

fn current_secs() -> u64 {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0));
    d.as_secs()
}
