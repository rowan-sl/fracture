/// This is for functions to generate common messages
/// All functions should be inlined, but you do not need #[inline] for this, if lto is on in cargo.toml
use crate::messages::msg::*;

pub fn ping() -> Message {
    Message {
        data: MessageVarient::Ping,
    }
}

pub fn gen_connect(name: String) -> Message {
    Message {
        data: MessageVarient::ConnectMessage {
            name: name,
        }
    }
}
