use crate::handlers::imports::{GlobalHandlerOperation, HandlerOperation, MessageHandler};
use fracture_core::msg::MessageVarient::ServerSendChat;
use crate::types::{InterfaceOperation, ChatMessage};

pub struct IncomingChatHandler {
    pending: Vec<HandlerOperation>,
}

impl MessageHandler for IncomingChatHandler {
    type Operation = HandlerOperation;

    fn new() -> Box<Self> {
        Box::new(Self { pending: vec![] })
    }

    fn handle(&mut self, msg: &fracture_core::msg::Message) -> bool {
        if let ServerSendChat {author, content, author_uuid} = msg.data.clone() {
            println!("Received chat (uuid: {}): <{}> {} ", author_uuid, author, content);
            self.pending.push(
                HandlerOperation::InterfaceOperation(
                    InterfaceOperation::ReceivedChat(
                        ChatMessage::try_from(msg.data.clone()).unwrap()
                    )
                )
            );
            true
        } else {
            false
        }
    }

    fn handle_global_op(&mut self, _op: &GlobalHandlerOperation) {}
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

    fn get_default_operations(&mut self) -> Vec<Self::Operation> {vec![]}
}
