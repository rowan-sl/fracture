use queues::queue;
use queues::IsQueue;
use queues::Queue;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use uuid::Uuid;

use fracture_core::handler::MessageHandler;
use fracture_core::msg;
use fracture_core::stat::SendStatus;
use fracture_core::SocketUtils;

use crate::types::{stati, ClientState, HandlerOperation, ServerInfo};

pub struct Client {
    name: String, //the name of the client
    sock: TcpStream,
    pub incoming: Queue<msg::Message>,
    outgoing: Queue<msg::Message>,
    /// Pending handler operations
    pub pending_op: Queue<HandlerOperation>,
    handlers: Vec<Box<dyn MessageHandler<Operation = HandlerOperation> + Send>>,
    pub server_info: Option<ServerInfo>,
    pub state: ClientState,
}

impl Client {
    pub fn new(
        sock: TcpStream,
        name: String,
        handlers: Vec<Box<dyn MessageHandler<Operation = HandlerOperation> + Send>>,
    ) -> Self {
        Self {
            sock,
            incoming: queue![],
            outgoing: queue![],
            pending_op: queue![],
            handlers,
            name,
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
                    .send_message(fracture_core::msg::Message {
                        data: fracture_core::msg::MessageVarient::DisconnectMessage {},
                    })
                    .await
                {
                    SendStatus::Failure(err) => {
                        match err.kind() {
                            std::io::ErrorKind::NotConnected => {
                                println!("During shutdown: Expected to be connected, but was not!\n{:#?}", err);
                            }
                            _ => {
                                println!("Error whilst sending shutdown msg!\n{:#?}", err);
                            }
                        };
                    }
                    SendStatus::SeriError(_) => panic!("Could not serialize shutdown msg"),
                    _ => {}
                };
            }
        };
    }

    /// Send one message from the queue
    async fn send_queued_message(&mut self) -> SendStatus {
        let value = self.outgoing.remove();
        match value {
            Ok(v) => self.send_message(v).await,
            Err(_err) => SendStatus::NoTask, //Do nothing as there is nothing to do ;)
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
                SendStatus::NoTask => {
                    return if sent == 0 {
                        stati::MultiSendStatus::NoTask
                    } else {
                        stati::MultiSendStatus::Worked {
                            amnt: sent,
                            bytes: sent_bytes,
                        }
                    }
                }
                SendStatus::Sent(stat) => {
                    sent += 1;
                    sent_bytes += stat as u128;
                }
                SendStatus::Failure(err) => {
                    return stati::MultiSendStatus::Failure(SendStatus::Failure(err));
                }
                SendStatus::SeriError(err) => {
                    return stati::MultiSendStatus::Failure(SendStatus::SeriError(err));
                }
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
        if let Ok(stat) = read {
            if let msg::MessageVarient::ServerForceDisconnect {
                reason,
                close_message,
            } = stat.msg.data
            {
                stati::UpdateReadStatus::ServerClosed {
                    reason,
                    close_message,
                }
            } else {
                self.incoming.add(stat.msg).unwrap();
                stati::UpdateReadStatus::Success
            }
        } else if let Err(err) = read {
            match err {
                fracture_core::stat::ReadMessageError::Disconnected => {
                    stati::UpdateReadStatus::ServerDisconnect
                }
                oerr => stati::UpdateReadStatus::ReadError(oerr),
            }
        } else {
            panic!("this should never happen, but it apeases the compiler")
        }
    }

    /// Processes incoming messages, and then queues messages for sending
    pub async fn update(&mut self) -> stati::UpdateStatus {
        use stati::UpdateStatus::{Noop, Success, Unexpected, Unhandled};
        use ClientState::{Begin, GetHandlerDefaultOps, Hanshake, Ready};
        match &self.state {
            Begin => {
                // send message to server about the client
                match self
                    .send_message(fracture_core::common::gen_connect(self.name.clone()))
                    .await
                {
                    fracture_core::stat::SendStatus::Sent(_) => {}
                    err => {
                        eprintln!("Failed to send connect message:\n{:#?}", err);
                        return stati::UpdateStatus::SendError(err);
                    }
                };
                self.state = Hanshake;
                Success
            }
            Hanshake => {
                // receive server data
                let next = self.incoming.remove();
                match next {
                    Ok(msg) => {
                        match msg.data {
                            fracture_core::msg::MessageVarient::ServerInfo {
                                server_name,
                                conn_status,
                                connected_users: _, //TODO stop ignoring the servers lies, prehaps when it stops lying
                                your_uuid,
                            } => {
                                if let msg::types::ConnectionStatus::Refused { reason } =
                                    conn_status
                                {
                                    println!("Connection refused:{}", reason);
                                    return stati::UpdateStatus::ConnectionRefused;
                                }
                                println!("Connected to: {}", server_name);
                                let real_uuid = Uuid::from_u128(your_uuid);
                                self.server_info = Some(ServerInfo {
                                    client_uuid: real_uuid,
                                    name: server_name,
                                });
                                self.state = ClientState::GetHandlerDefaultOps;
                                Success
                            }
                            _ => {
                                self.incoming.add(msg.clone()).unwrap();
                                Unexpected(msg)
                            },
                        }
                    }
                    Err(_) => Noop,
                }
            }
            GetHandlerDefaultOps => {
                for h in &mut self.handlers {
                    for op in h.get_default_operations() {
                        self.pending_op.add(op).unwrap();
                    }
                }
                self.state = ClientState::Ready;
                Success
            }
            Ready => {
                let msg = match self.incoming.remove() {
                    Ok(m) => m,
                    Err(_) => {
                        return Noop;
                    }
                };
                let mut handeld = false;
                for h in &mut self.handlers {
                    let result = h.handle(&msg);
                    if result {
                        handeld = true;
                        break;
                    }
                }
                if handeld {
                    Success
                } else {
                    Unhandled(msg)
                }
            }
        }
    }

    /// Collects pending actions from handlers and stores them internaly
    pub fn collect_actions(&mut self) {
        for h in &mut self.handlers {
            if let Some(ops) = h.get_operations() {
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
        match op {
            Ok(op) => {
                match op {
                    HandlerOperation::InterfaceOperation(_) => Ok(Some(op)),
                    HandlerOperation::ServerMsg { msg } => {
                        self.queue_msg(msg);
                        Ok(None)
                    }
                    #[allow(unreachable_patterns)] // this is a GOOD thing
                    _ => Err(Some(op)),
                }
            }
            Err(_) => Err(None),
        }
    }

    ///Manualy queue a handler operation to be run
    pub fn manual_handler_operation(&mut self, oper: HandlerOperation) {
        self.pending_op
            .add(oper)
            .expect("placed operation in queue");
    }
}

impl SocketUtils for Client {
    fn get_sock(&mut self) -> &mut TcpStream {
        &mut self.sock
    }
}
