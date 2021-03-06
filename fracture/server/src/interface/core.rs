//TODO make user state manager to store connected users and send updates and stuff
//TODO implement and test a message handler
//TODO implement handler operations

use queues::queue;
use queues::IsQueue;
use queues::Queue;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::TryRecvError;

#[allow(unused_imports)]
use log::{trace, debug, info, warn, error};

use fracture_core::handler::GlobalHandlerOperation;
use fracture_core::handler::ServerMessageHandler;
use fracture_core::msg;
use fracture_core::stat;
use fracture_core::SocketUtils;

pub mod stati {
    use fracture_core::stat;

    #[derive(thiserror::Error, Debug)]
    #[error("Send error: {0}")]
    pub struct MultiSendError ( #[from] pub stat::SendError);

    pub enum MultiSendStatus {
        Sent {
            amnt: u32,
            bytes: u128
        },
        NoTask,
    }

    #[derive(Debug)]
    pub enum UpdateStatus {
        Sucsess,
        Noop,
        ClientKicked(String),
        #[allow(dead_code)]
        Unexpected(fracture_core::msg::Message),
        #[allow(dead_code)]
        SendError(fracture_core::stat::SendStatus),
        Unhandled(fracture_core::msg::Message),
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
    /// Send a message back to the client
    Client { msg: msg::Message },
}

#[derive(Clone, Debug)]
pub struct ClientInfo {
    pub name: String,
    pub uuid: uuid::Uuid,
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
    to_send: Queue<fracture_core::msg::Message>,
    incoming: Queue<fracture_core::msg::Message>,
    /// what state the interface is in (like waiting for the client to send connection/auth stuff)
    state: InterfaceState,
    /// Handlers for messages, to be asked to handle new incoming messages
    handlers: Vec<
        Box<dyn ServerMessageHandler<ClientData = ClientInfo, Operation = HandlerOperation> + Send>,
    >,
    pending_op: Queue<HandlerOperation>,
    global_handler_rx: broadcast::Receiver<GlobalHandlerOperation>,
    global_handler_tx: broadcast::Sender<GlobalHandlerOperation>,
    pending_global_ops: Queue<GlobalHandlerOperation>,
    server_name: String,
    client_name: Option<String>,
    uuid: uuid::Uuid,
    socket: TcpStream,
}

impl ClientInterface {
    pub fn new(
        socket: TcpStream,
        name: String,
        handlers: Vec<
            Box<
                dyn ServerMessageHandler<ClientData = ClientInfo, Operation = HandlerOperation>
                    + Send,
            >,
        >,
        global_handler_channel: broadcast::Sender<GlobalHandlerOperation>,
    ) -> Self {
        let global_handler_rx = global_handler_channel.subscribe();
        let uuid = uuid::Uuid::new_v4();
        Self {
            to_send: queue![],
            incoming: queue![],
            state: InterfaceState::Start,
            handlers,
            socket,
            pending_op: queue![],
            pending_global_ops: queue![],
            global_handler_tx: global_handler_channel,
            global_handler_rx,
            server_name: name,
            client_name: None,
            uuid,
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
        pos_reason: Option<fracture_core::msg::types::ServerDisconnectReason>,
    ) {
        //TODO make it send a shutdown message and clear queue and stuff
        if let Some(reason) = pos_reason {
            // only send queued messages if it isnt alredy shutdown (or at least know it is)
            match self
                .send_message(fracture_core::msg::Message {
                    data: fracture_core::msg::MessageVarient::ServerForceDisconnect {
                        reason,
                        close_message: disconn_msg,
                    },
                })
                .await
            {
                Ok(_) => {}
                Err(e) => {
                    match e {
                        stat::SendError::Failure(err) => {
                            match err.kind() {
                                std::io::ErrorKind::NotConnected => {
                                    error!(
                                        "During shutdown: Expected to be connected, but was not!\n{:#?}",
                                        err
                                    );
                                }
                                _ => {
                                    error!("Error whilst sending shutdown msg!\n{:#?}", err);
                                }
                            };
                        }
                        stat::SendError::SeriError(_) => panic!("Could not serialize shutdown msg"),
                    }
                }
            };
        }
        match self.socket.shutdown().await {
            Ok(_) => {}
            Err(err) => {
                //Socket is already shutdown
                if err.kind() != std::io::ErrorKind::NotConnected {
                    error!("Error whilst closing connection: {:#?}", err);
                }
            }
        };
    }

    /// Get the address of the client connected
    pub fn get_client_addr(&mut self) -> Result<std::net::SocketAddr, std::io::Error> {
        self.socket.peer_addr()
    }

    /// Send one message from the queue
    async fn send_queued_message(&mut self) -> Result<stat::SendStatus, stat::SendError> {
        let value = self.to_send.remove();
        match value {
            Ok(v) => self.send_message(v).await,
            Err(_err) => Ok(stat::SendStatus::NoTask), //Do nothing as there is nothing to do ;)
        }
    }

    /// Send all queued messages
    ///
    /// ! THIS IS NOT CANCELATION SAFE!!!!
    pub async fn send_all_queued(&mut self) -> Result<stati::MultiSendStatus, stati::MultiSendError> {
        let mut sent = 0;
        let mut sent_bytes = 0;
        loop {
            let res = self.send_queued_message().await;
            match res {
                Ok(s) => {
                    match s { 
                        stat::SendStatus::NoTask => {
                            return if sent == 0 {
                                Ok(stati::MultiSendStatus::NoTask)
                            } else {
                                Ok(stati::MultiSendStatus::Sent {
                                    amnt: sent,
                                    bytes: sent_bytes,
                                })
                            }
                        }
                        stat::SendStatus::Sent(stat) => {
                            sent += 1;
                            sent_bytes += stat as u128;
                        }
                    }
                }
                Err(e) => {
                    match e {
                        stat::SendError::Failure(err) => {
                            return Err(stati::MultiSendError(stat::SendError::Failure(err)));
                        }
                        stat::SendError::SeriError(err) => {
                            return Err(stati::MultiSendError(stat::SendError::SeriError(err)));
                        }
                    }
                }
            }
        }
    }

    /// Queue a message for sending.
    /// Returns the value as if .add(msg) had been called on the msg queue.
    pub fn queue_message(
        &mut self,
        msg: fracture_core::msg::Message,
    ) -> Result<Option<fracture_core::msg::Message>, &str> {
        self.to_send.add(msg)
    }

    /// Process a message, and queue apropreate responses for sending
    /// returns if the message was handled or not
    pub async fn update(&mut self) -> stati::UpdateStatus {
        // The ping message should always be a valid thing to send/rcv
        use fracture_core::msg::{types, Message, MessageVarient};
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
                            debug!(
                                "Client named itself and completed auth: {:#?}",
                                self.name().unwrap()
                            );
                            let _ =
                                self.global_handler_tx
                                    .send(GlobalHandlerOperation::ClientNamed {
                                        uuid: self.uuid(),
                                        name: self.name().unwrap(),
                                    });
                            self.state = InterfaceState::RecevedConnectMessage;
                            return stati::UpdateStatus::Sucsess;
                        }
                        other => {
                            error!(
                                "Recieved {:#?} from client {:?} instead of connect message!",
                                other,
                                self.get_client_addr()
                            );
                            return stati::UpdateStatus::ClientKicked(format!(
                                "Recieved {:#?} instead of connect message!",
                                other
                            ));
                        }
                    };
                }
            }
            InterfaceState::RecevedConnectMessage => {
                for handler in &mut self.handlers {
                    handler.accept_client_data(ClientInfo {
                        name: self.client_name.clone().expect("This should not happen"),
                        uuid: self.uuid,
                    });
                }
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
                    return if handeld {
                        stati::UpdateStatus::Sucsess
                    } else {
                        stati::UpdateStatus::Unhandled(msg)
                    };
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
            Ok(op) => match op {
                HandlerOperation::Client { msg } => {
                    self.queue_message(msg).unwrap();
                    Ok(None)
                }
                #[allow(unreachable_patterns)] //this is fine, it will fix itself later
                _ => Err(Some(op)),
            },
            Err(_) => Err(None),
        }
    }

    /// Collects global actions from the handlers and sends them in the global operation channel
    pub fn colloect_send_global_actions(&mut self) {
        for h in &mut self.handlers {
            if let Some(ops) = h.get_global_operations() {
                for op in ops {
                    match self.global_handler_tx.send(op) {
                        Ok(_) => {}
                        Err(err) => {
                            error!("No one cared: {:?}", err);
                        }
                    }
                }
            }
        }
    }

    /// Collect global operations from the channel and store them internaly
    pub fn collect_recv_global_actions(&mut self) {
        loop {
            match self.global_handler_rx.try_recv() {
                Ok(op) => {
                    self.pending_global_ops.add(op).unwrap();
                }
                Err(err) => match err {
                    TryRecvError::Closed => {
                        panic!("No senders left! this should not happen");
                    }
                    TryRecvError::Lagged(amnt) => {
                        panic!("Missed receiving {} global operations! if this continues, increase the max allowed ammount!", amnt);
                    }
                    TryRecvError::Empty => break,
                },
            }
        }
    }

    /// Executes internaly stored global operations
    pub fn execute_global_actions(&mut self) {
        while let Ok(oper) = self.pending_global_ops.remove() {
            for h in &mut self.handlers {
                h.handle_global_op(&oper);
            }
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
