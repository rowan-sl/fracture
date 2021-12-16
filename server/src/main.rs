use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::broadcast::*;
use queues::Queue;
use queues::IsQueue;
use queues::queue;
use api::seri::res::*;

#[derive(Debug)]
enum SendStatus {
    Sent (usize),
    NoTask,
    Failure (std::io::Error),
    SeriError (api::seri::res::SerializationError),
}

enum MultiSendStatus {
    Worked {
        amnt: u32,
        bytes: u128,
    },
    Failure (SendStatus),
    NoTask,
}

struct ClientInterface {
    to_send: Queue<api::msg::Message>,
    socket: TcpStream,
}

impl ClientInterface {
    pub fn new(sock: TcpStream) -> ClientInterface {
        ClientInterface {
            to_send: queue![],
            socket: sock,
        }
    }

    pub async fn close(&mut self) {
        //TODO make it send a shutdown message and clear queue and stuff
        self.send_all_queued().await;
        match self.socket.shutdown().await {
            Ok(_) => {},
            Err(err) => {
                panic!("Cound not shutdown socket!\n{}", err);
            }
        };
        //& bye bye me
        drop(self);
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

    /// Process a message, and queue apropreate responses for sending
    pub async fn process(&mut self, msg: api::msg::Message) {
        //TODO implement this
    }

    // update the ClientInterface, reading and writing i/o, and doing processing
    pub async fn update(&mut self) {
        let latest = api::seri::get_message_from_socket(&mut self.socket).await;
        println!("{:#?}", latest);
        match latest {
            Err(err) => {
                panic!("Failed to get a message!, {:#?}", err);
            },
            Ok(GetMessageResponse {msg, bytes}) => {
                self.process(msg).await;
            }
        }
        match self.send_all_queued().await {
            MultiSendStatus::NoTask => {},
            MultiSendStatus::Failure (stat) => {
                panic!("Failed to send message {:?}", stat);
            },
            MultiSendStatus::Worked { amnt, bytes } => {}
        }
    }
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6142").await?;

    loop {
        let (mut socket, _) = listener.accept().await?;

        tokio::spawn(
            async move {
                let mut interface = ClientInterface::new(socket);
                println!("Connected to {:?}", interface.socket.peer_addr());
                loop {
                    println!("Reading from sock");
                    interface.update().await;
                }
            }
        );
    }
}