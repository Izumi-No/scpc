use criterion::{Criterion, black_box, criterion_group, criterion_main};

use scsp::timer::TimerWheel;

fn timer_wheel_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("TimerWheel");
    group.bench_function("insert and tick", |b| {
        b.iter(|| {
            let mut wheel = TimerWheel::new(60);
            for i in 0..1_000_000 {
                wheel.add(i, 30); // insert 10k timers with 30 ticks
            }
            for _ in 0..30 {
                let expired = wheel.tick();
                black_box(expired); // prevent optimization
            }
        })
    });
    group.finish();
}

criterion_group!(benches, timer_wheel_bench);
criterion_main!(benches);
