use crate::frame::{FrameHeader, encode_header};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub enum Transport {
    Tcp(tokio::net::TcpStream),
    Quic(quinn::SendStream, quinn::RecvStream),
}

impl Transport {
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
        }
    }
}
