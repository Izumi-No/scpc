use crate::transport::Transport;
use smallvec::SmallVec;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::time::Instant;

#[derive(Debug, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Authenticating,
    Active,
    Resuming,
    Closed,
}

pub struct Connection {
    pub state: ConnectionState,
    pub transport: Transport,
    pub session_id: Option<u64>,
    pub last_activity: Instant,

    pub read_buf: Vec<u8>,
    pub read_len: usize,
    pub write_buf: SmallVec<[u8; 4096]>,
    pub unacked_sends: HashMap<u32, Instant>, // message_id -> timestamp
}

impl Connection {
    pub fn new(transport: Transport) -> Self {
        Self {
            state: ConnectionState::Connecting,
            transport,
            session_id: None,
            last_activity: Instant::now(),
            read_buf: vec![0u8; 4096],
            write_buf: SmallVec::new(),
            read_len: 0,
            unacked_sends: HashMap::new(),
        }
    }

    /// Create a simulated connection that uses a Mock transport and a smaller read buffer to
    /// minimize heap usage during large-scale benchmarks. The provided `frame` is returned by the
    /// transport on every `receive_frame` call when `repeat` is true.
    pub fn simulated(frame: Arc<Vec<u8>>, buf_size: usize) -> Self {
        let transport = Transport::mock(frame, true);
        Self {
            state: ConnectionState::Connecting,
            transport,
            session_id: None,
            last_activity: Instant::now(),
            read_buf: vec![0u8; buf_size],
            write_buf: SmallVec::new(),
            read_len: 0,
            unacked_sends: HashMap::new(),
        }
    }

    /// Queues data to be sent. Flushes automatically if the threshold is reached.
    pub fn queue_send(&mut self, data: &[u8]) {
        self.write_buf.extend_from_slice(data);
    }

    // transition to closing state
    pub fn close(&mut self) {
        // Send Close frame, etc.
        self.state = ConnectionState::Closed;
    }

    pub fn start_auth(&mut self) {
        if self.state == ConnectionState::Connecting {
            // Send Hello frame, etc.
            self.state = ConnectionState::Authenticating;
        }
    }

    pub fn start_resume(&mut self) {
        if self.state == ConnectionState::Connecting {
            self.state = ConnectionState::Resuming;
        }
    }

    pub fn complete_auth(&mut self) {
        if self.state == ConnectionState::Authenticating || self.state == ConnectionState::Resuming
        {
            // Send AuthOk frame, etc.
            self.state = ConnectionState::Active;
        }
    }
}
