pub mod msg {
    use serde::Serialize;
    use serde::Deserialize;

    pub mod types {
        use super::*;
        #[derive(Deserialize, Serialize, Debug, Clone)]
        pub enum ServerForceDisconnectReason {
            Closed,
            Kicked,
            IpBanned,
            InvalidAuthentication,
        }
    }

    #[derive(Deserialize, Serialize, Debug, Clone)]
    pub enum MessageVarient {
        DisconnectMessage {},
        /// Client sends this as its first message
        ConnectMessage {
            name: String
        },
        Ping,
        Pong,
        /// Server has kicked/disconnected the client for some reason
        ServerForceDisconnect {
            reason: types::ServerForceDisconnectReason,
            close_message: String
        }
    }

    /// Hello, hello, can you hear me?
    #[derive(Deserialize, Serialize, Debug, Clone)]
    pub struct Message {
        pub data: MessageVarient
    }
}


pub mod header {
    use bytes::BufMut;
    use bytes::Buf;


    #[derive(Debug)]
    pub enum HeaderParserError {
        InvalidLength,
        InvalidPrefix,
        InvalidMsgSize,
        InvalidSuffix,
    }

    pub const HEADER_LEN: usize =
    b"ds-header".len() +
    8 + // because you cant do u64::BITS/8, since its a u32 not usize :/
    b"header-end".len();

    /// Header for a Message, includes data about its length
    ///
    /// Designed to be send before each Message, and have a fixed length so the other program
    /// knows how many by bytes to read
    pub struct MessageHeader {
        msg_size: u64
    }

    impl MessageHeader {
        /// Creates a new MessageHeader based off of msg
        ///
        /// Can panic, if it cannot convert msg.len() to u64
        pub fn new(msg: &Vec<u8>) -> MessageHeader {
            return MessageHeader {
                msg_size: u64::try_from(msg.len()).unwrap()
            }
        }

        /// Creates a header of size 0
        pub fn blank() -> MessageHeader {
            MessageHeader {
                msg_size: 0
            }
        }

        /// Get the size of the message
        /// Can panic, if it cannot convert the msg_size to usize
        pub fn size(&self) -> usize {
            return usize::try_from(self.msg_size).unwrap();
        }

        /// Converts the message header to bytes, and then drops it
        pub fn to_bytes(&mut self) -> bytes::Bytes {
            let mut result = bytes::BytesMut::new();
            result.put(&b"ds-header"[..]);
            result.put_u64(self.msg_size);
            result.put(&b"header-end"[..]);
            drop(self);
            result.freeze()
        }

        /// Takes bytes::Bytes and creates a header
        pub fn from_bytes(header: bytes::Bytes) -> Result<MessageHeader, HeaderParserError> {
            if header.len() != HEADER_LEN {
                return Err(HeaderParserError::InvalidLength);
            } else {
                let header_start = header.slice(..b"ds-header".len());
                let mut message_size = header.slice(b"ds-header".len()..b"ds-header".len()+8);
                let header_end = header.slice(b"ds-header".len()+8..);
                if &header_start[..] != b"ds-header" {
                    return Err(HeaderParserError::InvalidPrefix);
                }
                if &header_end[..] != b"header-end" {
                    return Err(HeaderParserError::InvalidSuffix);
                }
                Ok(
                    MessageHeader{
                        msg_size: message_size.get_u64()
                    }
                )
            }
        }
    }
}