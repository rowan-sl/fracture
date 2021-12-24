use crate::handlers::imports::{MessageHandler, HandlerOperation};
use api::msg::MessageVarient::TestMessageResponse;

pub struct TestHandler {
    pending: Vec<HandlerOperation>,
}

impl MessageHandler for TestHandler {
    type Operation = HandlerOperation;

    fn new() -> Box<Self> {
        Box::new(
            Self {
                pending: vec![],
            }
        )
    }

    fn handle(&mut self, msg: &api::msg::Message) -> bool {
        if let TestMessageResponse {} = msg.data {
            println!("Received test message response");
            return true;
        } else {
            return false;
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
        vec![
            HandlerOperation::ServerMsg {
                msg: api::msg::Message{
                    data: api::msg::MessageVarient::TestMessage {}
                }
            }
        ]
    }
}