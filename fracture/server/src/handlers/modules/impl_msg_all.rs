/// Implement msg GlobalHandlerOperation::MsgAll
/// when it recieves this, it will send that message to its associated client
use crate::handlers::imports::{
    ClientInfo, GlobalHandlerOperation, HandlerOperation, MessageHandler, ServerClientInfo,
    ServerMessageHandler,
};

pub struct MsgAllHandler {
    pending: Vec<HandlerOperation>,
}

impl MessageHandler for MsgAllHandler {
    type Operation = HandlerOperation;

    fn new() -> Box<Self> {
        Box::new(Self { pending: vec![] })
    }

    fn handle(&mut self, _: &fracture_core::msg::Message) -> bool {
        false
    }

    fn handle_global_op(&mut self, op: &GlobalHandlerOperation) {
        #[allow(irrefutable_let_patterns)] //not a issue, will fix itself later
        if let GlobalHandlerOperation::MsgAll { msg } = op {
            self.pending
                .push(HandlerOperation::Client { msg: msg.clone() });
        }
    }

    fn get_global_operations(&mut self) -> Option<Vec<GlobalHandlerOperation>> {
        None
    }

    fn get_operations(&mut self) -> Option<Vec<Self::Operation>> {
        if self.pending.is_empty() {
            None
        } else {
            let res = Some(self.pending.clone());
            self.pending.clear();
            res
        }
    }

    fn get_default_operations(&mut self) -> Vec<Self::Operation> {
        vec![]
    }
}

impl ServerClientInfo for MsgAllHandler {
    type ClientData = ClientInfo;

    fn accept_client_data(&mut self, _data: Self::ClientData) {}
}

impl ServerMessageHandler for MsgAllHandler {}
