use crate::transport::Transport;
use smallvec::SmallVec;
use std::collections::HashMap;
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

    pub read_buf: Box<[u8; 4096]>,
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
            read_buf: Box::new([0u8; 4096]),
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
