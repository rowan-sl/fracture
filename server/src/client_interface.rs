use tokio::io::{ AsyncWriteExt };
use tokio::net::TcpStream;
use queues::Queue;
use queues::IsQueue;
use queues::queue;
use api::seri::res::*;
// use tokio::sync::broadcast::*;
use api::header::HEADER_LEN;

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
pub enum UpdateError {
}

pub struct ClientInterface {
    to_send: Queue<api::msg::Message>,
    incoming: Queue<api::msg::Message>,
    socket: TcpStream,
}

#[derive(Debug)]
pub struct UpdateReadStatus {
    pub msg: api::msg::Message,
    pub bytes: usize,
}

#[derive(Debug)]
pub enum UpdateReadError {
    Disconnected,
    ReadError ( std::io::Error ),
    HeaderParser ( api::header::HeaderParserError ),
    DeserializationError ( Box<bincode::ErrorKind> ),
}

impl ClientInterface {
    pub fn new(sock: TcpStream) -> ClientInterface {
        ClientInterface {
            to_send: queue![],
            incoming: queue![],
            socket: sock,
        }
    }

    //TODO remove this
    #[allow(dead_code)]
    pub async fn close(&mut self, reason: String) {
        //TODO make it send a shutdown message and clear queue and stuff
        self.queue_message(api::msg::Message {data: api::msg::MessageVarient::ServerClosed {close_message: reason}}).unwrap();
        self.send_all_queued().await;
        match self.socket.shutdown().await {
            Ok(_) => {},
            Err(err) => {
                //Socket is already shutdown
                // panic!("Cound not shutdown socket!\n{}", err);
                println!("Socket already shut down, closed with error {:?}", err);
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
    async fn send_queued_message(&mut self) -> SendStatus {
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
                            Ok(_) => SendStatus::Sent(wrote_size),
                            Err(err) => SendStatus::Failure(err)
                        }
                    }
                    Err(err) => {
                        SendStatus::SeriError (err)
                    }
                }
            },
            Err(_err) => {
                SendStatus::NoTask
            }//Do nothing as there is nothing to do ;)
        }
    }

    /// Send all queued messages
    /// 
    /// ! THIS IS NOT CANCELATION SAFE!!!!
    pub async fn send_all_queued(&mut self) -> MultiSendStatus {
        let mut sent = 0;
        let mut sent_bytes = 0;
        loop {
            let res = self.send_queued_message().await;
            match res {
                SendStatus::NoTask => {
                    if sent == 0 {
                        return MultiSendStatus::NoTask;
                    } else {
                        return MultiSendStatus::Worked {
                            amnt: sent,
                            bytes:sent_bytes,
                        };
                    }
                },
                SendStatus::Sent (stat) => {
                    sent += 1;
                    sent_bytes += stat as u128;
                },
                SendStatus::Failure (err) => {
                    return MultiSendStatus::Failure (SendStatus::Failure (err));
                },
                SendStatus::SeriError (err) => {
                    return MultiSendStatus::Failure (SendStatus::SeriError (err));
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
    pub async fn process(&mut self, _msg: api::msg::Message) {
        //TODO implement this
    }

    /// update the ClientInterface, reading and writing i/o, and doing processing
    ///
    /// cancelation unsafe, and can wait for large ammounts of time before returning
    ///
    /// Alternatively, you can use update_read, update_process_all, and send_all_queued, which does the same thing as this
    #[allow(dead_code)]
    pub async fn update(&mut self) -> Result<UpdateStatus, UpdateError> {
        println!("Updating");
        let latest = api::seri::get_message_from_socket(&mut self.socket).await;
        println!("{:#?}", latest);
        let read = match latest {
            Err(err) => {
                panic!("Failed to get a message!, {:#?}", err);
            },
            Ok(GetMessageResponse {msg, bytes: _}) => {
                self.process(msg).await;
                true
            }
        };
        match self.send_all_queued().await {
            MultiSendStatus::NoTask => {
                if read {
                    return Ok(UpdateStatus::Sucsess);
                } else {
                    return Ok(UpdateStatus::Noop);
                }
            },
            MultiSendStatus::Failure (stat) => {
                panic!("Failed to send message {:?}", stat);
            },
            MultiSendStatus::Worked { amnt: _, bytes: _ } => {
                return Ok(UpdateStatus::Sucsess);
            }
        };
    }

    /// read one message from the socket
    pub async fn update_read(&mut self) -> Result<UpdateReadStatus, UpdateReadError> {
        // let latest = api::seri::get_message_from_socket(&mut self.socket).await;
        // match latest {
        //     Err(err) => {
        //         panic!("Failed to get a message!, {:#?}", err);
        //     },
        //     Ok(GetMessageResponse {msg, bytes: _}) => {
        //         self.incoming.add(msg).unwrap();
        //     }
        // };
        println!("Update read start");
        //TODO god fix this sin
        drop(self.socket.readable().await);//heck u and ill see u never
        println!("Ready to read");

        let mut header_buffer = [0; HEADER_LEN];
        let mut read = 0;
        loop {
            drop(self.socket.readable().await);
            match self.socket.try_read(&mut header_buffer) {
                Ok(0) => {
                    return Err(UpdateReadError::Disconnected);
                },
                Ok(n) => {
                    println!("{}", n);
                    read += n;
                    if read >= HEADER_LEN {
                        println!("{} {} {}", read, header_buffer.len(), HEADER_LEN);
                        break;
                    }
                },
                Err(err) => {
                    if err.kind() == tokio::io::ErrorKind::WouldBlock {
                        continue;
                    }
                    return Err(UpdateReadError::ReadError (err))
                },
            }
        }
        drop(read);
        println!("{:?}", header_buffer);
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
                            return Err(UpdateReadError::Disconnected);
                        },
                        Ok(n) => {
                            println!("{}", n);
                            read += n;
                            if read >= read_amnt {
                                println!("{} {} {}, {}", read, buffer.len(), buffer.capacity(), read_amnt);
                                //TODO implement parsing the message
                                let deserialized: std::result::Result<api::msg::Message, Box<bincode::ErrorKind>> = bincode::deserialize(&buffer[..]);
                                match deserialized {
                                    Ok(msg) => {
                                        println!("{:#?}", msg);
                                        return Ok(UpdateReadStatus {msg: msg, bytes: buffer.len()})
                                    },
                                    Err(err) => {
                                        return Err(UpdateReadError::DeserializationError (err));
                                    }
                                }
                            }
                        },
                        Err(err) => {
                            if err.kind() == tokio::io::ErrorKind::WouldBlock {
                                continue;
                            }
                            return Err(UpdateReadError::ReadError (err))
                        },
                    }
                }
            },
            Err(err) => {
                return Err(UpdateReadError::HeaderParser (err));
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