use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

use scsp_core::timer::TimerWheel;

fn timer_wheel_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("TimerWheel");
    // Pre-reserve per-slot capacity to avoid allocation noise during the timed loop.
    group.bench_function(BenchmarkId::new("insert and tick", "1m/60"), |b| {
        b.iter(|| {
            // Reserve about 20k entries per slot (for 1_000_000 timers across 60 slots).
            let mut wheel = TimerWheel::with_slot_capacity(60, 20_000);
            for i in 0..1_000_000 {
                wheel.add(i, 30);
            }
            for _ in 0..30 {
                let expired = wheel.tick();
                black_box(expired);
            }
        })
    });
    group.finish();
}

criterion_group!(benches, timer_wheel_bench);
criterion_main!(benches);
