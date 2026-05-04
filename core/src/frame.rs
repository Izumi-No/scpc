#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    Hello = 0x01,
    Auth = 0x02,
    AuthOk = 0x03,
    Ping = 0x04,
    Pong = 0x05,
    Send = 0x06,
    Ack = 0x07,
    Error = 0x08,
    Resume = 0x09,
    Close = 0x0A,
    Meta = 0x0B,
}

impl TryFrom<u8> for FrameType {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, &'static str> {
        match value {
            0x01 => Ok(Self::Hello),
            0x02 => Ok(Self::Auth),
            0x03 => Ok(Self::AuthOk),
            0x04 => Ok(Self::Ping),
            0x05 => Ok(Self::Pong),
            0x06 => Ok(Self::Send),
            0x07 => Ok(Self::Ack),
            0x08 => Ok(Self::Error),
            0x09 => Ok(Self::Resume),
            0x0A => Ok(Self::Close),
            0x0B => Ok(Self::Meta),
            _ => Err("Invalid frame type"),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    UnsupportedVersion = 1001,
    AuthFailed = 1002,
    InvalidFrame = 1003,
    Unauthorized = 1004,
    SessionExpired = 1005,
    RateLimited = 1006,
}

impl TryFrom<u16> for ErrorCode {
    type Error = &'static str;

    fn try_from(value: u16) -> Result<Self, &'static str> {
        match value {
            1001 => Ok(Self::UnsupportedVersion),
            1002 => Ok(Self::AuthFailed),
            1003 => Ok(Self::InvalidFrame),
            1004 => Ok(Self::Unauthorized),
            1005 => Ok(Self::SessionExpired),
            1006 => Ok(Self::RateLimited),
            _ => Err("Invalid error code"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QosLevel {
    Qos0 = 0,
    Qos1 = 1,
    Qos2 = 2,
}

impl TryFrom<u8> for QosLevel {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, &'static str> {
        match value {
            0 => Ok(Self::Qos0),
            1 => Ok(Self::Qos1),
            2 => Ok(Self::Qos2),
            _ => Err("Invalid QoS level"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameHeader {
    pub version: u8,
    pub frame_type: FrameType,
    pub flags: u16,
    pub stream_id: u32,
    pub message_id: u32,
    pub payload_len: u32,
}

impl FrameHeader {
    pub fn qos(&self) -> QosLevel {
        // Assume the lowest 2 bits of flags represent the QoS level
        let qos_bits = (self.flags & 0b0000_0011) as u8;
        QosLevel::try_from(qos_bits).unwrap_or(QosLevel::Qos0)
    }

    pub fn set_qos(&mut self, qos: QosLevel) {
        // Clear the lowest 2 bits and set the new QoS bits
        self.flags = (self.flags & !0b0000_0011) | (qos as u16);
    }
}

pub fn parse_frame(buf: &[u8]) -> Option<(FrameHeader, &[u8], &[u8])> {
    if buf.len() < 16 {
        return None;
    }

    let version = buf[0];
    let frame_type = FrameType::try_from(buf[1]).ok()?;
    let flags = u16::from_be_bytes([buf[2], buf[3]]);
    let stream_id = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
    let message_id = u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]);
    let payload_len = u32::from_be_bytes([buf[12], buf[13], buf[14], buf[15]]);

    let total_len = 16 + payload_len as usize;
    if buf.len() < total_len {
        return None; // Partial frame
    }

    let header = FrameHeader {
        version,
        frame_type,
        flags,
        stream_id,
        message_id,
        payload_len,
    };

    let payload = &buf[16..total_len];
    let remaining = &buf[total_len..];

    Some((header, payload, remaining))
}

pub fn encode_header(header: &FrameHeader, out: &mut [u8; 16]) {
    out[0] = header.version;
    out[1] = header.frame_type as u8;
    out[2..4].copy_from_slice(&header.flags.to_be_bytes());
    out[4..8].copy_from_slice(&header.stream_id.to_be_bytes());
    out[8..12].copy_from_slice(&header.message_id.to_be_bytes());
    out[12..16].copy_from_slice(&header.payload_len.to_be_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qos_level_try_from() {
        // Arrange, Act & Assert
        assert_eq!(QosLevel::try_from(0).unwrap(), QosLevel::Qos0);
        assert_eq!(QosLevel::try_from(1).unwrap(), QosLevel::Qos1);
        assert_eq!(QosLevel::try_from(2).unwrap(), QosLevel::Qos2);
        assert!(QosLevel::try_from(3).is_err());
    }

    #[test]
    fn test_frame_type_try_from() {
        // Arrange, Act & Assert
        assert_eq!(FrameType::try_from(0x01).unwrap(), FrameType::Hello);
        assert_eq!(FrameType::try_from(0x06).unwrap(), FrameType::Send);
        assert!(FrameType::try_from(0x0C).is_err());
    }

    #[test]
    fn test_error_code_try_from() {
        // Arrange, Act & Assert
        assert_eq!(
            ErrorCode::try_from(1001).unwrap(),
            ErrorCode::UnsupportedVersion
        );
        assert_eq!(ErrorCode::try_from(1004).unwrap(), ErrorCode::Unauthorized);
        assert!(ErrorCode::try_from(9999).is_err());
    }

    #[test]
    fn test_frame_header_qos_manipulation() {
        // Arrange
        let mut header = FrameHeader {
            version: 1,
            frame_type: FrameType::Send,
            flags: 0b1111_1100, // Top bits set, bottom 2 clear
            stream_id: 1,
            message_id: 1,
            payload_len: 0,
        };

        // Act
        header.set_qos(QosLevel::Qos2);

        // Assert
        assert_eq!(header.qos(), QosLevel::Qos2);
        assert_eq!(header.flags, 0b1111_1110);
    }

    #[test]
    fn test_encode_and_parse_frame() {
        // Arrange
        let header = FrameHeader {
            version: 1,
            frame_type: FrameType::Ping,
            flags: 2, // QoS 2
            stream_id: 42,
            message_id: 100,
            payload_len: 4,
        };
        let payload = [0xDE, 0xAD, 0xBE, 0xEF];

        let mut buf = vec![0u8; 16 + 4 + 10]; // 10 bytes extra padding

        // Act
        let mut header_buf = [0u8; 16];
        encode_header(&header, &mut header_buf);
        buf[0..16].copy_from_slice(&header_buf);
        buf[16..20].copy_from_slice(&payload);

        let parsed = parse_frame(&buf);

        // Assert
        assert!(parsed.is_some());
        let (parsed_header, parsed_payload, remaining) = parsed.unwrap();

        assert_eq!(parsed_header, header);
        assert_eq!(parsed_payload, &payload);
        assert_eq!(remaining.len(), 10);
    }

    #[test]
    fn test_parse_frame_incomplete() {
        // Arrange
        let header = FrameHeader {
            version: 1,
            frame_type: FrameType::Ping,
            flags: 0,
            stream_id: 0,
            message_id: 0,
            payload_len: 10,
        };
        let mut buf = [0u8; 16];
        encode_header(&header, &mut buf);

        // Act
        let parsed = parse_frame(&buf);

        // Assert
        assert!(
            parsed.is_none(),
            "Should fail because payload length is 10 but buffer only has 16 bytes"
        );
    }
}
