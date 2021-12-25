use crate::handlers::imports::{GlobalHandlerOperation, HandlerOperation, MessageHandler};
use api::msg::MessageVarient::TestMessage;

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

    fn handle(&mut self, msg: &api::msg::Message) -> bool {
        if let TestMessage {} = msg.data {
            println!("Received test message");
            self.pending_global.push(GlobalHandlerOperation::MsgAll {
                msg: api::msg::Message {
                    data: api::msg::MessageVarient::TestMessageResponse {},
                },
            });
            return true;
        } else {
            return false;
        }
    }

    fn handle_global_op(&mut self, op: &GlobalHandlerOperation) {
        if let GlobalHandlerOperation::MsgAll {msg} = op {
            self.pending.push(
                HandlerOperation::Client {
                    msg: msg.clone()
                }
            );
        }
    }

    fn get_global_operations(&mut self) -> Option<Vec<GlobalHandlerOperation>> {
        if self.pending_global.len() == 0 {
            return None;
        } else {
            let res = Some(self.pending_global.clone());
            self.pending_global.clear();
            return res;
        }
    }

    fn get_operations(&mut self) -> Option<Vec<Self::Operation>> {
        if self.pending.len() == 0 {
            return None;
        } else {
            let res = Some(self.pending.clone());
            self.pending.clear();
            return res;
        }
    }

    fn get_default_operations(&mut self) -> Vec<Self::Operation> {
        vec![]
    }
}
