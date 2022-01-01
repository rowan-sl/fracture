use crate::types::CommChannels;

#[derive(Clone, Debug)]
pub enum GUIMessage {
    SubmitMessage,
    TextInputChanged(String),
    Ticked,
    Close,
}

pub struct FractureGUIFlags {
    pub comm: CommChannels,
    pub name: String,
}

impl FractureGUIFlags {
    pub fn new(comm: CommChannels, name: impl Into<String>) -> Self {
        FractureGUIFlags {
            comm,
            name: name.into(),
        }
    }
}
