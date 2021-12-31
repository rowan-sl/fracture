pub mod imports;
pub mod modules;

use crate::interface::core::{HandlerOperation, ClientInfo};
use fracture_core::handler::{ServerMessageHandler, MessageHandler};

/// Current handlers for the client
pub fn get_default() -> Vec<Box<dyn ServerMessageHandler<ClientData = ClientInfo, Operation = HandlerOperation>  + Send>> {
    vec![
        // modules::test_handler::TestHandler::new(),
        modules::impl_msg_all::MsgAllHandler::new(),
        modules::incoming_chat::IncomingChatHandler::new(),
    ]
}
