//TODO make user state manager to store connected users and send updates and stuff
//TODO implement and test a message handler
//TODO implement handler operations

use queues::queue;
use queues::IsQueue;
use queues::Queue;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

use api::msg;
use api::stat;
use api::SocketUtils;

pub mod stati {
    use api::stat;
    pub enum MultiSendStatus {
        Worked { amnt: u32, bytes: u128 },
        Failure(stat::SendStatus),
        NoTask,
    }

    #[derive(Debug)]
    #[allow(dead_code)]
    pub enum UpdateStatus {
        Sucsess,
        Noop,
    }

    #[derive(Debug)]
    pub enum UpdateReadStatus {
        Disconnected,
        GracefullDisconnect,
        Sucsess,
        ReadError(stat::ReadMessageError),
    }
}

/// Operations that a `MessageHandler` can request
#[allow(dead_code)]
pub enum HandlerOperation {
    /// Send a public message to all other users
    Public { msg: msg::Message },
    /// Send a message back to the client
    Client { msg: msg::Message },
    /// Send a chat message to all other clients
    Chat { message_content: String },
}

/// Generic trait for createing a message handler.
/// All handlers must be `Send`
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
        socket: TcpStream,
        name: String,
        handlers: Vec<Box<dyn MessageHandler + Send>>,
    ) -> Self {
        Self {
            to_send: queue![],
            incoming: queue![],
            state: InterfaceState::Start,
            handlers,
            socket,
            server_name: name,
            client_name: None,
            uuid: uuid::Uuid::new_v4(),
        }
    }

    /// Get the name of the connected client (it names itself).
    /// Will return None if the client has not sent connection data yet
    pub fn name(&self) -> Option<String> {
        self.client_name.clone()
    }

    /// Get the uuid of the client.
    /// this is not specified by the client, but generated randomly on initialization.
    pub const fn uuid(&self) -> uuid::Uuid {
        self.uuid
    }

    /// Close the socket, optionaly notifying the client why it is being disconnected
    /// does NOT send any queued messages, as that is kinda pointless since the client could not reply and may become confused
    pub async fn close(
        &mut self,
        reason: String,
        expect_alredy_shutdown: bool,
        client_requested: bool,
    ) {
        //TODO make it send a shutdown message and clear queue and stuff
        if !expect_alredy_shutdown {
            // only send queued messages if it isnt alredy shutdown (or at least know it is)
            let dconn_reason = if client_requested {
                api::msg::types::ServerDisconnectReason::ClientRequestedDisconnect
            } else {
                api::msg::types::ServerDisconnectReason::Closed
            };
            match self
                .send_message(api::msg::Message {
                    data: api::msg::MessageVarient::ServerForceDisconnect {
                        reason: dconn_reason,
                        close_message: reason,
                    },
                })
                .await
            {
                stat::SendStatus::Failure(err) => {
                    match err.kind() {
                        std::io::ErrorKind::NotConnected => {
                            println!(
                                "During shutdown: Expected to be connected, but was not!\n{:#?}",
                                err
                            );
                        }
                        _ => {
                            println!("Error whilst sending shutdown msg!\n{:#?}", err);
                        }
                    };
                }
                stat::SendStatus::SeriError(_) => panic!("Could not serialize shutdown msg"),
                _ => {}
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
    }

    #[allow(dead_code)]
    pub fn register_handler(&mut self, handler: Box<dyn MessageHandler + Send>) {
        self.handlers.push(handler);
    }

    #[allow(dead_code)]
    pub fn get_handlers(&mut self) -> &mut Vec<Box<dyn MessageHandler + Send>> {
        &mut self.handlers
    }

    /// Get the address of the client connected
    pub fn get_client_addr(&mut self) -> Result<std::net::SocketAddr, std::io::Error> {
        self.socket.peer_addr()
    }

    /// Send one message from the queue
    async fn send_queued_message(&mut self) -> stat::SendStatus {
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
                            Ok(_) => stat::SendStatus::Sent(wrote_size),
                            Err(err) => stat::SendStatus::Failure(err),
                        }
                    }
                    Err(err) => stat::SendStatus::SeriError(err),
                }
            }
            Err(_err) => stat::SendStatus::NoTask, //Do nothing as there is nothing to do ;)
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
                stat::SendStatus::NoTask => {
                    return if sent == 0 {
                        stati::MultiSendStatus::NoTask
                    } else {
                        stati::MultiSendStatus::Worked {
                            amnt: sent,
                            bytes: sent_bytes,
                        }
                    }
                }
                stat::SendStatus::Sent(stat) => {
                    sent += 1;
                    sent_bytes += stat as u128;
                }
                stat::SendStatus::Failure(err) => {
                    return stati::MultiSendStatus::Failure(stat::SendStatus::Failure(err));
                }
                stat::SendStatus::SeriError(err) => {
                    return stati::MultiSendStatus::Failure(stat::SendStatus::SeriError(err));
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
        use api::msg::{types, Message, MessageVarient};
        if let MessageVarient::Ping = msg.data {
            self.queue_message(Message {
                data: MessageVarient::Pong,
            })
            .unwrap();
            return true;
        }
        // handle everything else
        match self.state {
            InterfaceState::Start => {
                match msg.data {
                    MessageVarient::ConnectMessage { name } => {
                        //TODO make this actulay handle users (with the user state handler) (mabey later)
                        self.client_name = Some(name);
                        println!(
                            "Client named itself and completed auth: {:#?}",
                            self.name().unwrap()
                        );
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
                                reason: types::ServerDisconnectReason::InvalidConnectionSequence,
                            },
                        })
                        .unwrap();
                    }
                };
            }
            InterfaceState::RecevedConnectMessage => {
                self.queue_message(Message {
                    data: MessageVarient::ServerInfo {
                        server_name: self.server_name.clone(),
                        conn_status: types::ConnectionStatus::Connected,
                        connected_users: vec![], //TODO make this actulay send users instead of l y i n g (so many lies)
                        your_uuid: self.uuid.as_u128(),
                    },
                })
                .unwrap();
                self.state = InterfaceState::Ready;
            }
            InterfaceState::Ready => {
                // normal stuff handling
                let mut handeld = false;
                for h in &mut self.handlers {
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
        true
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
                if let msg::MessageVarient::DisconnectMessage {} = stat.msg.data {
                    stati::UpdateReadStatus::GracefullDisconnect
                } else {
                    let added = self.incoming.add(stat.msg);
                    if added.is_err() {
                        panic!("Could not queue message for sending!");
                    } else {
                        stati::UpdateReadStatus::Sucsess
                    }
                }
            }
            Err(err_or_disconnect) => match err_or_disconnect {
                stat::ReadMessageError::Disconnected => stati::UpdateReadStatus::Disconnected,
                err => stati::UpdateReadStatus::ReadError(err),
            },
        }
    }

    /// Process all messages
    pub async fn update_process_all(&mut self) {
        while let Ok(msg) = self.incoming.remove() {
            self.process(msg).await;
        }
    }
}

impl SocketUtils for ClientInterface {
    fn get_sock(&mut self) -> &mut TcpStream {
        &mut self.socket
    }
}
