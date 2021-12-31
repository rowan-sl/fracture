use crate::handlers::imports::{GlobalHandlerOperation, HandlerOperation, MessageHandler};
use crate::types::{RawMessage, InterfaceOperation};
use fracture_core::msg::MessageVarient::ServerInfoUpdate;
use fracture_core::msg::types::UserNameUpdate;
use crate::conf::SHOW_USERS_UUIDS;

pub struct InfoUpdateHandler {
    pending: Vec<HandlerOperation>,
}

impl MessageHandler for InfoUpdateHandler {
    type Operation = HandlerOperation;

    fn new() -> Box<Self> {
        Box::new(Self { pending: vec![] })
    }

    fn handle(&mut self, msg: &fracture_core::msg::Message) -> bool {
        if let ServerInfoUpdate {
            name: _,//TODO this SHOULD not be important, as name chaning isnt done yet
            user_updates,
        } = msg.data.clone()
        {
            for update in user_updates {
                self.pending.push(
                    HandlerOperation::InterfaceOperation(
                        InterfaceOperation::ReceivedRawMessage(
                            match update {
                                UserNameUpdate::NewUser { uuid } => {
                                    if !SHOW_USERS_UUIDS {return true;}
                                    let dconv_uuid = uuid::Uuid::from_u128(uuid);
                                    RawMessage::new(
                                        format!("New user joined with uuid {}", dconv_uuid)
                                    )
                                }
                                UserNameUpdate::UserNamed { uuid, name } => {
                                    let dconv_uuid = uuid::Uuid::from_u128(uuid);
                                    if SHOW_USERS_UUIDS {
                                        RawMessage::new(
                                            format!("User {} named themselves {}", dconv_uuid, name)
                                        )
                                    } else {
                                        RawMessage::new(
                                            format!("{} joined the server.", name)//join message here, since it was not shown earlyer
                                        )
                                    }
                                }
                                UserNameUpdate::UserLeft { uuid, name } => {
                                    let dconv_uuid = uuid::Uuid::from_u128(uuid);
                                    if let Some(name) = name {
                                        RawMessage::new(
                                            if SHOW_USERS_UUIDS {
                                                format!("{} (id: {}) left the server.", name, dconv_uuid)
                                            } else {
                                                format!("{} left the server.", name)
                                            }
                                        )
                                    } else {
                                        RawMessage::new(
                                            format!("unnamed user (id: {}) left the server.", dconv_uuid)//show UUID anyway, because there is nothing left to identify it by
                                        )
                                    }
                                }
                            }
                        )
                    )
                );
            }
            true
        } else {
            false
        }
    }

    fn handle_global_op(&mut self, _op: &GlobalHandlerOperation) {}
    fn get_global_operations(&mut self) -> Option<Vec<GlobalHandlerOperation>> {
        None
    }

    fn get_operations(&mut self) -> Option<Vec<Self::Operation>> {
        if self.pending.is_empty() {
            None
        } else {
            let res = Some(self.pending.clone());
            self.pending.clear();
            res
        }
    }

    fn get_default_operations(&mut self) -> Vec<Self::Operation> {
        vec![]
    }
}