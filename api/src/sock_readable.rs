use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt;

pub mod stat {
    #[derive(Debug)]
    pub enum SendStatus {
        Sent(usize),
        NoTask,
        Failure(std::io::Error),
        SeriError(crate::seri::res::SerializationError),
    }

    #[derive(Debug)]
    pub struct ReadMessageStatus {
        pub msg: crate::msg::Message,
        pub bytes: usize,
    }

    #[derive(Debug)]
    pub enum ReadMessageError {
        Disconnected,
        ReadError(std::io::Error),
        HeaderParser(crate::msg::HeaderParserError),
        DeserializationError(Box<bincode::ErrorKind>),
    }
}

use stat::{ReadMessageError, ReadMessageStatus, SendStatus};

/// Some utils for dealing with sockets on a struct (reading and writing)
#[async_trait::async_trait]
pub trait SocketUtils {
    /// the socket, since a trait cannot require a feild
    fn get_sock(&mut self) -> &mut TcpStream;
    /// read one message from the socket
    async fn read_msg(&mut self) -> Result<ReadMessageStatus, ReadMessageError> {
        let sock = self.get_sock();
        //TODO god fix this sin
        drop(sock.readable().await); //heck u and ill see u never

        let mut header_buffer = [0; crate::msg::HEADER_LEN];
        let mut read = 0;
        loop {
            drop(sock.readable().await);
            match sock.try_read(&mut header_buffer) {
                Ok(0) => {
                    return Err(ReadMessageError::Disconnected);
                }
                Ok(n) => {
                    read += n;
                    if read >= crate::msg::HEADER_LEN {
                        break;
                    }
                }
                Err(err) => {
                    if err.kind() == tokio::io::ErrorKind::WouldBlock {
                        continue;
                    }
                    return Err(ReadMessageError::ReadError(err));
                }
            }
        }
        let header_r =
            crate::msg::Header::from_bytes(&crate::seri::vec2bytes(Vec::from(header_buffer)));
        match header_r {
            Ok(header) => {
                let read_amnt = header.size();
                let mut buffer: Vec<u8> = Vec::with_capacity(read_amnt);
                let mut read = 0;
                loop {
                    //TODO goodbye
                    drop(sock.readable().await);
                    match sock.try_read_buf(&mut buffer) {
                        Ok(0) => {
                            return Err(ReadMessageError::Disconnected);
                        }
                        Ok(n) => {
                            read += n;
                            if read >= read_amnt {
                                //TODO implement parsing the message
                                let deserialized: std::result::Result<
                                    crate::msg::Message,
                                    Box<bincode::ErrorKind>,
                                > = bincode::deserialize(&buffer[..]);
                                match deserialized {
                                    Ok(msg) => {
                                        return Ok(ReadMessageStatus {
                                            msg,
                                            bytes: buffer.len(),
                                        })
                                    }
                                    Err(err) => {
                                        return Err(ReadMessageError::DeserializationError(err));
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            if err.kind() == tokio::io::ErrorKind::WouldBlock {
                                continue;
                            }
                            return Err(ReadMessageError::ReadError(err));
                        }
                    }
                }
            }
            Err(err) => Err(ReadMessageError::HeaderParser(err)),
        }
    }

    async fn send_message(&mut self, message: crate::msg::Message) -> SendStatus {
        let socket = self.get_sock();
        let serialized_msg_res = crate::seri::serialize(&message);
        match serialized_msg_res {
            Ok(mut serialized_msg) => {
                let wrote_size = serialized_msg.size();
                let b_data = serialized_msg.into_bytes();
                let write_status = socket.write_all(&b_data).await;
                match write_status {
                    Ok(_) => SendStatus::Sent(wrote_size),
                    Err(err) => SendStatus::Failure(err),
                }
            }
            Err(err) => SendStatus::SeriError(err),
        }
    }
}
