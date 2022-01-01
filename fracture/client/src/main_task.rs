use std::sync::mpsc::{Receiver as MPSCReceiver, Sender as MPSCSender};

use tokio::io;
use tokio::join;
use tokio::net::TcpStream;
use tokio::sync::broadcast::{channel, Receiver, Sender};
use tokio::task;
use tokio::task::JoinHandle;

use fracture_core::msg;
use fracture_core::stat;
use fracture_core::utils::wait_update_time;

use crate::client::Client;
use crate::handlers::get_default;
use crate::types::{stati, ShutdownMessage};

use crate::{types, CommMessage};

#[derive(Debug)]
pub enum CommMainError {
    ConnectionRefused(std::io::Error),
    GenericConnectionError(std::io::Error),
}

pub async fn comm_main(
    comm_send: MPSCSender<CommMessage>,
    comm_recv: MPSCReceiver<CommMessage>,
    addr: std::net::SocketAddrV4,
    name: String,
) -> Result<(), CommMainError> {
    let stream = match TcpStream::connect(addr).await {
        Ok(st) => st,
        Err(err) => {
            use std::io::ErrorKind::ConnectionRefused;

            let return_val = if err.kind() == ConnectionRefused {
                eprintln!("Connection Refused! The server may not be online, or there may be a problem with the network. Error folows:\n{:#?}", err);
                Err(CommMainError::ConnectionRefused(err))
            } else {
                eprintln!("Error while connecting:\n{:#?}", err);
                Err(CommMainError::GenericConnectionError(err))
            };
            eprintln!("Aborting!");
            return return_val;
        }
    };
    let (shutdown_tx, _): (Sender<ShutdownMessage>, Receiver<ShutdownMessage>) = channel(5);
    let ctrlc_transmitter = shutdown_tx.clone();

    let _task_results = join!(
        get_main_task(shutdown_tx, stream, name, comm_send, comm_recv),
        get_ctrlc_listener(ctrlc_transmitter)
    );
    println!("Exited");
    Ok(())
}

pub fn get_main_task(
    shutdown_tx: Sender<ShutdownMessage>,
    stream: TcpStream,
    name: String,
    comm_send: MPSCSender<CommMessage>,
    comm_recv: MPSCReceiver<CommMessage>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut close_rcv = shutdown_tx.subscribe();
        let mut client = Client::new(stream, name, get_default());
        loop {
            tokio::select! {
                stat = client.update_read() => {//TODO make it so that update read is not cancled, because with update loop that could cause corruption (just wait for readable, not actualy to read teh whole thing)
                    match stat {
                        stati::UpdateReadStatus::ServerClosed { reason, close_message } => {
                            use msg::types::ServerDisconnectReason;
                            match reason {
                                ServerDisconnectReason::ClientRequestedDisconnect => {
                                    panic!("this should never happen...");
                                }
                                ServerDisconnectReason::Closed => {
                                    println!("Disconnected by server: {}", close_message);
                                }
                                ServerDisconnectReason::InvalidConnectionSequence => {
                                    println!("Kicked for invalid connection sequence:\n{}", close_message);
                                }
                            }
                            client.close(stati::CloseType::ServerDisconnected).await;
                            break;
                        }
                        stati::UpdateReadStatus::ServerDisconnect => {
                            println!("Server disconnected!");
                            client.close(stati::CloseType::ServerDisconnected).await;
                            break;
                        }
                        stati::UpdateReadStatus::ReadError (err) => {
                            eprintln!("Error whilst reading message!\n{:#?}", err);
                            client.close(stati::CloseType::Force).await;
                            break;
                        }
                        stati::UpdateReadStatus::Success => {}//dont do anything, updates are handled seperatly
                    };
                }
                _ = wait_update_time() => {// client update loop
                    while let Ok(cmsg) = comm_recv.try_recv() {
                        match cmsg {
                            CommMessage::SendChat(msg) => {
                                client.manual_handler_operation(types::HandlerOperation::ServerMsg{msg: msg.into()});
                            }
                            other => {
                                panic!("Client handler received unexpected CommMessage\n{:#?}", other);
                            }
                        }
                    }
                    match client.update().await {
                        stati::UpdateStatus::ConnectionRefused => {
                            //TODO this should actualy never happen, so remove it or implement it
                            eprintln!("Connection to server refused!");
                            client.close(stati::CloseType::ServerDisconnected).await;
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
                            client.close(stati::CloseType::Force).await;
                            break;
                        }
                        _ => {}//these should be Noop and Success, so no issue ignoring them
                    };
                    client.collect_actions();
                    loop {
                        match client.execute_action().await {
                            Ok(pos_msg) => {
                                use types::{HandlerOperation, InterfaceOperation};
                                if let Some(msg) = pos_msg {
                                    match msg {
                                        HandlerOperation::InterfaceOperation (oper) => {
                                            match oper {
                                                InterfaceOperation::ReceivedChat (msg) => {
                                                    //Ignore messages from self
                                                    if msg.author_uuid.expect("message has a uuid") == client.server_info.clone().expect("glient has a uuid").client_uuid {
                                                        println!("Ignoring {:?} because it was sent by this client", msg);
                                                    } else {
                                                        comm_send.send(CommMessage::HandleChat(msg)).expect("GUI received CommMessage");
                                                    }
                                                }
                                                InterfaceOperation::ReceivedRawMessage (msg) => {
                                                    comm_send.send(CommMessage::RawMessage(msg)).expect("GUI received CommMessage");
                                                }
                                                #[allow(unreachable_patterns)]//not a problem
                                                unexpected => {panic!("unhandled InterfaceOperation:\n{:#?}", unexpected)}
                                            }
                                        }
                                        unexpected_msg => {panic!("client handler was asked to handle a operation it did not know about!\n{:#?}", unexpected_msg)}
                                    }
                                }
                            }
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
                        };
                    }
                    if let stati::MultiSendStatus::Failure (ms_err) = client.send_all_queued().await {//we only care about failure here
                        match ms_err {
                            stat::SendStatus::Failure (err) => {
                                if err.kind() == std::io::ErrorKind::NotConnected {
                                    println!("Disconnected!");
                                } else {
                                    eprintln!("Error while sending message:\n{:#?}", err);
                                }
                                client.close(stati::CloseType::ServerDisconnected).await;
                                break;
                            }
                            stat::SendStatus::SeriError (err) => {
                                panic!("Could not serialize msessage:\n{:#?}", err);
                            }
                            _ => {unreachable!()}//this should not happen
                        }
                    }
                }
                _ = close_rcv.recv() => {
                    println!("Disconnecting");
                    client.close(stati::CloseType::Graceful).await;
                    break;
                }
            };
        }
        println!("Exiting");
    })
}

pub fn get_ctrlc_listener(
    ctrlc_transmitter: Sender<ShutdownMessage>,
) -> task::JoinHandle<io::Result<()>> {
    tokio::spawn(async move {
        let sig_res = tokio::signal::ctrl_c().await;
        println!("\nRecieved ctrl+c, shutting down");
        if ctrlc_transmitter.receiver_count() == 0 {
            return sig_res;
        }
        ctrlc_transmitter.send(ShutdownMessage {}).unwrap();
        sig_res
    })
}
