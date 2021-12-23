pub mod stati {
    use api::msg;

    #[derive(Debug)]
    pub enum MultiSendStatus {
        Worked { amnt: u32, bytes: u128 },
        Failure(api::stat::SendStatus),
        NoTask,
    }

    #[derive(Debug)]
    pub enum CloseType {
        /// Do not notify the server, just close quickly
        Force,
        /// Close, but assume the server has already closed, so dont attempt sending disconnect msg
        ServerDisconnected,
        /// close properly
        Graceful,
    }

    #[derive(Debug)]
    pub struct ReadMessageStatus {
        pub msg: api::msg::Message,
        pub bytes: usize,
    }

    pub enum UpdateReadStatus {
        ServerClosed {
            reason: api::msg::types::ServerDisconnectReason,
            close_message: String,
        },
        ServerDisconnect,
        ReadError(api::stat::ReadMessageError),
        Success,
    }

    pub enum UpdateStatus {
        Unexpected(msg::Message),
        Success,
        Unhandled(msg::Message),
        ConnectionRefused,
        Noop,
    }
}

/// For stuff to interact with the user
#[derive(Clone, Copy, Debug)]
pub enum InterfaceOperation {}

//TODO add more of these
/// Operations that a `MessageHandler` can request occur
#[derive(Clone, Copy, Debug)]
pub enum HandlerOperation {
    /// Do a program operation
    #[allow(dead_code)]
    InterfaceOperation(InterfaceOperation),
}

/// Generic trait for createing a message handler.
/// All handlers must be Send
pub trait MessageHandler {
    fn new() -> Self
    where
        Self: Sized;

    /// takes a message, potentialy handleing it.
    /// returns wether or not the message was handled (should the interface attempt to continue trying new handlers to handle it)
    fn handle(&mut self, msg: &api::msg::Message) -> bool;

    /// Get operations that the handler is requesting the interface do
    fn get_operations(&mut self) -> Option<Vec<HandlerOperation>>;
}

pub enum ClientState {
    /// nothing has happened
    Begin,
    /// sent the connect message, waiting for a response
    Hanshake,
    /// Ready for normal stuff
    Ready,
}

pub struct ServerInfo {
    pub name: String,
    pub client_uuid: uuid::Uuid,
}

#[derive(Clone, Debug)]
pub struct ShutdownMessage {}
