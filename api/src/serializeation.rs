/// This module provides utils for serialization and deserialization of Messages.
pub mod seri {
    use crate::messages::header::*;
    use crate::messages::msg::*;
    use bytes::Bytes;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpStream;

    pub mod res {
        use super::{HeaderParserError, Message};

        #[derive(Debug)]
        pub enum SerializationError {
            Generic,
            BincodeErr(Box<bincode::ErrorKind>),
        }

        #[derive(Debug)]
        pub struct GetMessageResponse {
            pub msg: Message,
            pub bytes: usize,
        }

        #[derive(Debug)]
        pub enum GetMessageError {
            Disconnected,
            ReadError,
            Error(std::io::Error),
            HeaderParser(HeaderParserError),
            DeserializationError(Box<bincode::ErrorKind>),
        }
    }

    pub struct SerializedMessage {
        message_bytes: Vec<u8>,
        header: MessageHeader,
    }

    impl SerializedMessage {
        /// Returns self as bytes, a combination of bolth the Header, and then the Message, droping the SerializedMessage it was called on
        pub fn into_bytes(&mut self) -> Vec<u8> {
            let mut result = bytes2vec(self.header.to_bytes());
            result.append(&mut self.message_bytes);
            drop(self);
            result
        }

        pub fn size(&self) -> usize {
            self.message_bytes.len()
        }
    }

    /// Serialize a api::Message into a Vec<u8> for sending over sockets.
    ///
    /// This also produces a header which can be turned into bytes
    pub fn serialize(message: &Message) -> Result<SerializedMessage, res::SerializationError> {
        let encoded_msg = bincode::serialize(message);
        match encoded_msg {
            Ok(dat) => {
                let header = MessageHeader::new(&dat);
                Ok(SerializedMessage {
                    message_bytes: dat,
                    header: header,
                })
            }
            Err(err) => Err(res::SerializationError::BincodeErr(err)),
        }
    }

    /// Sends a message all in one go, to socket
    ///
    /// equivilant to doing
    /// `let encoded = api::seri::serialize(&message).unwrap().as_bytes();`
    /// `stream.write_all(&encoded).await.unwrap();`
    /// including the part where it will panic, so if that is not desierable, DO IT DIFFERENTLY
    /// this is mostly meant as a quick way to test new things, and not intended for final use
    pub async fn fullsend(msg: &Message, socket: &mut TcpStream) {
        let encoded = self::serialize(&msg).unwrap().into_bytes();
        socket.write_all(&encoded).await.unwrap();
    }

    /// Recieve and deserialize a message from a TcpStream
    /// asynchronus, and will not return untill is encounters a error, or reads one whole message
    ///
    /// This is probably cancelation safe
    ///
    /// If you want something that is non-blocking, then use the SocketMessageReader struct
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
                    return Err(res::GetMessageError::Error(err));
                }
            }
        }
        drop(read);
        let header_r = MessageHeader::from_bytes(vec2bytes(Vec::from(header_buffer)));
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
                                            msg: msg,
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
                            return Err(res::GetMessageError::Error(err));
                        }
                    }
                }
            }
            Err(err) => {
                return Err(res::GetMessageError::HeaderParser(err));
            }
        }
    }

    /// Creates `bytes::Bytes` from `Vec<u8>`
    pub fn vec2bytes(data: Vec<u8>) -> Bytes {
        Bytes::from(data)
    }

    /// Create `Vec<u8>` from `bytes::Bytes`
    pub fn bytes2vec(data: Bytes) -> Vec<u8> {
        return Vec::from(&data[..]);
    }
}
