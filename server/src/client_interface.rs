use tokio::io::{ AsyncWriteExt };
use tokio::net::TcpStream;
use queues::Queue;
use queues::IsQueue;
use queues::queue;
use api::seri::res::*;
use tokio::sync::broadcast::*;

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
pub enum UpdateStatus {
    Shutdown,
    Sucsess,
    Noop,
}

#[derive(Debug)]
pub enum UpdateError {
    SysChannelRead,
}

pub struct ClientInterface {
    to_send: Queue<api::msg::Message>,
    socket: TcpStream,
    sys_msg_rx: Receiver<crate::types::ProgramMessage>,
}

impl ClientInterface {
    pub fn new(sock: TcpStream, sys_msg: Receiver<crate::types::ProgramMessage>) -> ClientInterface {
        ClientInterface {
            to_send: queue![],
            socket: sock,
            sys_msg_rx: sys_msg,
        }
    }

    pub async fn close(&mut self) {
        //TODO make it send a shutdown message and clear queue and stuff
        self.queue_message(api::msg::Message {data: api::msg::MessageVarient::ServerClosed {close_message: String::from("Client interface closed")}}).unwrap();
        self.send_all_queued().await;
        match self.socket.shutdown().await {
            Ok(_) => {},
            Err(err) => {
                panic!("Cound not shutdown socket!\n{}", err);
            }
        };
        //& bye bye me
        // drop(self);
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
    async fn send_all_queued(&mut self) -> MultiSendStatus {
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
    pub async fn process(&mut self, msg: api::msg::Message) {
        //TODO implement this
    }

    // update the ClientInterface, reading and writing i/o, and doing processing
    pub async fn update(&mut self) -> Result<UpdateStatus, UpdateError> {
        println!("Updating");
        tokio::select! {
            latest = api::seri::get_message_from_socket(&mut self.socket) => {
                println!("{:#?}", latest);
                match latest {
                    Err(err) => {
                        panic!("Failed to get a message!, {:#?}", err);
                    },
                    Ok(GetMessageResponse {msg, bytes: _}) => {
                        self.process(msg).await;
                    }
                }
                match self.send_all_queued().await {
                    MultiSendStatus::NoTask => {
                        return Ok(UpdateStatus::Noop);
                    },
                    MultiSendStatus::Failure (stat) => {
                        panic!("Failed to send message {:?}", stat);
                    },
                    MultiSendStatus::Worked { amnt: _, bytes: _ } => {
                        return Ok(UpdateStatus::Sucsess);
                    }
                }
            }
            res = self.sys_msg_rx.recv() => {
                println!("Recieved a sys message");
                match res {
                    Ok(msg) => {
                        match msg {
                            crate::types::ProgramMessage::Shutdown => {
                                println!("Shutting down!");
                                self.close().await;
                                return Ok(UpdateStatus::Shutdown);
                            }
                        }
                    },
                    Err(_err) => {
                        return Err(UpdateError::SysChannelRead);
                    }
                }
            }
        };
    }
}