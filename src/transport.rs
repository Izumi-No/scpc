use crate::frame::{FrameHeader, encode_header};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// A tiny in-memory transport used for simulation and testing.
/// It returns the same pre-built frame on every call to `receive_frame` if `repeat` is true.
struct MockTransport {
    frame: Arc<Vec<u8>>,
    offset: usize,
    repeat: bool,
    // store sent frames for inspection (not used in benchmarks but helpful for tests)
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

pub enum Transport {
    Tcp(tokio::net::TcpStream),
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
            Self::Quic(send, _) => {
                send.write_all(&frame).await?;
            }
            Self::Mock(mock) => {
                // record the sent frame; no async IO needed
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
            Self::Quic(_, recv) => {
                let n = recv.read(buf).await?.unwrap_or(0);
                Ok(n)
            }
            Self::Mock(mock) => {
                // Ensure there is data to read
                if mock.frame.is_empty() {
                    // no data -> simulate closed connection
                    return Ok(0);
                }
                // copy as much as fits from the current frame (respecting offset)
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
}
