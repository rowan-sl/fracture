use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

pub mod stat {
    use thiserror::Error;

    #[derive(Debug)]
    pub enum SendStatus {
        Sent(usize),
        NoTask,
    }

    #[derive(Error, Debug)]
    pub enum SendError {
        #[error("Failed to send message: {0}")]
        Failure(#[from] std::io::Error),
        #[error("Failed to serialize message: {0}")]
        SeriError(#[from] Box<bincode::ErrorKind>),
    }

    #[derive(Debug)]
    pub struct ReadMessageStatus {
        pub msg: crate::msg::Message,
        pub bytes: usize,
    }

    #[derive(Error, Debug)]
    pub enum ReadMessageError {
        #[error("Disconnected while writing message!")]
        Disconnected,
        #[error("Failed to read from socket: {0}")]
        ReadError(std::io::Error),
        #[error("Failed to parse msg header: {0}")]
        HeaderParser(#[from] crate::msg::HeaderParserError),
        #[error("Failed to deserialize message: {0}")]
        DeserializationError(#[from] Box<bincode::ErrorKind>),
    }
}

use stat::{ReadMessageError, ReadMessageStatus, SendStatus};

use self::stat::SendError;

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
        let header =
            crate::msg::Header::from_bytes(&crate::seri::vec2bytes(Vec::from(header_buffer)))?;
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
                        let msg: crate::msg::Message = bincode::deserialize(&buffer[..])?;
                        if crate::conf::SOCK_DBG {
                            println!("Receved:\n{:#?}", msg);
                        }
                        return Ok(ReadMessageStatus {
                            msg,
                            bytes: buffer.len(),
                        });
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

    async fn send_message(&mut self, message: crate::msg::Message) -> Result<SendStatus, SendError> {
        if crate::conf::SOCK_DBG {
            println!("Sent:\n{:#?}", message);
        }
        let socket = self.get_sock();
        let mut serialized_msg = crate::seri::serialize(&message)?;
        let wrote_size = serialized_msg.size();
        let b_data = serialized_msg.into_bytes();
        let write_status = socket.write_all(&b_data).await;
        match write_status {
            Ok(_) => Ok(SendStatus::Sent(wrote_size)),
            Err(err) => Err(SendError::Failure(err)),
        }
    }
}
