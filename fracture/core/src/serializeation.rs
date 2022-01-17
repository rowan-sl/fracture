/// This module provides utils for serialization and deserialization of Messages.
pub mod seri {
    use crate::msg;
    use crate::msg::Message;
    use crate::msg::{Header, HeaderParserError, HEADER_LEN};
    use bytes::Bytes;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpStream;

    pub mod res {
        use thiserror::Error;
        use super::{HeaderParserError, Message};

        #[derive(Debug)]
        pub struct GetMessageResponse {
            pub msg: Message,
            pub bytes: usize,
        }

        #[derive(Error, Debug)]
        pub enum GetMessageError {
            #[error("Connection closed")]
            Disconnected,
            #[error("Failed to read from socket: {0}")]
            ReadError(std::io::Error),
            #[error("Failed to parse header: {0}")]
            HeaderParser(HeaderParserError),
            #[error("Failed to deserialize message: {0}")]
            DeserializationError(Box<bincode::ErrorKind>),
        }
    }

    pub struct SerializedMessage {
        message_bytes: Vec<u8>,
        header: Header,
    }

    impl SerializedMessage {
        /// Returns self as bytes, a combination of bolth the Header, and then the Message
        pub fn into_bytes(&mut self) -> Vec<u8> {
            let mut result = bytes2vec(&self.header.to_bytes());
            result.append(&mut self.message_bytes);
            result
        }

        #[must_use]
        pub fn size(&self) -> usize {
            self.message_bytes.len()
        }
    }

    /// Serialize a `api::Message` into a `Vec<u8>` for sending over sockets.
    ///
    /// # Returns
    /// a `SerializedMessage`
    ///
    /// # Errors
    /// if it cannot serialize the message
    pub fn serialize(message: &Message) -> Result<SerializedMessage, Box<bincode::ErrorKind>> {
        let encoded_msg = bincode::serialize(message)?;
        let header = msg::Header::new(&encoded_msg);
        Ok(SerializedMessage {
            message_bytes: encoded_msg,
            header,
        })
    }

    /// Sends a message all in one go, to socket
    ///
    /// equivilant to doing
    /// ```
    /// let encoded = api::seri::serialize(&message).unwrap().as_bytes();
    /// stream.write_all(&encoded).await.unwrap();
    /// ```
    ///
    /// # Panics
    /// if it could not serialize the message or write to the socket
    pub async fn fullsend(msg: &Message, socket: &mut TcpStream) {
        let encoded = self::serialize(msg).unwrap().into_bytes();
        socket.write_all(&encoded).await.unwrap();
    }

    /// Recieve and deserialize a message from a `TcpStream`
    /// asynchronus, and will not return untill is encounters a error, or reads one whole message
    ///
    /// # Cancelation Saftey
    /// this is cancelation safe, however when canceled it will not store what it has read, so the next attempt to read and deserialize __will__ fail
    ///
    /// # Errors
    /// if one of these has happened while decoding:
    /// - the client has disconnected
    /// - there was a error while reading from the socket
    /// - the message could not be deserialized
    pub async fn get_message_from_socket(
        socket: &mut TcpStream,
    ) -> Result<res::GetMessageResponse, res::GetMessageError> {
        //TODO god fix this sin
        drop(socket.readable().await); //heck u and ill see u never

        let mut header_buffer = [0; HEADER_LEN];
        let mut read = 0;
        loop {
            drop(socket.readable().await);
            match socket.try_read(&mut header_buffer) {
                Ok(0) => {
                    return Err(res::GetMessageError::Disconnected);
                }
                Ok(n) => {
                    read += n;
                    if read >= HEADER_LEN {
                        break;
                    }
                }
                Err(err) => {
                    if err.kind() == tokio::io::ErrorKind::WouldBlock {
                        continue;
                    }
                    return Err(res::GetMessageError::ReadError(err));
                }
            }
        }
        let header_r = msg::Header::from_bytes(&vec2bytes(Vec::from(header_buffer)));
        match header_r {
            Ok(header) => {
                let read_amnt = header.size();
                let mut buffer: Vec<u8> = Vec::with_capacity(read_amnt);
                let mut read = 0;
                loop {
                    //TODO goodbye
                    drop(socket.readable().await);
                    match socket.try_read_buf(&mut buffer) {
                        Ok(0) => {
                            return Err(res::GetMessageError::Disconnected);
                        }
                        Ok(n) => {
                            read += n;
                            if read >= read_amnt {
                                //TODO implement parsing the message
                                let deserialized: std::result::Result<
                                    Message,
                                    Box<bincode::ErrorKind>,
                                > = bincode::deserialize(&buffer[..]);
                                match deserialized {
                                    Ok(msg) => {
                                        return Ok(res::GetMessageResponse {
                                            msg,
                                            bytes: buffer.len(),
                                        })
                                    }
                                    Err(err) => {
                                        return Err(res::GetMessageError::DeserializationError(
                                            err,
                                        ));
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            if err.kind() == tokio::io::ErrorKind::WouldBlock {
                                continue;
                            }
                            return Err(res::GetMessageError::ReadError(err));
                        }
                    }
                }
            }
            Err(err) => Err(res::GetMessageError::HeaderParser(err)),
        }
    }

    /// Creates `bytes::Bytes` from `Vec<u8>`
    #[must_use]
    pub fn vec2bytes(data: Vec<u8>) -> Bytes {
        Bytes::from(data)
    }

    /// Create `Vec<u8>` from `bytes::Bytes`
    #[must_use]
    pub fn bytes2vec(data: &Bytes) -> Vec<u8> {
        Vec::from(&data[..])
    }
}
