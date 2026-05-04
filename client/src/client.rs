use crate::transport::ClientTransport;
use anyhow::{Result, bail};
use scsp_core::frame::{FrameHeader, FrameType, QosLevel, encode_header, parse_frame};

#[derive(Debug, PartialEq, Eq)]
pub enum ClientState {
    Disconnected,
    Connecting,
    Authenticating,
    Active,
    Closed,
}

pub struct Client {
    pub transport: ClientTransport,
    pub state: ClientState,
    pub read_buf: Vec<u8>,
    pub read_len: usize,
    pub msg_id_counter: u32,
    pub session_id: Option<u64>,
}

impl Client {
    /// Retry an async operation with a timeout per attempt and simple backoff.
    async fn retry_with_timeout<F, Fut, T>(
        mut attempts: u32,
        timeout_ms: u64,
        backoff_ms: u64,
        mut op: F,
    ) -> anyhow::Result<T>
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<T>>,
    {
        let mut last_err: Option<anyhow::Error> = None;
        while attempts > 0 {
            attempts -= 1;
            let fut = op();
            let dur = std::time::Duration::from_millis(timeout_ms);
            match tokio::time::timeout(dur, fut).await {
                Ok(Ok(v)) => return Ok(v),
                Ok(Err(e)) => {
                    last_err = Some(e);
                }
                Err(_) => {
                    last_err = Some(anyhow::anyhow!("operation timed out"));
                }
            }

            if attempts > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow::anyhow!("unknown error")))
    }

    /// Connect to the given address. If `prefer_quic` is true and the binary is compiled with
    /// the `quic` feature, the client will attempt a QUIC connection first and fall back to TCP on failure.
    /// The QUIC attempt will retry up to 3 times with a per-attempt timeout of 200ms and 100ms backoff.
    pub async fn connect(addr: &str, prefer_quic: bool) -> Result<Self> {
        // addr is expected to be "host:port"
        if prefer_quic {
            #[cfg(feature = "quic")]
            {
                if let Some(host) = addr.split(':').next() {
                    let connect_closure = || {
                        let addr = addr.to_string();
                        let host = host.to_string();
                        async move {
                            let t =
                                scsp_core::transport::Transport::connect_quic(&addr, &host).await;
                            match t {
                                Ok(tr) => Ok(tr),
                                Err(e) => Err(anyhow::anyhow!(e.to_string())),
                            }
                        }
                    };

                    // 3 attempts, 200ms timeout each, 100ms backoff
                    if let Ok(transport) =
                        Self::retry_with_timeout(3, 200, 100, connect_closure).await
                    {
                        return Ok(Self {
                            transport,
                            state: ClientState::Connecting,
                            read_buf: vec![0u8; 8192],
                            read_len: 0,
                            msg_id_counter: 1,
                            session_id: None,
                        });
                    }
                    // else fallthrough to TCP
                }
            }
        }

        // TCP fallback
        let transport = ClientTransport::connect_tcp(addr).await?;
        Ok(Self {
            transport,
            state: ClientState::Connecting,
            read_buf: vec![0u8; 8192],
            read_len: 0,
            msg_id_counter: 1,
            session_id: None,
        })
    }

    pub async fn send_frame(
        &mut self,
        frame_type: FrameType,
        flags: u16,
        payload: &[u8],
    ) -> Result<u32> {
        let msg_id = self.msg_id_counter;
        self.msg_id_counter = self.msg_id_counter.wrapping_add(1);

        let header = FrameHeader {
            version: 1,
            frame_type,
            flags,
            stream_id: 0,
            message_id: msg_id,
            payload_len: payload.len() as u32,
        };

        self.transport.send_frame(&header, payload).await?;
        Ok(msg_id)
    }

    pub async fn read_next_frame(&mut self) -> Result<(FrameHeader, Vec<u8>)> {
        loop {
            // Try to parse from existing buffer
            if let Some((header, payload, _)) = parse_frame(&self.read_buf[..self.read_len]) {
                let total_len = 16 + header.payload_len as usize;
                let payload_vec = payload.to_vec();
                let header_clone = header.clone();

                // Shift buffer
                let remaining = self.read_len - total_len;
                self.read_buf.copy_within(total_len..self.read_len, 0);
                self.read_len = remaining;

                return Ok((header_clone, payload_vec));
            }

            // Need more data
            let space = self.read_buf.len() - self.read_len;
            if space == 0 {
                bail!("Buffer full, frame too large");
            }

            let n = self
                .transport
                .receive_frame(&mut self.read_buf[self.read_len..])
                .await?;
            if n == 0 {
                self.state = ClientState::Closed;
                bail!("Connection closed by server");
            }
            self.read_len += n;
        }
    }

    pub async fn authenticate(&mut self, token: &[u8]) -> Result<()> {
        // 1. Send Hello
        self.send_frame(FrameType::Hello, 0, &[]).await?;

        // 2. Wait for Server Hello
        let (hello_hdr, _) = self.read_next_frame().await?;
        if hello_hdr.frame_type != FrameType::Hello {
            bail!("Expected Hello, got {:?}", hello_hdr.frame_type);
        }

        self.state = ClientState::Authenticating;

        // 3. Send Auth
        self.send_frame(FrameType::Auth, 0, token).await?;

        // 4. Wait for AuthOk
        let (auth_hdr, payload) = self.read_next_frame().await?;
        match auth_hdr.frame_type {
            FrameType::AuthOk => {
                self.state = ClientState::Active;
                // Parse session ID from payload if present (simplified)
                if payload.len() >= 8 {
                    let mut sid_bytes = [0u8; 8];
                    sid_bytes.copy_from_slice(&payload[0..8]);
                    self.session_id = Some(u64::from_be_bytes(sid_bytes));
                }
                Ok(())
            }
            FrameType::Error => {
                self.state = ClientState::Closed;
                bail!("Authentication failed");
            }
            _ => bail!("Unexpected frame during auth: {:?}", auth_hdr.frame_type),
        }
    }

    pub async fn ping(&mut self) -> Result<()> {
        self.send_frame(FrameType::Ping, 0, &[]).await?;
        let (hdr, _) = self.read_next_frame().await?;
        if hdr.frame_type != FrameType::Pong {
            bail!("Expected Pong, got {:?}", hdr.frame_type);
        }
        Ok(())
    }

    pub async fn send_message(&mut self, payload: &[u8], qos: QosLevel) -> Result<()> {
        let mut flags = 0;
        flags |= qos as u16;

        let msg_id = self.send_frame(FrameType::Send, flags, payload).await?;

        if qos == QosLevel::Qos1 || qos == QosLevel::Qos2 {
            let (hdr, _) = self.read_next_frame().await?;
            if hdr.frame_type != FrameType::Ack || hdr.message_id != msg_id {
                bail!("Failed to receive valid Ack for message {}", msg_id);
            }
        }
        Ok(())
    }

    pub async fn close(&mut self) -> Result<()> {
        self.send_frame(FrameType::Close, 0, &[]).await?;
        self.state = ClientState::Closed;
        Ok(())
    }
}
