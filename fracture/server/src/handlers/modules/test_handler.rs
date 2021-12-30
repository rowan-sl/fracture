use crate::handlers::imports::{GlobalHandlerOperation, HandlerOperation, MessageHandler};
use fracture_core::msg::MessageVarient::TestMessage;

pub struct TestHandler {
    pending: Vec<HandlerOperation>,
    pending_global: Vec<GlobalHandlerOperation>,
}

impl MessageHandler for TestHandler {
    type Operation = HandlerOperation;

    fn new() -> Box<Self> {
        Box::new(Self {
            pending: vec![],
            pending_global: vec![],
        })
    }

    fn handle(&mut self, msg: &fracture_core::msg::Message) -> bool {
        if let TestMessage {} = msg.data {
            println!("Received test message");
            self.pending_global.push(GlobalHandlerOperation::MsgAll {
                msg: fracture_core::msg::Message {
                    data: fracture_core::msg::MessageVarient::TestMessageResponse {},
                },
            });
            true
        } else {
            false
        }
    }

    fn handle_global_op(&mut self, op: &GlobalHandlerOperation) {
        #[allow(irrefutable_let_patterns)] //not a issue, will fix itself later
        if let GlobalHandlerOperation::MsgAll { msg } = op {
            self.pending
                .push(HandlerOperation::Client { msg: msg.clone() });
        }
    }

    fn get_global_operations(&mut self) -> Option<Vec<GlobalHandlerOperation>> {
        if self.pending_global.is_empty() {
            None
        } else {
            let res = Some(self.pending_global.clone());
            self.pending_global.clear();
            res
        }
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
