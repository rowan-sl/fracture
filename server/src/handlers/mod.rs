pub mod imports;
pub mod modules;

use api::handler::MessageHandler;
use crate::interface::core::HandlerOperation;


/// Current handlers for the client
const default_handlers: Vec<Box<dyn MessageHandler<Operation = HandlerOperation> + Send>> = vec![

];