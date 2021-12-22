//TODO make user state manager to store connected users and send updates and stuff
//TODO implement and test a message handler
//TODO implement handler operations

use api::header::HEADER_LEN;
use api::msg;
use queues::queue;
use queues::IsQueue;
use queues::Queue;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

pub mod stati {
    #[derive(Debug)]
    pub enum SendStatus {
        Sent(usize),
        NoTask,
        Failure(std::io::Error),
        SeriError(api::seri::res::SerializationError),
    }

    pub enum MultiSendStatus {
        Worked { amnt: u32, bytes: u128 },
        Failure(SendStatus),
        NoTask,
    }

    #[derive(Debug)]
    #[allow(dead_code)]
    pub enum UpdateStatus {
        Sucsess,
        Noop,
    }

    #[derive(Debug)]
    pub struct ReadMessageStatus {
        pub msg: api::msg::Message,
        pub bytes: usize,
    }

    #[derive(Debug)]
    pub enum ReadMessageError {
        Disconnected,
        ReadError(std::io::Error),
        HeaderParser(api::header::HeaderParserError),
        DeserializationError(Box<bincode::ErrorKind>),
    }

    #[derive(Debug)]
    pub enum UpdateReadStatus {
        Disconnected,
        GracefullDisconnect,
        Sucsess,
        ReadError(ReadMessageError),
    }
}

/// Operations that a MessageHandler can request occur
pub enum HandlerOperation {
    /// Send a public message to all other users
    SendPublicMessage {
        msg: msg::Message,
    },
    /// Send a message back to the client
    SendSelfMessage {
        msg: msg::Message,
    },
    /// Send a chat message to all other clients
    SendChatMessage {
        message_content: String,
    },
}

/// Generic trait for createing a message handler.
/// All handlers must be Send
pub trait MessageHandler {
    fn new() -> Self
    where
        Self: Sized;

    /// takes a message, potentialy handleing it.
    /// returns wether or not the message was handled (should the interface attempt to continue trying new handlers to handle it)
    fn handle(&mut self, msg: &api::msg::Message) -> bool;

    /// Get operations that the handler is requesting the interface do
    fn get_operations(&mut self) -> Option<Vec<HandlerOperation>>;
}

//TODO this
pub enum InterfaceState {
    /// Begining state, in this state it is waiting for the client to send ConnectMessage
    Start,
    /// Client has sent ConnectMessage,
    /// Server sends the data back
    RecevedConnectMessage,
    /// Client is ready, this is the state where normal message handlers can do there job
    Ready,
}

pub struct ClientInterface {
    to_send: Queue<api::msg::Message>,
    incoming: Queue<api::msg::Message>,
    /// what state the interface is in (like waiting for the client to send connection/auth stuff)
    state: InterfaceState,
    /// Handlers for messages, to be asked to handle new incoming messages
    handlers: Vec<Box<dyn MessageHandler + Send>>,
    server_name: String,
    client_name: Option<String>,
    uuid: uuid::Uuid,
    socket: TcpStream,
}

impl ClientInterface {
    pub fn new(
        sock: TcpStream,
        name: String,
        handlers: Vec<Box<dyn MessageHandler + Send>>,
    ) -> ClientInterface {
        ClientInterface {
            to_send: queue![],
            incoming: queue![],
            state: InterfaceState::Start,
            handlers: handlers,
            socket: sock,
            server_name: name,
            client_name: None,
            uuid: uuid::Uuid::new_v4(),
        }
    }

    /// Get the name of the connected client (it names itself).
    /// Will return None if the client has not sent connection data yet
    pub fn name(&self) -> Option<String> {
        return self.client_name.clone();
    }

    /// Get the uuid of the client.
    /// this is not specified by the client, but generated randomly on initialization.
    pub fn uuid(&self) -> uuid::Uuid {
        return self.uuid.clone();
    }

    /// Close the socket, optionaly notifying the client why it is being disconnected
    /// does NOT send any queued messages, as that is kinda pointless since the client could not reply and may become confused
    pub async fn close(&mut self, reason: String, expect_alredy_shutdown: bool, client_requested: bool) {
        //TODO make it send a shutdown message and clear queue and stuff
        if !expect_alredy_shutdown {
            // only send queued messages if it isnt alredy shutdown (or at least know it is)
            let dconn_reason = match client_requested {
                true => api::msg::types::ServerDisconnectReason::ClientRequestedDisconnect,
                false => api::msg::types::ServerDisconnectReason::Closed,
            };
            match self.send_message(api::msg::Message {
                data: api::msg::MessageVarient::ServerForceDisconnect {
                    reason: dconn_reason,
                    close_message: reason,
                },
            }).await {
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
        }
        match self.socket.shutdown().await {
            Ok(_) => {}
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

    #[allow(dead_code)]
    pub fn register_handler(&mut self, handler: Box<dyn MessageHandler + Send>) {
        self.handlers.push(handler);
    }

    #[allow(dead_code)]
    pub fn get_handlers(&mut self) -> &mut Vec<Box<dyn MessageHandler + Send>> {
        return &mut self.handlers;
    }

    /// Get the address of the client connected
    pub fn get_client_addr(&mut self) -> Result<std::net::SocketAddr, std::io::Error> {
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
            Err(_err) => stati::SendStatus::NoTask, //Do nothing as there is nothing to do ;)
        }
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

    /// Queue a message for sending.
    /// Returns the value as if .add(msg) had been called on the msg queue.
    pub fn queue_message(
        &mut self,
        msg: api::msg::Message,
    ) -> Result<Option<api::msg::Message>, &str> {
        self.to_send.add(msg)
    }

    /// Process a message, and queue apropreate responses for sending
    /// returns if the message was handled or not
    async fn process(&mut self, msg: api::msg::Message) -> bool {
        // The ping message should always be a valid thing to send/rcv
        use api::msg::*;
        match msg.data {
            MessageVarient::Ping => {
                self.queue_message(Message {
                    data: MessageVarient::Pong,
                })
                .unwrap();
                return true;
            }
            _ => {}
        };
        // handle everything else
        match self.state {
            InterfaceState::Start => {
                match msg.data {
                    MessageVarient::ConnectMessage { name } => {
                        //TODO make this actulay handle users (with the user state handler) (mabey later)
                        self.client_name = Some(name);
                        self.state = InterfaceState::RecevedConnectMessage;
                    }
                    other => {
                        println!(
                            "Recieved {:#?} from client {:?} instead of connect message!",
                            other,
                            self.get_client_addr()
                        );
                        self.queue_message(Message {
                            data: MessageVarient::ServerForceDisconnect {
                                close_message: format!(
                                    "Recieved {:#?} instead of connect message!",
                                    other
                                ),
                                reason:
                                    types::ServerDisconnectReason::InvalidConnectionSequence,
                            },
                        })
                        .unwrap();
                    }
                };
            }
            InterfaceState::RecevedConnectMessage => {
                self.queue_message(Message {
                    data: MessageVarient::ServerInfo {
                        name: self.server_name.clone(),
                        conn_status: types::ConnectionStatus::Connected,
                        connected_users: vec![], //TODO make this actulay send users instead of l y i n g
                    },
                })
                .unwrap();
                self.state = InterfaceState::Ready;
            }
            InterfaceState::Ready => {
                // normal stuff handling
                let mut handeld = false;
                for h in self.handlers.iter_mut() {
                    let result = h.handle(&msg);
                    if result {
                        handeld = true;
                        break;
                    }
                }
                if !handeld {
                    return false;
                }
            }
        };
        return true;
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
                    msg::MessageVarient::DisconnectMessage {} => {
                        return stati::UpdateReadStatus::GracefullDisconnect;
                    },
                    _ => {
                        let added = self.incoming.add(stat.msg);
                        if added.is_err() {
                            panic!("Could not queue message for sending!");
                        } else {
                            return stati::UpdateReadStatus::Sucsess;
                        }
                    }
                };
            }
            Err(err) => {
                match err {
                    stati::ReadMessageError::Disconnected => {
                        return stati::UpdateReadStatus::Disconnected;
                    }
                    oerr => {
                        return stati::UpdateReadStatus::ReadError(oerr);
                    }
                };
            }
        };
    }

    /// read one message from the socket
    pub async fn read_msg(&mut self) -> Result<stati::ReadMessageStatus, stati::ReadMessageError> {
        //TODO god fix this sin
        drop(self.socket.readable().await); //heck u and ill see u never

        let mut header_buffer = [0; HEADER_LEN];
        let mut read = 0;
        loop {
            drop(self.socket.readable().await);
            match self.socket.try_read(&mut header_buffer) {
                Ok(0) => {
                    return Err(stati::ReadMessageError::Disconnected);
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
                    return Err(stati::ReadMessageError::ReadError(err));
                }
            }
        }
        drop(read);
        let header_r =
            api::header::MessageHeader::from_bytes(api::seri::vec2bytes(Vec::from(header_buffer)));
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
