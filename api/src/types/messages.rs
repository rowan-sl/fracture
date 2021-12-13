use serde::{ Deserialize, Serialize };


#[derive(Deserialize, Serialize)]
pub enum MessageType {
    HelloThere,
    Goodbye,
    Text { text: String },
}


#[derive(Deserialize, Serialize)]
pub struct Message {
    msg_type: MessageType,
}
