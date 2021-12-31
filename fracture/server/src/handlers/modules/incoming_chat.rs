use crate::handlers::imports::{GlobalHandlerOperation, HandlerOperation, MessageHandler, ServerClientInfo, ServerMessageHandler, ClientInfo};
use fracture_core::msg::MessageVarient::ClientSendChat;

pub struct IncomingChatHandler {
    pending: Vec<HandlerOperation>,
    pending_global: Vec<GlobalHandlerOperation>,
    client_data: Option<ClientInfo>,
}

impl MessageHandler for IncomingChatHandler {
    type Operation = HandlerOperation;

    fn new() -> Box<Self> {
        Box::new(Self {
            pending: vec![],
            pending_global: vec![],
            client_data: None,
        })
    }

    fn handle(&mut self, msg: &fracture_core::msg::Message) -> bool {
        if let ClientSendChat { content } = msg.data.clone() {
            println!("Received message {}", content);
            let dat = self.client_data.clone().unwrap();
            self.pending_global.push(GlobalHandlerOperation::MsgAll {
                msg: fracture_core::msg::Message {
                    data: fracture_core::msg::MessageVarient::ServerSendChat {
                        content,
                        author: dat.name,
                        author_uuid: dat.uuid.as_u128(),
                    },
                },
            });
            true
        } else {
            false
        }
    }

    fn handle_global_op(&mut self, _op: &GlobalHandlerOperation) {}

    fn get_global_operations(&mut self) -> Option<Vec<GlobalHandlerOperation>> {
        if self.pending_global.is_empty() {
            None
        } else {
            let res = Some(self.pending_global.clone());
            self.pending_global.clear();
            res
        }
    }

    fn get_operations(&mut self) -> Option<Vec<Self::Operation>> {None}

    fn get_default_operations(&mut self) -> Vec<Self::Operation> {
        vec![]
    }
}

impl ServerClientInfo for IncomingChatHandler {
    type ClientData = ClientInfo;

    fn accept_client_data(&mut self, data: Self::ClientData) {
        self.client_data = Some(data);
    }
}

impl ServerMessageHandler for IncomingChatHandler {}
