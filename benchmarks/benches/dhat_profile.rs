#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use scsp_core::frame::{FrameHeader, FrameType, encode_header, parse_frame};
use scsp_core::timer::TimerWheel;

fn main() {
    let _profiler = dhat::Profiler::builder().testing().build();

    // Profile TimerWheel
    let mut wheel = TimerWheel::new(60);
    for i in 0..10_000 {
        wheel.add(i, 30);
    }
    for _ in 0..30 {
        let _ = wheel.tick();
    }

    // Profile Frame Parse
    let header = FrameHeader {
        version: 1,
        frame_type: FrameType::Send,
        flags: 2,
        stream_id: 1024,
        message_id: 9999,
        payload_len: 256,
    };

    let payload = vec![0xAB; 256];
    let mut encode_buf = [0u8; 16];

    for _ in 0..10_000 {
        encode_header(&header, &mut encode_buf);
    }

    let mut parse_buf = vec![0u8; 16 + 256];
    encode_header(&header, &mut parse_buf[0..16].try_into().unwrap());
    parse_buf[16..].copy_from_slice(&payload);

    for _ in 0..10_000 {
        let _ = parse_frame(&parse_buf);
    }

    let stats = dhat::HeapStats::get();
    println!("Total allocations: {}", stats.total_blocks);
    println!("Total bytes allocated: {}", stats.total_bytes);
    println!("Max bytes in memory: {}", stats.max_bytes);
}
