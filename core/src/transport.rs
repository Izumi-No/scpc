use crate::frame::{FrameHeader, encode_header};
use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[cfg(feature = "quic")]
use quinn::NewConnection;

/// A tiny in-memory transport used for simulation and testing.
/// It returns the same pre-built frame on every call to `receive_frame` if `repeat` is true.
pub struct MockTransport {
    pub frame: Arc<Vec<u8>>,
    pub offset: usize,
    pub repeat: bool,
    // store sent frames for inspection
    pub sent: Vec<Vec<u8>>,
}

impl MockTransport {
    pub fn new(frame: Arc<Vec<u8>>, repeat: bool) -> Self {
        Self {
            frame,
            offset: 0,
            repeat,
            sent: Vec::new(),
        }
    }
}

/// Transport abstraction shared between client and server.
pub enum Transport {
    Tcp(tokio::net::TcpStream),
    #[cfg(feature = "quic")]
    Quic(quinn::SendStream, quinn::RecvStream),
    Mock(MockTransport),
}

impl Transport {
    /// Helper to construct a mock transport easily
    pub fn mock(frame: Arc<Vec<u8>>, repeat: bool) -> Self {
        Self::Mock(MockTransport::new(frame, repeat))
    }

    pub async fn send_frame(
        &mut self,
        header: &FrameHeader,
        payload: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut buf = [0u8; 16];
        encode_header(header, &mut buf);
        let mut frame = Vec::with_capacity(16 + payload.len());
        frame.extend_from_slice(&buf);
        frame.extend_from_slice(payload);

        match self {
            Self::Tcp(stream) => {
                stream.write_all(&frame).await?;
            }
            #[cfg(feature = "quic")]
            Self::Quic(send, _) => {
                send.write_all(&frame).await?;
            }
            Self::Mock(mock) => {
                mock.sent.push(frame);
            }
        }
        Ok(())
    }

    pub async fn receive_frame(
        &mut self,
        buf: &mut [u8],
    ) -> Result<usize, Box<dyn std::error::Error>> {
        match self {
            Self::Tcp(stream) => {
                let n = stream.read(buf).await?;
                Ok(n)
            }
            #[cfg(feature = "quic")]
            Self::Quic(_, recv) => {
                let n = recv.read(buf).await?.unwrap_or(0);
                Ok(n)
            }
            Self::Mock(mock) => {
                if mock.frame.is_empty() {
                    return Ok(0);
                }
                let remaining = mock.frame.len().saturating_sub(mock.offset);
                if remaining == 0 {
                    if mock.repeat {
                        mock.offset = 0;
                    } else {
                        return Ok(0);
                    }
                }
                let remaining = mock.frame.len().saturating_sub(mock.offset);
                if remaining == 0 {
                    return Ok(0);
                }
                let to_copy = std::cmp::min(buf.len(), remaining);
                buf[..to_copy].copy_from_slice(&mock.frame[mock.offset..mock.offset + to_copy]);
                mock.offset += to_copy;
                if mock.offset >= mock.frame.len() {
                    if mock.repeat {
                        mock.offset = 0;
                    }
                }
                Ok(to_copy)
            }
        }
    }

    pub async fn connect_tcp(addr: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let stream = tokio::net::TcpStream::connect(addr).await?;
        Ok(Self::Tcp(stream))
    }

    #[cfg(feature = "quic")]
    pub async fn connect_quic(
        remote: &str,
        server_name: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // remote is "host:port"
        let remote_addr: std::net::SocketAddr = remote.parse()?;
        let local_addr: std::net::SocketAddr = "0.0.0.0:0".parse()?;

        let mut endpoint = quinn::Endpoint::client(local_addr)?;
        let client_cfg = quinn::ClientConfig::default();
        endpoint.set_default_client_config(client_cfg);
        let connecting = endpoint.connect(&remote_addr, server_name)?;
        let NewConnection { connection, .. } = connecting.await?;
        let (send, recv) = connection.open_bi().await?;
        Ok(Self::Quic(send, recv))
    }
}
