use crossbeam_queue::ArrayQueue;
use slab::Slab;
use tokio::time::Instant;

use crate::connection::Connection;
use crate::frame::{FrameHeader, FrameType, QosLevel, parse_frame};
use crate::timer::TimerWheel;

pub enum ShardMessage {
    //quit, broadcast, etc. can be added here
    NewConnection(Connection),
}

pub struct Shard {
    pub conns: Slab<Connection>,
    pub inbox: ArrayQueue<ShardMessage>,
    pub timers: TimerWheel,
}

impl Shard {
    pub fn new(inbox_capacity: usize) -> Self {
        Self {
            conns: Slab::with_capacity(10_000),
            inbox: ArrayQueue::new(inbox_capacity),
            timers: TimerWheel::new(60), // 60 slots
        }
    }

    pub async fn tick(&mut self) {
        self.process_inbox();
        self.process_io().await;
        self.process_frames();
        self.flush_writes().await;
        self.process_timers();
    }

    fn process_inbox(&mut self) {
        while let Some(msg) = self.inbox.pop() {
            match msg {
                ShardMessage::NewConnection(conn) => {
                    let id = self.conns.insert(conn);
                    self.timers.add(id, 30); // e.g. 30 ticks timeout
                }
            }
        }
    }

    async fn process_io(&mut self) {
        let mut to_remove = Vec::new();

        for (id, conn) in self.conns.iter_mut() {
            // Read/write logic here. If connection is closed or errored, add id to to_remove.
            let space = conn.read_buf.len() - conn.read_len;
            if space > 0 {
                match conn
                    .transport
                    .receive_frame(&mut conn.read_buf[conn.read_len..])
                    .await
                {
                    Ok(0) => {
                        // Connection closed
                        to_remove.push(id);
                    }
                    Ok(n) => {
                        conn.read_len += n;
                        // Process frames in read_buf here, and update last_activity
                        conn.last_activity = Instant::now();
                    }
                    Err(_) => {
                        // Read error
                        to_remove.push(id);
                    }
                }
            }
        }

        for id in to_remove {
            self.conns.remove(id);
        }
    }

    fn process_frames(&mut self) {
        let mut responses: Vec<(usize, FrameHeader, Vec<u8>)> = Vec::new();

        for (id, conn) in self.conns.iter_mut() {
            let mut offset = 0;
            while offset < conn.read_len {
                if let Some((header, payload, _remaining)) =
                    parse_frame(&conn.read_buf[offset..conn.read_len])
                {
                    // Handle frame based on header.frame_type and payload
                    match header.frame_type {
                        FrameType::Hello => {
                            let mut response_header = header.clone();
                            response_header.frame_type = FrameType::Auth;
                            responses.push((id, response_header, vec![]));
                            conn.last_activity = Instant::now();
                            conn.start_auth();
                        }
                        FrameType::Auth => {
                            // Validate auth payload, etc.
                            let mut response_header = header.clone();
                            response_header.frame_type = FrameType::AuthOk;
                            responses.push((id, response_header, vec![]));
                            conn.last_activity = Instant::now();
                            conn.complete_auth();
                        }
                        FrameType::Ping => {
                            let mut response_header = header.clone();
                            response_header.frame_type = FrameType::Pong;
                            responses.push((id, response_header, payload.to_vec()));
                            conn.last_activity = Instant::now();
                        }
                        FrameType::Send => {
                            // Process Send frame, route message, etc.
                            if header.qos() == QosLevel::Qos1 || header.qos() == QosLevel::Qos2 {
                                let mut response_header = header.clone();
                                response_header.frame_type = FrameType::Ack;
                                // In a full QoS 2 implementation, we would also verify if the msg ID was already processed.
                                responses.push((id, response_header, vec![]));
                            }
                            conn.last_activity = Instant::now();
                        }
                        FrameType::Ack => {
                            // Clear message from unacknowledged sends
                            conn.unacked_sends.remove(&header.message_id);
                            conn.last_activity = Instant::now();
                        }
                        FrameType::Close => {
                            conn.close();
                        }
                        FrameType::Resume => {
                            // Validate resume payload
                            let mut response_header = header.clone();
                            response_header.frame_type = FrameType::AuthOk;
                            responses.push((id, response_header, vec![]));
                            conn.last_activity = Instant::now();
                            conn.start_resume();
                            conn.complete_auth();
                        }
                        FrameType::Pong | FrameType::Meta => {
                            // Simply update activity for now
                            conn.last_activity = Instant::now();
                        }
                        FrameType::Error => {
                            // On error, close the connection
                            conn.close();
                        }
                        _ => {}
                    }

                    offset += 16 + header.payload_len as usize;
                } else {
                    break; // No more complete frames
                }
            }

            if offset > 0 {
                // Shift unparsed data to the front
                let remaining = conn.read_len - offset;
                conn.read_buf.copy_within(offset..conn.read_len, 0);
                conn.read_len = remaining;
            }
        }

        // Queue responses to connections
        for (id, header, payload) in responses {
            if let Some(conn) = self.conns.get_mut(id) {
                let mut header_buf = [0u8; 16];
                crate::frame::encode_header(&header, &mut header_buf);
                conn.queue_send(&header_buf);
                conn.queue_send(&payload);
            }
        }
    }

    async fn flush_writes(&mut self) {
        let mut to_remove = Vec::new();

        for (id, conn) in self.conns.iter_mut() {
            if !conn.write_buf.is_empty() {
                match conn
                    .transport
                    .send_frame(
                        &FrameHeader {
                            version: 1,
                            frame_type: FrameType::Send,
                            flags: 0,
                            stream_id: 0,
                            message_id: 0,
                            payload_len: conn.write_buf.len() as u32,
                        },
                        &conn.write_buf,
                    )
                    .await
                {
                    Ok(_) => {
                        conn.write_buf.clear();
                    }
                    Err(_) => {
                        // Write error
                        to_remove.push(id);
                    }
                }
            }
        }

        for id in to_remove {
            self.conns.remove(id);
        }
    }

    fn process_timers(&mut self) {
        // In a real implementation this would trigger based on real time.
        // For simplicity, we just tick the wheel and drop idle connections.

        let expired = self.timers.tick();
        let now = Instant::now();

        for id in expired {
            if let Some(conn) = self.conns.get(id) {
                if now.duration_since(conn.last_activity).as_secs() > 30 {
                    self.conns.remove(id);
                } else {
                    // Re-arm timer
                    self.timers.add(id, 30);
                }
            }
        }
    }
}
