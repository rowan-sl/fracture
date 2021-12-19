use tokio::io::{ AsyncWriteExt };
use tokio::net::TcpStream;
use queues::Queue;
use queues::IsQueue;
use queues::queue;
use api::header::HEADER_LEN;


pub mod stati {
    #[derive(Debug)]
    pub enum SendStatus {
        Sent (usize),
        NoTask,
        Failure (std::io::Error),
        SeriError (api::seri::res::SerializationError),
    }

    pub enum MultiSendStatus {
        Worked {
            amnt: u32,
            bytes: u128,
        },
        Failure (SendStatus),
        NoTask,
    }

    #[derive(Debug)]
    #[allow(dead_code)]
    pub enum UpdateStatus {
        Sucsess,
        Noop
    }

    #[derive(Debug)]
    pub struct ReadMessageStatus {
        pub msg: api::msg::Message,
        pub bytes: usize,
    }

    #[derive(Debug)]
    pub enum ReadMessageError {
        Disconnected,
        ReadError ( std::io::Error ),
        HeaderParser ( api::header::HeaderParserError ),
        DeserializationError ( Box<bincode::ErrorKind> ),
    }

    #[derive(Debug)]
    pub enum UpdateReadStatus {
        Disconnected,
        Sucsess,
        ReadError ( ReadMessageError ),
    }
}

pub struct ClientInterface {
    to_send: Queue<api::msg::Message>,
    incoming: Queue<api::msg::Message>,
    server_name: String,
    socket: TcpStream,
}

impl ClientInterface {
    pub fn new(sock: TcpStream, name: String) -> ClientInterface {
        ClientInterface {
            to_send: queue![],
            incoming: queue![],
            socket: sock,
            server_name: name,
        }
    }

    /// Close the socket, optionaly notifying the client why it is being disconnected, and sending all queued messages
    pub async fn close(&mut self, reason: String, expect_alredy_shutdown: bool) {
        //TODO make it send a shutdown message and clear queue and stuff
        if !expect_alredy_shutdown {
            // only send queued messages if it isnt alredy shutdown (or at least know it is)
            self.queue_message(
                api::msg::Message {
                    data: api::msg::MessageVarient::ServerForceDisconnect {
                        reason: api::msg::types::ServerForceDisconnectReason::Closed,
                        close_message: reason
                    }
                }
            ).unwrap();
            self.send_all_queued().await;
        }
        match self.socket.shutdown().await {
            Ok(_) => {},
            Err(err) => {
                //Socket is already shutdown
                if err.kind() == std::io::ErrorKind::NotConnected {
                    if !expect_alredy_shutdown {
                        println!("Socket was expected to be open, but it was not!");
                    }
                } else {
                    println!("Error whilst closing connection: {:#?}", err);
                }
            }
        };
        //& bye bye me
        drop(self);
    }

    /// Get the address of the client connected
    pub fn get_client_addr(&mut self) -> Result<std::net::SocketAddr, std::io::Error>{
        self.socket.peer_addr()
    }

    /// Send one message from the queue
    async fn send_queued_message(&mut self) -> stati::SendStatus {
        let value = self.to_send.remove();
        match value {
            Ok(v) => {
                let serialized_msg_res = api::seri::serialize(&v);
                match serialized_msg_res {
                    Ok(mut serialized_msg) => {
                        let wrote_size = serialized_msg.size();
                        let b_data = serialized_msg.as_bytes();
                        let write_status = self.socket.write_all(&b_data).await;
                        match write_status {
                            Ok(_) => stati::SendStatus::Sent (wrote_size),
                            Err(err) => stati::SendStatus::Failure (err)
                        }
                    }
                    Err(err) => {
                        stati::SendStatus::SeriError (err)
                    }
                }
            },
            Err(_err) => {
                stati::SendStatus::NoTask
            }//Do nothing as there is nothing to do ;)
        }
    }

    /// Send all queued messages
    ///
    /// ! THIS IS NOT CANCELATION SAFE!!!!
    pub async fn send_all_queued(&mut self) -> stati::MultiSendStatus {
        let mut sent = 0;
        let mut sent_bytes = 0;
        loop {
            let res = self.send_queued_message().await;
            match res {
                stati::SendStatus::NoTask => {
                    if sent == 0 {
                        return stati::MultiSendStatus::NoTask;
                    } else {
                        return stati::MultiSendStatus::Worked {
                            amnt: sent,
                            bytes:sent_bytes,
                        };
                    }
                },
                stati::SendStatus::Sent (stat) => {
                    sent += 1;
                    sent_bytes += stat as u128;
                },
                stati::SendStatus::Failure (err) => {
                    return stati::MultiSendStatus::Failure (stati::SendStatus::Failure (err));
                },
                stati::SendStatus::SeriError (err) => {
                    return stati::MultiSendStatus::Failure (stati::SendStatus::SeriError (err));
                }
            }
        }
    }

    /// Queue a message for sending.
    /// Returns the value as if .add(msg) had been called on the msg queue.
    pub fn queue_message(&mut self, msg: api::msg::Message) -> Result<Option<api::msg::Message>, &str> {
        self.to_send.add(msg)
    }

    /// Process a message, and queue apropreate responses for sending
    pub async fn process(&mut self, msg: api::msg::Message) {
        use api::msg::*;
        match msg.data {
            MessageVarient::Ping => {
                self.queue_message(
                    Message {
                        data: MessageVarient::Pong
                    }
                ).unwrap();
            },
            _ => {}
        };
    }

    /// Handles the reading message half of updating the client.
    /// for the most part it handles errors that occur in it, but it will return info for some situations,
    /// Like disconnects.
    ///
    /// This IS cancelation safe
    pub async fn update_read(&mut self) -> stati::UpdateReadStatus {
        let read = self.read_msg().await;
        match read {
            Ok(stat) => {
                let added = self.incoming.add(stat.msg);
                if added.is_err() {
                    panic!("Could not queue message for sending!");
                } else {
                    return stati::UpdateReadStatus::Sucsess;
                }
            },
            Err(err) => {
                match err {
                    stati::ReadMessageError::Disconnected => {
                        return stati::UpdateReadStatus::Disconnected;
                    },
                    oerr => {
                        return stati::UpdateReadStatus::ReadError ( oerr );
                    }
                };
            }
        };
    }

    /// read one message from the socket
    pub async fn read_msg(&mut self) -> Result<stati::ReadMessageStatus, stati::ReadMessageError> {
        //TODO god fix this sin
        drop(self.socket.readable().await);//heck u and ill see u never

        let mut header_buffer = [0; HEADER_LEN];
        let mut read = 0;
        loop {
            drop(self.socket.readable().await);
            match self.socket.try_read(&mut header_buffer) {
                Ok(0) => {
                    return Err(stati::ReadMessageError::Disconnected);
                },
                Ok(n) => {
                    read += n;
                    if read >= HEADER_LEN {
                        break;
                    }
                },
                Err(err) => {
                    if err.kind() == tokio::io::ErrorKind::WouldBlock {
                        continue;
                    }
                    return Err(stati::ReadMessageError::ReadError (err))
                },
            }
        }
        drop(read);
        let header_r = api::header::MessageHeader::from_bytes(api::seri::vec2bytes(Vec::from(header_buffer)));
        match header_r {
            Ok(header) => {
                let read_amnt = header.size();
                let mut buffer: Vec<u8> = Vec::with_capacity(read_amnt);
                let mut read = 0;
                loop {
                    //TODO goodbye
                    drop(self.socket.readable().await);
                    match self.socket.try_read_buf(&mut buffer) {
                        Ok(0) => {
                            return Err(stati::ReadMessageError::Disconnected);
                        },
                        Ok(n) => {
                            read += n;
                            if read >= read_amnt {
                                //TODO implement parsing the message
                                let deserialized: std::result::Result<api::msg::Message, Box<bincode::ErrorKind>> = bincode::deserialize(&buffer[..]);
                                match deserialized {
                                    Ok(msg) => {
                                        return Ok(stati::ReadMessageStatus {msg: msg, bytes: buffer.len()})
                                    },
                                    Err(err) => {
                                        return Err(stati::ReadMessageError::DeserializationError (err));
                                    }
                                }
                            }
                        },
                        Err(err) => {
                            if err.kind() == tokio::io::ErrorKind::WouldBlock {
                                continue;
                            }
                            return Err(stati::ReadMessageError::ReadError (err))
                        },
                    }
                }
            },
            Err(err) => {
                return Err(stati::ReadMessageError::HeaderParser (err));
            }
        }
    }

    /// Process all messages
    pub async fn update_process_all(&mut self) {
        loop {
            match self.incoming.remove() {
                Ok(msg) => {
                    self.process(msg).await;
                }
                Err(_) => {
                    break;
                }
            }
        }
    }
}