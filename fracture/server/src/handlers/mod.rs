pub mod imports;
pub mod modules;

use crate::interface::core::HandlerOperation;
use fracture_core::handler::MessageHandler;

/// Current handlers for the client
pub fn get_default_handlers() -> Vec<Box<dyn MessageHandler<Operation = HandlerOperation> + Send>> {
    vec![modules::test_handler::TestHandler::new()]
}
