mod client;
mod conf;
mod types;

use tokio::io;
use tokio::join;
use tokio::net::TcpStream;
use tokio::sync::broadcast::{channel, Receiver, Sender};
use tokio::task;

use api::msg;

use client::*;
use types::*;

#[tokio::main]
async fn main() {
    let stream = TcpStream::connect(conf::ADDR).await.unwrap();
    let (shutdown_tx, _): (Sender<ShutdownMessage>, Receiver<ShutdownMessage>) = channel(5);
    let ctrlc_transmitter = shutdown_tx.clone();

    let main_task = tokio::spawn(async move {
        let mut close_rcv = shutdown_tx.subscribe();
        let mut client = Client::new(stream, vec![]);
        loop {
            tokio::select! {
                stat = client.update_read() => {
                    match stat {
                        stati::UpdateReadStatus::ServerClosed { reason, close_message } => {
                            use msg::types::ServerDisconnectReason;
                            match reason {
                                ServerDisconnectReason::ClientRequestedDisconnect => {
                                    println!("this should never happen...");
                                }
                                ServerDisconnectReason::Closed => {
                                    println!("Server closed with message {}", close_message);
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
                        stati::UpdateReadStatus::Success => {
                            //TODO implement client update logic here
                            match client.update().await {
                                stati::UpdateStatus::ConnectionRefused => {
                                    //TODO this should actualy never happen, so remove it or implement it
                                    println!("Connection to server refused!");
                                    client.close(stati::CloseType::ServerDisconnected).await;
                                    break;
                                }
                                stati::UpdateStatus::Unexpected (msg) => {
                                    //TODO make this a error
                                    println!("Unexpected message {:#?}", msg);
                                }
                                stati::UpdateStatus::Unhandled (msg) => {
                                    //TODO make this a error too
                                    println!("Unhandled message:\n{:#?}", msg);
                                }
                                _ => {}//these should be Noop and Success, so no issue ignoring them
                            };
                            client.collect_actions();
                            loop {
                                match client.execute_action().await {
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
                                        match pos_msg {
                                            Some(_msg) => {
                                                //TODO add handling things here
                                            }
                                            None => {}
                                        };
                                    }
                                };
                            }
                            match client.send_all_queued().await {
                                stati::MultiSendStatus::Failure (ms_err) => {
                                    match ms_err {
                                        stati::SendStatus::Failure (err) => {
                                            if err.kind() == std::io::ErrorKind::NotConnected {
                                                println!("Disconnected!");
                                            } else {
                                                eprintln!("Error while sending message:\n{:#?}", err);
                                            }
                                            client.close(stati::CloseType::ServerDisconnected).await;
                                            break;
                                        }
                                        stati::SendStatus::SeriError (err) => {
                                            panic!("Could not serialize msessage:\n{:#?}", err);
                                        }
                                        _ => {panic!()}//this should not happen
                                    }
                                }
                                _ => {}//just nothing to do or worked, we dont care
                            }
                        }
                    };
                }
                _ = close_rcv.recv() => {
                    println!("Disconnecting from {:?}", conf::ADDR);
                    client.close(stati::CloseType::Graceful).await;
                    break;
                }
            };
        }
        println!("Exiting");
    });
    let _ = join!(main_task, get_ctrlc_listener(ctrlc_transmitter));
}

fn get_ctrlc_listener(
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
