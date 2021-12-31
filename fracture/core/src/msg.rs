use serde::Deserialize;
use serde::Serialize;

pub mod types {
    use super::{Deserialize, Serialize};
    #[derive(Deserialize, Serialize, Debug, Clone)]
    pub enum ServerDisconnectReason {
        Closed,
        ClientRequestedDisconnect,
        InvalidConnectionSequence,
    }

    #[derive(Deserialize, Serialize, Debug, Clone)]
    pub struct UserId {
        //TODO implement this? posibly w/ constructor and stuff
    }

    //TODO this
    /// Information about what is happening with one user
    /// for example, it could be used to say that one user changed its name to something else
    #[derive(Deserialize, Serialize, Debug, Clone)]
    pub enum UserNameUpdate {
        NameChange { id: UserId, new: String },

        NewUser { id: UserId, name: String },

        UserLeft { id: UserId },
    }

    //TODO this
    /// Used in `ServerInfo` to specify if the connection has been accepted or refused
    #[derive(Deserialize, Serialize, Debug, Clone)]
    pub enum ConnectionStatus {
        Connected,
        Refused { reason: String },
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub enum MessageVarient {
    //TODO implement
    /// Client sends this as its last message when it leaves
    DisconnectMessage {},

    /// Client sends this as its first message
    ConnectMessage {
        name: String,
    },

    //TODO this
    /// Server sends this after client sends `ConnectMessage`, with info about the server
    /// should not be sent any other time
    ServerInfo {
        server_name: String,
        conn_status: types::ConnectionStatus,
        connected_users: Vec<types::UserNameUpdate>,
        your_uuid: u128,
    },

    //TODO this
    /// Update to server info
    /// can be sent at any time
    ServerInfoUpdate {
        name: Option<String>,
        user_updates: Vec<types::UserNameUpdate>,
    },

    /// Request a simple response
    Ping,

    /// The simple response
    Pong,

    /// Server has kicked/disconnected the client for some reason
    ServerForceDisconnect {
        reason: types::ServerDisconnectReason,
        close_message: String,
    },

    /// Client sends this to server
    ClientSendChat {
        content: String,
    },

    /// Server sends this to the client
    ServerSendChat {
        content: String,
        author: String,
        author_uuid: u128,
    },

    TestMessage {},
    TestMessageResponse {},
}

/// Hello, hello, can you hear me?
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Message {
    pub data: MessageVarient,
}

use bytes::Buf;
use bytes::BufMut;

#[derive(Debug)]
pub enum HeaderParserError {
    InvalidLength,
    InvalidPrefix,
    InvalidMsgSize,
    InvalidSuffix,
}

pub const HEADER_LEN: usize = b"ds-header".len() +
8 + // because you cant do u64::BITS/8, since its a u32 not usize :/
b"header-end".len();

/// Header for a `Message`, includes data about its length
///
/// Designed to be send before each Message, and have a fixed length so the other program
/// knows how many by bytes to read
pub struct Header {
    msg_size: u64,
}

impl Header {
    /// Creates a new `Header` based off of msg
    ///
    /// # Panics
    /// if it cannot convert msg.len() to u64
    #[must_use]
    pub fn new(msg: &[u8]) -> Self {
        Self {
            msg_size: u64::try_from(msg.len()).unwrap(),
        }
    }

    /// Creates a header of size 0
    #[must_use]
    pub const fn blank() -> Self {
        Self { msg_size: 0 }
    }

    /// Get the size of the message
    ///
    /// # Panics
    /// if it cannot convert the `msg_size` to usize
    #[must_use]
    pub fn size(&self) -> usize {
        usize::try_from(self.msg_size).unwrap()
    }

    /// Converts the message header to `Bytes`
    #[must_use]
    pub fn to_bytes(&mut self) -> bytes::Bytes {
        let mut result = bytes::BytesMut::new();
        result.put(&b"ds-header"[..]);
        result.put_u64(self.msg_size);
        result.put(&b"header-end"[..]);
        result.freeze()
    }

    /// Takes `bytes::Bytes` and creates a `Header`
    ///
    /// # Errors
    /// if the header is invalid, returning what part was invalid
    pub fn from_bytes(header: &bytes::Bytes) -> Result<Self, HeaderParserError> {
        if header.len() == HEADER_LEN {
            let header_start = header.slice(..b"ds-header".len());
            let mut message_size = header.slice(b"ds-header".len()..b"ds-header".len() + 8);
            let header_end = header.slice(b"ds-header".len() + 8..);
            if &header_start[..] != b"ds-header" {
                return Err(HeaderParserError::InvalidPrefix);
            }
            if &header_end[..] != b"header-end" {
                return Err(HeaderParserError::InvalidSuffix);
            }
            Ok(Self {
                msg_size: message_size.get_u64(),
            })
        } else {
            Err(HeaderParserError::InvalidLength)
        }
    }
}
