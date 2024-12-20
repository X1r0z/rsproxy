use std::io::{Error, Result};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::tcp::{self, NetStream};

pub async fn handle_connection(stream: NetStream) -> Result<()> {
    let (mut reader, mut writer) = stream.split();

    // 1. auth negotiation
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf).await?;

    if buf[0] != 0x05 {
        return Err(Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid SOCKS5 protocol version",
        ));
    }

    let nmethods = buf[1] as usize;
    let mut methods = vec![0u8; nmethods];
    reader.read_exact(&mut methods).await?;

    // no auth needed
    writer.write_all(&[0x05, 0x00]).await?;

    // 2. handle request
    let mut header = [0u8; 4];
    reader.read_exact(&mut header).await?;

    if header[0] != 0x05 {
        return Err(Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid SOCKS5 request",
        ));
    }

    if header[1] != 0x01 {
        return Err(Error::new(
            std::io::ErrorKind::Unsupported,
            "Only CONNECT command supported",
        ));
    }

    let addr = match header[3] {
        0x01 => {
            // IPv4
            let mut addr = [0u8; 4];
            reader.read_exact(&mut addr).await?;
            let mut port = [0u8; 2];
            reader.read_exact(&mut port).await?;
            format!(
                "{}.{}.{}.{}:{}",
                addr[0],
                addr[1],
                addr[2],
                addr[3],
                u16::from_be_bytes(port)
            )
        }
        0x03 => {
            // domain
            let len = reader.read_u8().await? as usize;
            let mut domain = vec![0u8; len];
            reader.read_exact(&mut domain).await?;
            let mut port = [0u8; 2];
            reader.read_exact(&mut port).await?;
            format!(
                "{}:{}",
                String::from_utf8_lossy(&domain),
                u16::from_be_bytes(port)
            )
        }
        0x04 => {
            return Err(Error::new(
                std::io::ErrorKind::Unsupported,
                "IPv6 address not supported",
            ));
        }
        _ => {
            return Err(Error::new(
                std::io::ErrorKind::Unsupported,
                "Unsupported address type",
            ))
        }
    };

    // 3. connect to the target server
    let target = NetStream::Tcp(match TcpStream::connect(&addr).await {
        Ok(stream) => stream,
        Err(e) => {
            writer
                .write_all(&[0x05, 0x04, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
                .await?;
            return Err(e.into());
        }
    });

    // 4. send success response
    writer
        .write_all(&[0x05, 0x00, 0x00, 0x01, 0, 0, 0, 0, 0, 0])
        .await?;

    // 5. forward data
    tcp::handle_forward_splitted(reader, writer, target).await
}
