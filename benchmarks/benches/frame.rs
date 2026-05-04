use criterion::{Criterion, black_box, criterion_group, criterion_main};
use scsp_core::frame::{FrameHeader, FrameType, encode_header, parse_frame};

fn frame_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("Frame Processing");

    let header = FrameHeader {
        version: 1,
        frame_type: FrameType::Send,
        flags: 2, // QoS 2
        stream_id: 1024,
        message_id: 9999,
        payload_len: 256,
    };

    let payload = vec![0xAB; 256];
    let mut encode_buf = [0u8; 16];

    group.bench_function("encode_header", |b| {
        b.iter(|| {
            encode_header(black_box(&header), black_box(&mut encode_buf));
        })
    });

    let mut parse_buf = vec![0u8; 16 + 256];
    encode_header(&header, &mut parse_buf[0..16].try_into().unwrap());
    parse_buf[16..].copy_from_slice(&payload);

    group.bench_function("parse_frame", |b| {
        b.iter(|| {
            let parsed = parse_frame(black_box(&parse_buf));
            black_box(parsed);
        })
    });

    group.finish();
}

criterion_group!(benches, frame_bench);
criterion_main!(benches);
