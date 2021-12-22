use api::common;
use api::seri;
use api::msg;
use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt;
use tokio::task;
use tokio::join;
use tokio::io;
use queues::Queue;
use queues::IsQueue;
use queues::queue;
use tokio::sync::broadcast::{channel, Receiver, Sender};

pub mod stati {
    #[derive(Debug)]
    pub enum SendStatus {
        Sent(usize),
        NoTask,
        Failure(std::io::Error),
        SeriError(api::seri::res::SerializationError),
    }

    #[derive(Debug)]
    pub enum MultiSendStatus {
        Worked { amnt: u32, bytes: u128 },
        Failure(SendStatus),
        NoTask,
    }

    #[derive(Debug)]
    pub enum CloseType {
        /// Do not notify the server, just close quickly
        Force,
        /// Close, but assume the server has already closed, so dont attempt sending disconnect msg
        ServerDisconnected,
        /// close properly
        Graceful,
    }
}

#[derive(Clone, Debug)]
pub struct ShutdownMessage {}

struct Client {
    sock: TcpStream,
    incoming: Queue<msg::Message>,
    outgoing: Queue<msg::Message>,
    server_name: Option<String>,
}

impl Client {
    pub fn new(sock: TcpStream) -> Client {
        Client {
            sock: sock,
            incoming: queue![],
            outgoing: queue![],
            server_name: None,
        }
    }

    pub async fn close(&mut self, method: stati::CloseType) {
        match method {
            stati::CloseType::Force => {
                match self.sock.shutdown().await {
                    Ok(_) => {},
                    Err(err) => {
                        println!("Error whilest closing connection to server:\n{:#?}", err);
                    },
                };
            },
            stati::CloseType::ServerDisconnected => {
                match self.sock.shutdown().await {
                    Ok(_) => {},
                    Err(err) => {
                        if err.kind() != std::io::ErrorKind::NotConnected {
                            println!("Error whilest closing connection to server:\n{:#?}", err);
                        }
                    },
                };
            },
            stati::CloseType::Graceful => {
                match self.send_message(api::msg::Message {data: api::msg::MessageVarient::DisconnectMessage {}}).await {
                    stati::SendStatus::Failure(err) => {
                        match err.kind() {
                            std::io::ErrorKind::NotConnected => {
                                println!("During shutdown: Expected to be connected, but was not!\n{:#?}", err);
                            },
                            _ => {
                                println!("Error whilst sending shutdown msg!\n{:#?}", err);
                            }
                        };
                    },
                    stati::SendStatus::SeriError(_) => panic!("Could not serialize shutdown msg"),
                    _ => {},
                };
            },
        };
        drop(self);
    }

    async fn send_message(&mut self, message: api::msg::Message) -> stati::SendStatus {
        let serialized_msg_res = api::seri::serialize(&message);
        match serialized_msg_res {
            Ok(mut serialized_msg) => {
                let wrote_size = serialized_msg.size();
                let b_data = serialized_msg.into_bytes();
                let write_status = self.socket.write_all(&b_data).await;
                match write_status {
                    Ok(_) => stati::SendStatus::Sent(wrote_size),
                    Err(err) => stati::SendStatus::Failure(err),
                }
            }
            Err(err) => stati::SendStatus::SeriError(err),
        }
    }

    /// Send one message from the queue
    async fn send_queued_message(&mut self) -> stati::SendStatus {
        let value = self.outgoing.remove();
        match value {
            Ok(v) => {
                let serialized_msg_res = api::seri::serialize(&v);
                match serialized_msg_res {
                    Ok(mut serialized_msg) => {
                        let wrote_size = serialized_msg.size();
                        let b_data = serialized_msg.into_bytes();
                        let write_status = self.sock.write_all(&b_data).await;
                        match write_status {
                            Ok(_) => stati::SendStatus::Sent(wrote_size),
                            Err(err) => stati::SendStatus::Failure(err),
                        }
                    }
                    Err(err) => stati::SendStatus::SeriError(err),
                }
            }
            Err(_err) => stati::SendStatus::NoTask, //Do nothing as there is nothing to do ;)
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
                            bytes: sent_bytes,
                        };
                    }
                }
                stati::SendStatus::Sent(stat) => {
                    sent += 1;
                    sent_bytes += stat as u128;
                }
                stati::SendStatus::Failure(err) => {
                    return stati::MultiSendStatus::Failure(stati::SendStatus::Failure(err));
                }
                stati::SendStatus::SeriError(err) => {
                    return stati::MultiSendStatus::Failure(stati::SendStatus::SeriError(err));
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let stream = TcpStream::connect("127.0.0.1:6142").await.unwrap();
    let (shutdown_tx, _): (Sender<ShutdownMessage>, Receiver<ShutdownMessage>) = channel(5);
    let ctrlc_transmitter = shutdown_tx.clone();

    let main_task = tokio::spawn(async move {
        let close_rcv = shutdown_tx.subscribe();
        let client = Client::new(stream);
    });
    join!(main_task, get_ctrlc_listener(ctrlc_transmitter));
}

fn get_ctrlc_listener(
    ctrlc_transmitter: Sender<ShutdownMessage>,
) -> task::JoinHandle<io::Result<()>> {
    tokio::spawn(async move {
        let sig_res = tokio::signal::ctrl_c().await;
        println!("\nRecieved ctrl+c, shutting down");
        if ctrlc_transmitter.receiver_count() == 0 {
            return sig_res;
        }
        ctrlc_transmitter
            .send(ShutdownMessage {})
            .unwrap();
        sig_res
    })
}
