use tokio::net::TcpStream;
use tokio::sync::broadcast::Sender;
use tokio::task;

use fracture_core::handler::GlobalHandlerOperation;
use fracture_core::stat;
use fracture_core::utils::wait_update_time;

use crate::handlers::get_default;
use crate::interface::core::{stati, ClientInterface};
use crate::argparser;

#[derive(Clone, Debug)]
pub struct ShutdownMessage {
    pub reason: String,
}

pub async fn handle_client(
    socket: TcpStream,
    addr: std::net::SocketAddr,
    shutdown_sender: &Sender<ShutdownMessage>,
    global_handler_channel: Sender<GlobalHandlerOperation>,
    args: argparser::ParsedArgs,
) -> task::JoinHandle<()> {
    let mut client_shutdown_channel = shutdown_sender.subscribe(); //make shure to like and
    tokio::spawn(async move {
        let mut interface = ClientInterface::new(
            socket,
            String::from(args.name),
            get_default(),
            global_handler_channel.clone(),
        );
        println!(
            "Connected to {:?}, reported ip {:?}",
            addr,
            interface.get_client_addr().unwrap()
        );
        let _ = global_handler_channel.send(GlobalHandlerOperation::ClientConnect {
            uuid: interface.uuid(),
        });
        println!("client ID for {:?} is {:?}", addr, interface.uuid());
        loop {
            tokio::select! {
                stat = interface.update_read() => {
                    match stat {
                        stati::UpdateReadStatus::Disconnected => {
                            println!("{:?} disconnected", addr);
                            interface.close(String::from(""), None).await;// do not notify the client of disconnecting, as it is already disconnected
                            break;
                        },
                        stati::UpdateReadStatus::ReadError ( err_or_disconnect ) => {
                            use stat::ReadMessageError::{DeserializationError, HeaderParser, ReadError, Disconnected};
                            match err_or_disconnect {
                                Disconnected => {}, // already handeled, should never occur
                                err => {
                                    match err {
                                        DeserializationError(bincode_err) => {
                                            eprintln!("Recevied malformed or incomplete message from client! (could not deserialize) error folows:\n{:#?}", bincode_err);
                                        }
                                        HeaderParser(parser_err) => {
                                            eprintln!("Recevied malformed or incomplete message from client! (header parser error) error folows:\n{:#?}", parser_err);
                                        }
                                        ReadError(read_err) => {
                                            eprintln!("Error while reading message! (read error) error folows:\n{:#?}", read_err);
                                        }
                                        Disconnected => panic!("wtf rust")
                                    }
                                }
                            }
                        },
                        stati::UpdateReadStatus::GracefullDisconnect => {
                            println!("{:?} gracefully disconnected", addr);
                            interface.close(String::from(""), Some(fracture_core::msg::types::ServerDisconnectReason::ClientRequestedDisconnect)).await;
                            break;
                        },
                        stati::UpdateReadStatus::Sucsess => {}
                    };
                }
                _ = wait_update_time() => {//update loop
                    match interface.update().await {
                        stati::UpdateStatus::ClientKicked (reason) => {
                            //TODO this should actualy never happen, so remove it or implement it
                            eprintln!("Kicked client for invalid connection!");
                            interface.close(reason, Some(fracture_core::msg::types::ServerDisconnectReason::InvalidConnectionSequence)).await;
                            break;
                        }
                        stati::UpdateStatus::Unexpected (msg) => {
                            //TODO make this a error
                            eprintln!("Unexpected message {:#?}", msg);
                        }
                        stati::UpdateStatus::Unhandled (msg) => {
                            //TODO make this a error too
                            eprintln!("Unhandled message:\n{:#?}", msg);
                        }
                        stati::UpdateStatus::SendError(err) => {
                            eprintln!("Send error: {:#?}", err);
                            interface.close(String::from(""), None).await;
                            break;
                        }
                        _ => {}//these should be Noop and Success, so no issue ignoring them
                    };
                    interface.collect_actions();
                    loop {
                        match interface.execute_action().await {
                            Err(oper) => {
                                match oper {
                                    Some(unexpected_op) => {
                                        // a message was not explicitly pased on or dealt with
                                        //TODO make this a error
                                        println!("Unhandled operation:\n{:#?}", unexpected_op);
                                    }
                                    None => {
                                        break;
                                    }
                                }
                            }
                            Ok(pos_msg) => {
                                if let Some(_msg) = pos_msg {
                                    //TODO add handling things here
                                }
                            }
                        };
                    }
                    interface.colloect_send_global_actions();
                    interface.collect_recv_global_actions();
                    interface.execute_global_actions();
                    if let stati::MultiSendStatus::Failure(err) = interface.send_all_queued().await {
                        match err {
                            stat::SendStatus::Failure (ioerr) => {
                                if ioerr.kind() == std::io::ErrorKind::NotConnected {
                                    println!("{:?} disconnected", addr);
                                    interface.close(String::from(""), None).await;// do not notify the client of disconnecting, as it is already disconnected
                                } else {
                                    panic!("Error while sending messages:\n{:#?}", ioerr);
                                }
                                break;
                            }
                            stat::SendStatus::SeriError (serr) => {
                                panic!("Could not serialize message:\n{:#?}", serr);
                            }
                            _ => {}
                        }
                    }
                }
                smsg = client_shutdown_channel.recv() => {
                    println!("Closing connection to {:?}", addr);
                    interface.close(smsg.unwrap().reason, None).await;
                    break;
                }
            };
        }

        let _ = global_handler_channel.send(GlobalHandlerOperation::ClientDisconnect {
            uuid: interface.uuid(),
            name: interface.name(),
        });

        println!("Connection to {:?} closed", addr);
    })
}
