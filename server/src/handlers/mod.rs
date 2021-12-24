pub mod imports;
pub mod modules;

use api::handler::MessageHandler;
use crate::interface::core::HandlerOperation;


/// Current handlers for the client
pub fn get_default_handlers() -> Vec<Box<dyn MessageHandler<Operation = HandlerOperation> + Send>> {
    vec![
        modules::test_handler::TestHandler::new()
    ]
}