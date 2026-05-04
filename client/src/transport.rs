use anyhow::Result;
use scsp_core::frame::{FrameHeader, encode_header};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[cfg(feature = "quic")]
use quinn::{ClientConfig, Endpoint, NewConnection};
#[cfg(feature = "quic")]
use std::sync::Arc;

pub enum ClientTransport {
    Tcp(TcpStream),
    #[cfg(feature = "quic")]
    Quic(quinn::SendStream, quinn::RecvStream),
}

impl ClientTransport {
    pub async fn send_frame(&mut self, header: &FrameHeader, payload: &[u8]) -> Result<()> {
        let mut buf = [0u8; 16];
        encode_header(header, &mut buf);
        match self {
            ClientTransport::Tcp(stream) => {
                stream.write_all(&buf).await?;
                if !payload.is_empty() {
                    stream.write_all(payload).await?;
                }
                Ok(())
            }
            #[cfg(feature = "quic")]
            ClientTransport::Quic(send, _recv) => {
                send.write_all(&buf).await?;
                if !payload.is_empty() {
                    send.write_all(payload).await?;
                }
                Ok(())
            }
        }
    }

    pub async fn receive_frame(&mut self, buf: &mut [u8]) -> Result<usize> {
        match self {
            ClientTransport::Tcp(stream) => {
                let n = stream.read(buf).await?;
                Ok(n)
            }
            #[cfg(feature = "quic")]
            ClientTransport::Quic(_send, recv) => {
                // quinn::RecvStream implements AsyncRead
                let n = recv.read(buf).await?;
                Ok(n.unwrap_or(0))
            }
        }
    }

    pub async fn connect_tcp(addr: &str) -> Result<Self> {
        let stream = TcpStream::connect(addr).await?;
        Ok(ClientTransport::Tcp(stream))
    }

    #[cfg(feature = "quic")]
    pub async fn connect_quic(remote: &str, server_name: &str) -> Result<Self> {
        // remote is "host:port"
        let remote_addr: std::net::SocketAddr = remote.parse()?;
        let local_addr: std::net::SocketAddr = "0.0.0.0:0".parse()?;

        let mut endpoint_builder = Endpoint::client(local_addr)?;
        // Use default native root certs
        let mut client_cfg = ClientConfig::default();
        endpoint_builder.set_default_client_config(client_cfg.clone());
        let (endpoint, _) = endpoint_builder.bind(&local_addr)?;

        let connecting = endpoint.connect(&remote_addr, server_name)?;
        let NewConnection { connection, .. } = connecting.await?;
        let (send, recv) = connection.open_bi().await?;
        Ok(ClientTransport::Quic(send, recv))
    }
}
