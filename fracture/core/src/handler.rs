#[derive(Clone, Debug)]
pub enum GlobalHandlerOperation {
    MsgAll { msg: crate::msg::Message },
}

/// Generic trait for createing a message handler.
/// All handlers must be `Send`
pub trait MessageHandler {
    type Operation;

    fn new() -> Box<Self>
    where
        Self: Sized;

    /// takes a message, potentialy handleing it.
    /// returns wether or not the message was handled (should the interface attempt to continue trying new handlers to handle it)
    fn handle(&mut self, msg: &crate::msg::Message) -> bool;

    /// Takes a `GlobalHandlerOperation` and does any necesary operations for it.
    /// Does not return anything
    fn handle_global_op(&mut self, op: &GlobalHandlerOperation);

    /// Get operations that the handler is requesting the interface do
    fn get_operations(&mut self) -> Option<Vec<Self::Operation>>;

    /// Get all operations that are performed on startup, with no prompting
    ///
    /// These are performed after the auth/handshake step
    fn get_default_operations(&mut self) -> Vec<Self::Operation>;

    /// Get all global operations the handler is requesting be performed
    fn get_global_operations(&mut self) -> Option<Vec<GlobalHandlerOperation>>;
}

/// Trait for a handler that requires info about the server
/// all server handlers must implement this
pub trait ServerClientInfo {
    type ClientData;

    fn accept_client_data(&mut self, data: Self::ClientData);
}

pub trait ServerMessageHandler: MessageHandler + ServerClientInfo {}
