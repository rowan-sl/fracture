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
        SendError(api::stat::SendStatus),
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
        msg: api::msg::Message,
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
