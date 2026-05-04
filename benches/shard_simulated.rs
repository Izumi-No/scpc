use scsp::connection::Connection;
use scsp::frame::{FrameHeader, FrameType, encode_header};
use scsp::shard::Shard;
use scsp::transport::Transport;
use std::sync::Arc;
use std::time::Instant;

fn build_frame(payload: &[u8]) -> Vec<u8> {
    let header = FrameHeader {
        version: 1,
        frame_type: FrameType::Send,
        flags: 0,
        stream_id: 0,
        message_id: 1,
        payload_len: payload.len() as u32,
    };
    let mut header_buf = [0u8; 16];
    encode_header(&header, &mut header_buf);
    let mut frame = Vec::with_capacity(16 + payload.len());
    frame.extend_from_slice(&header_buf);
    frame.extend_from_slice(payload);
    frame
}

fn main() {
    // Allow override via environment variable SIM_TARGET
    let target: usize = std::env::var("SIM_TARGET")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(200_000);

    println!(
        "Shard simulated bench: creating {} simulated connections",
        target
    );
    let payload = b"hello";
    let frame = build_frame(payload);
    let frame_arc = Arc::new(frame);

    let mut shard = Shard::new(1024);

    // populate shard with simulated connections
    let buf_size = 128usize; // per-connection read buffer size
    for _ in 0..target {
        let conn = Connection::simulated(frame_arc.clone(), buf_size);
        let id = shard.conns.insert(conn);
        shard.timers.add(id, 30);
    }

    println!("Inserted {} connections, starting tick loop", target);

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();

    let start = Instant::now();
    rt.block_on(async {
        // run multiple ticks to process IO and frames
        for _ in 0..10 {
            shard.tick().await;
        }
    });
    let dur = start.elapsed();

    println!("Shard simulated tick loop completed in {:?}", dur);
}
