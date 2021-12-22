mod conf;

use queues::queue;
use queues::IsQueue;
use queues::Queue;
use tokio::io;
use tokio::io::AsyncWriteExt;
use tokio::join;
use tokio::net::TcpStream;
use tokio::sync::broadcast::{channel, Receiver, Sender};
use tokio::task;
use uuid::Bytes;
use uuid::Uuid;

use api::msg;
use api::seri;

pub mod stati {
    use api::msg;
    use api::seri;

    #[derive(Debug)]
    pub enum SendStatus {
        Sent(usize),
        NoTask,
        Failure(std::io::Error),
        SeriError(seri::res::SerializationError),
    }

    #[derive(Debug)]
    pub enum MultiSendStatus {
        Worked { amnt: u32, bytes: u128 },
        Failure(SendStatus),
        NoTask,
    }

    #[derive(Debug)]
    pub enum ReadMessageError {
        Disconnected,
        ReadError(std::io::Error),
        HeaderParser(api::header::HeaderParserError),
        DeserializationError(Box<bincode::ErrorKind>),
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

    #[derive(Debug)]
    pub struct ReadMessageStatus {
        pub msg: api::msg::Message,
        pub bytes: usize,
    }

    pub enum UpdateReadStatus {
        ServerClosed {
            reason: api::msg::types::ServerDisconnectReason,
            close_message: String,
        },
        ServerDisconnect,
        ReadError(ReadMessageError),
        Success,
    }

    pub enum UpdateStatus {
        Unexpected(msg::Message),
        Success,
        Unhandled,
        ConnectionRefused,
        Noop,
    }
}

/// For stuff to interact with the user
enum InterfaceOperation {

}

//TODO
/// Operations that a MessageHandler can request occur
enum HandlerOperation {
    /// Send a public message to all other users
    SendMessage { msg: msg::Message },
    /// Send a chat message to all other clients
    SendChatMessage { message_content: String },
    /// Do a program operation
    InterfaceOperation (InterfaceOperation)
}

/// Generic trait for createing a message handler.
/// All handlers must be Send
trait MessageHandler {
    fn new() -> Self
    where
        Self: Sized;

    /// takes a message, potentialy handleing it.
    /// returns wether or not the message was handled (should the interface attempt to continue trying new handlers to handle it)
    fn handle(&mut self, msg: &api::msg::Message) -> bool;

    /// Get operations that the handler is requesting the interface do
    fn get_operations(&mut self) -> Option<Vec<HandlerOperation>>;
}

enum ClientState {
    /// nothing has happened
    Begin,
    /// sent the connect message, waiting for a response
    Hanshake,
    /// Ready for normal stuff
    Ready,
}

struct ServerInfo {
    name: String,
    client_uuid: uuid::Uuid,
}

#[derive(Clone, Debug)]
pub struct ShutdownMessage {}

struct Client {
    sock: TcpStream,
    incoming: Queue<msg::Message>,
    outgoing: Queue<msg::Message>,
    server_info: Option<ServerInfo>,
    state: ClientState,
}

impl Client {
    pub fn new(sock: TcpStream) -> Client {
        Client {
            sock: sock,
            incoming: queue![],
            outgoing: queue![],
            server_info: None,
            state: ClientState::Begin,
        }
    }

    pub async fn close(&mut self, method: stati::CloseType) {
        match method {
            stati::CloseType::Force => {
                match self.sock.shutdown().await {
                    Ok(_) => {}
                    Err(err) => {
                        println!("Error whilest closing connection to server:\n{:#?}", err);
                    }
                };
            }
            stati::CloseType::ServerDisconnected => {
                match self.sock.shutdown().await {
                    Ok(_) => {}
                    Err(err) => {
                        if err.kind() != std::io::ErrorKind::NotConnected {
                            println!("Error whilest closing connection to server:\n{:#?}", err);
                        }
                    }
                };
            }
            stati::CloseType::Graceful => {
                match self
                    .send_message(api::msg::Message {
                        data: api::msg::MessageVarient::DisconnectMessage {},
                    })
                    .await
                {
                    stati::SendStatus::Failure(err) => {
                        match err.kind() {
                            std::io::ErrorKind::NotConnected => {
                                println!("During shutdown: Expected to be connected, but was not!\n{:#?}", err);
                            }
                            _ => {
                                println!("Error whilst sending shutdown msg!\n{:#?}", err);
                            }
                        };
                    }
                    stati::SendStatus::SeriError(_) => panic!("Could not serialize shutdown msg"),
                    _ => {}
                };
            }
        };
        drop(self);
    }

    async fn send_message(&mut self, message: api::msg::Message) -> stati::SendStatus {
        let serialized_msg_res = seri::serialize(&message);
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

    /// Send one message from the queue
    async fn send_queued_message(&mut self) -> stati::SendStatus {
        let value = self.outgoing.remove();
        match value {
            Ok(v) => {
                let serialized_msg_res = seri::serialize(&v);
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

    /// read one message from the socket
    pub async fn read_msg(&mut self) -> Result<stati::ReadMessageStatus, stati::ReadMessageError> {
        //TODO god fix this sin
        drop(self.sock.readable().await); //heck u and ill see u never

        let mut header_buffer = [0; api::header::HEADER_LEN];
        let mut read = 0;
        loop {
            drop(self.sock.readable().await);
            match self.sock.try_read(&mut header_buffer) {
                Ok(0) => {
                    return Err(stati::ReadMessageError::Disconnected);
                }
                Ok(n) => {
                    read += n;
                    if read >= api::header::HEADER_LEN {
                        break;
                    }
                }
                Err(err) => {
                    if err.kind() == tokio::io::ErrorKind::WouldBlock {
                        continue;
                    }
                    return Err(stati::ReadMessageError::ReadError(err));
                }
            }
        }
        drop(read);
        let header_r =
            api::header::MessageHeader::from_bytes(seri::vec2bytes(Vec::from(header_buffer)));
        match header_r {
            Ok(header) => {
                let read_amnt = header.size();
                let mut buffer: Vec<u8> = Vec::with_capacity(read_amnt);
                let mut read = 0;
                loop {
                    //TODO goodbye
                    drop(self.sock.readable().await);
                    match self.sock.try_read_buf(&mut buffer) {
                        Ok(0) => {
                            return Err(stati::ReadMessageError::Disconnected);
                        }
                        Ok(n) => {
                            read += n;
                            if read >= read_amnt {
                                //TODO implement parsing the message
                                let deserialized: std::result::Result<
                                    api::msg::Message,
                                    Box<bincode::ErrorKind>,
                                > = bincode::deserialize(&buffer[..]);
                                match deserialized {
                                    Ok(msg) => {
                                        return Ok(stati::ReadMessageStatus {
                                            msg: msg,
                                            bytes: buffer.len(),
                                        })
                                    }
                                    Err(err) => {
                                        return Err(stati::ReadMessageError::DeserializationError(
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
                            return Err(stati::ReadMessageError::ReadError(err));
                        }
                    }
                }
            }
            Err(err) => {
                return Err(stati::ReadMessageError::HeaderParser(err));
            }
        }
    }

    pub fn queue_msg(&mut self, msg: msg::Message) {
        self.outgoing.add(msg).unwrap();
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
                match stat.msg.data {
                    msg::MessageVarient::ServerForceDisconnect {
                        reason,
                        close_message,
                    } => {
                        return stati::UpdateReadStatus::ServerClosed {
                            reason: reason,
                            close_message: close_message,
                        };
                    }
                    _ => {
                        self.incoming.add(stat.msg).unwrap();
                        return stati::UpdateReadStatus::Success;
                    }
                };
            }
            Err(err) => {
                match err {
                    stati::ReadMessageError::Disconnected => {
                        return stati::UpdateReadStatus::ServerDisconnect;
                    }
                    oerr => {
                        return stati::UpdateReadStatus::ReadError(oerr);
                    }
                };
            }
        };
    }

    /// Processes incoming messages, and then queues messages for sending
    pub async fn update(&mut self) -> stati::UpdateStatus {
        use stati::UpdateStatus::*;
        use ClientState::*;
        match &self.state {
            Begin => {
                // send message to server about the client
                self.queue_msg(api::common::gen_connect(String::from(conf::NAME)));
                self.state = Hanshake;
                return Success;
            }
            Hanshake => {
                // receive server data
                let next = self.outgoing.remove();
                match next {
                    Ok(msg) => {
                        match msg.data {
                            api::msg::MessageVarient::ServerInfo {
                                server_name,
                                conn_status,
                                connected_users: _, //TODO stop ignoring the servers lies, prehaps when it stops lying
                                your_uuid,
                            } => {
                                match conn_status {
                                    msg::types::ConnectionStatus::Refused { reason } => {
                                        println!("Connection refused:{}", reason);
                                        return stati::UpdateStatus::ConnectionRefused;
                                    }
                                    _ => {}
                                }
                                let real_uuid = Uuid::from_u128(your_uuid);
                                self.server_info = Some(ServerInfo {
                                    client_uuid: real_uuid,
                                    name: server_name,
                                });
                                return Success;
                            }
                            _ => {
                                return Unexpected(msg);
                            }
                        };
                    }
                    Err(_) => {
                        return Noop;
                    }
                };
            }
            Ready => {
                return Noop;
            }
        };
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
        ctrlc_transmitter.send(ShutdownMessage {}).unwrap();
        sig_res
    })
}
