pub mod stati {
    use fracture_core::msg;

    #[derive(Debug)]
    pub enum MultiSendStatus {
        Worked { amnt: u32, bytes: u128 },
        Failure(fracture_core::stat::SendStatus),
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
        pub msg: fracture_core::msg::Message,
        pub bytes: usize,
    }

    pub enum UpdateReadStatus {
        ServerClosed {
            reason: fracture_core::msg::types::ServerDisconnectReason,
            close_message: String,
        },
        ServerDisconnect,
        ReadError(fracture_core::stat::ReadMessageError),
        Success,
    }

    pub enum UpdateStatus {
        Unexpected(msg::Message),
        SendError(fracture_core::stat::SendStatus),
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
#[derive(Clone, Debug)]
pub enum HandlerOperation {
    /// Do a program operation
    #[allow(dead_code)]
    InterfaceOperation(InterfaceOperation),
    ServerMsg {
        msg: fracture_core::msg::Message,
    },
}

#[derive(Debug)]
pub enum ClientState {
    /// nothing has happened
    Begin,
    /// sent the connect message, waiting for a response
    Hanshake,
    /// Get all default operations
    GetHandlerDefaultOps,
    /// Ready for normal stuff
    Ready,
}

pub struct ServerInfo {
    pub name: String,
    pub client_uuid: uuid::Uuid,
}

#[derive(Clone, Debug)]
pub struct ShutdownMessage {}
