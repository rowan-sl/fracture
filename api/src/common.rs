/// This is for functions to generate common messages
/// All functions should be inlined, but you do not need #[inline] for this, if lto is on in cargo.toml
use crate::msg::{Message, MessageVarient};

#[must_use] pub const fn ping() -> Message {
    Message {
        data: MessageVarient::Ping,
    }
}

#[must_use] pub const fn gen_connect(name: String) -> Message {
    Message {
        data: MessageVarient::ConnectMessage { name },
    }
}
