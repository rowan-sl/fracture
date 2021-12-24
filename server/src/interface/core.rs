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
use api::handler::MessageHandler;

pub mod stati {
    use api::stat;
    pub enum MultiSendStatus {
        Worked { amnt: u32, bytes: u128 },
        Failure(stat::SendStatus),
        NoTask,
    }

    #[derive(Debug)]
    pub enum UpdateStatus {
        Sucsess,
        Noop,
        ClientKicked(String),
        #[allow(dead_code)]
        Unexpected(api::msg::Message),
        #[allow(dead_code)]
        SendError(api::stat::SendStatus),
        Unhandled(api::msg::Message),
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
#[derive(Clone, Debug)]
pub enum HandlerOperation {
    /// Send a public message to all other users
    Public { msg: msg::Message },
    /// Send a message back to the client
    Client { msg: msg::Message },
    /// Send a chat message to all other clients
    Chat { message_content: String },
}

//TODO this
#[derive(Debug)]
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
    handlers: Vec<Box<dyn MessageHandler<Operation = HandlerOperation> + Send>>,
    pending_op: Queue<HandlerOperation>,
    server_name: String,
    client_name: Option<String>,
    uuid: uuid::Uuid,
    socket: TcpStream,
}

impl ClientInterface {
    pub fn new(
        socket: TcpStream,
        name: String,
        handlers: Vec<Box<dyn MessageHandler<Operation = HandlerOperation> + Send>>,
    ) -> Self {
        Self {
            to_send: queue![],
            incoming: queue![],
            state: InterfaceState::Start,
            handlers,
            socket,
            pending_op: queue![],
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
        disconn_msg: String,
        pos_reason: Option<api::msg::types::ServerDisconnectReason>,
    ) {
        //TODO make it send a shutdown message and clear queue and stuff
        if let Some(reason) = pos_reason {
            // only send queued messages if it isnt alredy shutdown (or at least know it is)
            match self
                .send_message(api::msg::Message {
                    data: api::msg::MessageVarient::ServerForceDisconnect {
                        reason,
                        close_message: disconn_msg,
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
                if err.kind() != std::io::ErrorKind::NotConnected {
                    println!("Error whilst closing connection: {:#?}", err);
                }
            }
        };
    }

    #[allow(dead_code)]
    pub fn register_handler(&mut self, handler: Box<dyn MessageHandler<Operation = HandlerOperation> + Send>) {
        self.handlers.push(handler);
    }

    #[allow(dead_code)]
    pub fn get_handlers(&mut self) -> &mut Vec<Box<dyn MessageHandler<Operation = HandlerOperation> + Send>> {
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
                self.send_message(v).await
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
    pub async fn update(&mut self) -> stati::UpdateStatus {
        // The ping message should always be a valid thing to send/rcv
        use api::msg::{types, Message, MessageVarient};
        if let Ok(msg) = self.incoming.peek() {
            if let MessageVarient::Ping = msg.data {
                self.incoming.remove().unwrap();
                self.queue_message(Message {
                    data: MessageVarient::Pong,
                })
                .unwrap();
                return stati::UpdateStatus::Sucsess;
            }
        }
        // handle everything else
        match self.state {
            InterfaceState::Start => {
                if let Ok(msg) = self.incoming.remove() {
                    match msg.data {
                        MessageVarient::ConnectMessage { name } => {
                            //TODO make this actulay handle users (with the user state handler) (mabey later)
                            self.client_name = Some(name);
                            println!(
                                "Client named itself and completed auth: {:#?}",
                                self.name().unwrap()
                            );
                            self.state = InterfaceState::RecevedConnectMessage;
                            return stati::UpdateStatus::Sucsess;
                        }
                        other => {
                            println!(
                                "Recieved {:#?} from client {:?} instead of connect message!",
                                other,
                                self.get_client_addr()
                            );
                            return stati::UpdateStatus::ClientKicked(
                                format!(
                                    "Recieved {:#?} instead of connect message!",
                                    other
                                )
                            );
                        }
                    };
                }
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
                return stati::UpdateStatus::Sucsess;
            }
            InterfaceState::Ready => {
                // normal stuff handling
                if let Ok(msg) = self.incoming.remove() {
                    let mut handeld = false;
                    for h in &mut self.handlers {
                        let result = h.handle(&msg);
                        if result {
                            handeld = true;
                            break;
                        }
                    }
                    if handeld {
                        return stati::UpdateStatus::Sucsess;
                    } else {
                        return stati::UpdateStatus::Unhandled(msg);
                    }
                }
            }
        };
        stati::UpdateStatus::Noop
    }

    /// Collects pending actions from handlers and stores them internaly
    pub fn collect_actions(&mut self) {
        for h in &mut self.handlers {
            if let Some(ops) = h.get_operations() {
                // println!("Handler gave operations:\n{:#?}", ops);
                for op in ops {
                    self.pending_op.add(op).unwrap();
                }
            }
        }
    }

    /// Handles one handler action, returning it if it requires further processing by other code (interface operations)
    ///
    /// Errors: if it does not know what to do with the operation
    ///
    /// Ok(Some()) -> you need to deal with it
    ///
    /// Ok(None) -> you dont have to do anything
    ///
    /// Err(Some()) -> we dont know what to do with this
    ///
    /// Err(None) -> Nothing to do
    pub async fn execute_action(
        &mut self,
    ) -> Result<Option<HandlerOperation>, Option<HandlerOperation>> {
        let op = self.pending_op.remove();
        // println!("Executing operations:\n{:#?}", op);
        match op {
            Ok(op) => {
                match op {
                    HandlerOperation::Client {msg} => {
                        self.queue_message(msg).unwrap();
                        return Ok(None);
                    }
                    _ => Err(Some(op)),
                }
            }
            Err(_) => Err(None),
        }
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
}

impl SocketUtils for ClientInterface {
    fn get_sock(&mut self) -> &mut TcpStream {
        &mut self.socket
    }
}
