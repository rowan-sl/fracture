use std::sync::mpsc::{Receiver as MPSCReceiver, Sender as MPSCSender};

use iced::{Align, Element, Row, Text};

use crate::ui::types::GUIMessage;

pub mod stati {
    use fracture_core::msg;
    use thiserror::Error;

    #[derive(Debug)]
    pub enum MultiSendStatus {
        Worked { amnt: u32, bytes: u128 },
        NoTask,
    }

    #[derive(Debug, Error)]
    #[error("Failed to send message: {error}")]
    pub struct MultiSendError {
        #[from]
        pub error: fracture_core::stat::SendError,
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

    #[derive(Debug, Error)]
    pub enum UpdateReadError {
        #[error("Server closed: {close_message}")]
        ServerClosed {
            reason: fracture_core::msg::types::ServerDisconnectReason,
            close_message: String,
        },
        #[error("Server disconnected")]
        ServerDisconnect,
        #[error("Failed to read message: {0}")]
        ReadError(#[from] fracture_core::stat::ReadMessageError),
    }

    pub enum UpdateStatus {
        Success,
        Noop,
    }

    #[derive(Error, Debug)]
    pub enum UpdateError {
        /// This means that the message was late, HOWEVER it will be reycled untill it is dealt with
        #[error("Unexpected message {:?}", 0)]
        Unexpected(msg::Message),
        #[error("Failed to send message: {0}")]
        SendError(#[from] fracture_core::stat::SendError),
        #[error("Unhandled message: {:?}", 0)]
        Unhandled(msg::Message),
        #[error("Connection refused")]
        ConnectionRefused,
    }
}

pub trait ChatViewable<T> {
    fn view(&mut self) -> Element<T>;
}

#[derive(Clone, Debug)]
pub struct ChatMessage {
    msg_text: String,
    author_name: String,
    pub author_uuid: Option<uuid::Uuid>,
}

impl ChatMessage {
    pub fn new(msg_text: String, author_name: String) -> Self {
        Self {
            msg_text,
            author_name,
            author_uuid: None,
        }
    }
}

impl ChatViewable<GUIMessage> for ChatMessage {
    fn view(&mut self) -> Element<GUIMessage> {
        Row::new()
            .align_items(Align::Start)
            .spacing(4)
            .padding(3)
            .push(Text::new(self.author_name.clone() + ": "))
            .push(Text::new(self.msg_text.clone()))
            .into()
    }
}

impl TryFrom<fracture_core::msg::MessageVarient> for ChatMessage {
    type Error = ();
    fn try_from(item: fracture_core::msg::MessageVarient) -> Result<Self, Self::Error> {
        use fracture_core::msg::MessageVarient::ServerSendChat;
        match item {
            ServerSendChat {
                content,
                author,
                author_uuid,
            } => Ok(Self {
                msg_text: content,
                author_name: author,
                author_uuid: Some(uuid::Uuid::from_u128(author_uuid)),
            }),
            _ => Err(()),
        }
    }
}

impl From<ChatMessage> for fracture_core::msg::Message {
    fn from(item: ChatMessage) -> fracture_core::msg::Message {
        fracture_core::msg::Message {
            data: fracture_core::msg::MessageVarient::ClientSendChat {
                content: item.msg_text,
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct RawMessage {
    text: String,
}

impl RawMessage {
    pub fn new(text: String) -> Self {
        Self { text }
    }
}

impl ChatViewable<GUIMessage> for RawMessage {
    fn view(&mut self) -> Element<GUIMessage> {
        Row::new()
            .align_items(Align::Start)
            .spacing(4)
            .padding(3)
            .push(Text::new(self.text.clone()))
            .into()
    }
}

/// For stuff to interact with the user
#[derive(Clone, Debug)]
pub enum InterfaceOperation {
    ReceivedChat(ChatMessage),
    ReceivedRawMessage(RawMessage),
}

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

#[derive(Clone)]
pub struct ServerInfo {
    pub name: String,
    pub client_uuid: uuid::Uuid,
}

#[derive(Clone, Debug)]
pub struct ShutdownMessage {}

#[derive(Debug, Clone)]
pub enum CommMessage {
    //GUI -> Comm
    SendChat(ChatMessage),
    //Comm -> GUI
    HandleChat(ChatMessage),
    RawMessage(RawMessage),
    ServerInfo {
        server_name: String,
    }
}

pub struct CommChannels {
    pub sending: MPSCSender<CommMessage>,
    pub receiving: MPSCReceiver<CommMessage>,
}
