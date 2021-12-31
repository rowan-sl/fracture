use crate::handlers::imports::{
    ClientInfo, GlobalHandlerOperation, HandlerOperation, MessageHandler, ServerClientInfo,
    ServerMessageHandler,
};
use fracture_core::msg;

pub struct UserInfoUpdateHandler {
    client_data: Option<ClientInfo>,
    pending: Vec<HandlerOperation>,
}

impl MessageHandler for UserInfoUpdateHandler {
    type Operation = HandlerOperation;

    fn new() -> Box<Self> {
        Box::new(Self {
            client_data: None,
            pending: vec![]
        })
    }

    fn handle(&mut self, _msg: &fracture_core::msg::Message) -> bool {false}

    fn handle_global_op(&mut self, op: &GlobalHandlerOperation) {
        self.pending.push(
            HandlerOperation::Client {
                msg: msg::Message {
                    data: msg::MessageVarient::ServerInfoUpdate {
                        name: None,
                        user_updates: vec![
                            match op {
                                GlobalHandlerOperation::ClientConnect { uuid } => {
                                    msg::types::UserNameUpdate::NewUser { uuid: uuid.as_u128() }
                                }
                                GlobalHandlerOperation::ClientNamed { uuid, name } => {
                                    msg::types::UserNameUpdate::UserNamed { uuid: uuid.as_u128(), name: (*name).clone() }
                                }
                                GlobalHandlerOperation::ClientDisconnect { uuid, name } => {
                                    msg::types::UserNameUpdate::UserLeft { uuid: uuid.as_u128(), name: (*name).clone() }
                                }
                                _ => {return}
                            }
                        ],
                    }
                }
            }
        )
    }

    fn get_global_operations(&mut self) -> Option<Vec<GlobalHandlerOperation>> {None}

    fn get_operations(&mut self) -> Option<Vec<Self::Operation>> {
        if self.pending.is_empty() {
            None
        } else {
            Some(self.pending.drain(0..).collect())
        }
    }

    fn get_default_operations(&mut self) -> Vec<Self::Operation> {
        vec![]
    }
}

impl ServerClientInfo for UserInfoUpdateHandler {
    type ClientData = ClientInfo;

    fn accept_client_data(&mut self, data: Self::ClientData) {
        self.client_data = Some(data);
    }
}

impl ServerMessageHandler for UserInfoUpdateHandler {}
