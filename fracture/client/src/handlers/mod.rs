pub mod imports;
pub mod modules;

use imports::{HandlerOperation, MessageHandler};

/// Current handlers for the client
pub fn get_default() -> Vec<Box<dyn MessageHandler<Operation = HandlerOperation> + Send>> {
    vec![
        // modules::test_handler::TestHandler::new()
        modules::incoming_chat::IncomingChatHandler::new(),
        modules::server_info_update::InfoUpdateHandler::new(),
    ]
}
