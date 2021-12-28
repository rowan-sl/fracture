mod client;
mod conf;
mod handlers;
mod types;

use std::thread;
use std::sync::mpsc::{channel as std_channel, Receiver as StdReceiver, Sender as StdSender, TryRecvError};


use tokio::io;
use tokio::join;
use tokio::net::TcpStream;
use tokio::sync::broadcast::{channel, Receiver, Sender};
use tokio::task;
use tokio::task::JoinHandle;
use tokio::runtime::Builder;

use fracture_core::msg;
use fracture_core::stat;
use fracture_core::utils::wait_update_time;

use client::Client;
use handlers::get_default;
use types::{stati, ShutdownMessage};


#[derive(Debug)]
enum CommMessage {
    ConnectionFailed,
    ConnectionRefused,
    CTRLCExit,
}

struct TaskCloseMsg;

fn main() {
    let (comm_incoming_send, comm_incoming_recv) = std_channel::<CommMessage>();
    let (_comm_outgoing_send, comm_outgoing_recv) = std_channel::<CommMessage>();
    let (shutdown_send, shutdown_recv) = std_channel::<TaskCloseMsg>();

    let comm_res = thread::spawn(move || {
        let comm_ctx = Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        comm_ctx.block_on(comm_main(comm_incoming_send, comm_outgoing_recv));
    });

    let printer = thread::spawn(move || {
        loop {
            if shutdown_recv.try_recv().is_ok() {
                break;
            }
            match comm_incoming_recv.try_recv() {
                Ok(msg) => {
                    println!("{:#?}", msg);
                }
                Err(err) => {
                    if err == TryRecvError::Disconnected {
                        break;
                    }
                }
            }
        }
    });

    comm_res.join().unwrap();
    shutdown_send.send(TaskCloseMsg).unwrap();
    printer.join().unwrap();
}

async fn comm_main(comm_send: StdSender<CommMessage>, _comm_rcvr: StdReceiver<CommMessage>) {
    let stream = match TcpStream::connect(conf::ADDR).await {
        Ok(st) => st,
        Err(err) => {
            use std::io::ErrorKind::ConnectionRefused;

            let return_val =
            if err.kind() == ConnectionRefused {
                eprintln!("Connection Refused! The server may not be online, or there may be a problem with the network. Error folows:\n{:#?}", err);
                CommMessage::ConnectionRefused
            } else {
                eprintln!("Error while connecting:\n{:#?}", err);
                CommMessage::ConnectionFailed
            };
            eprintln!("Aborting!");
            comm_send.send(return_val).unwrap();
            return;
        }
    };
    let (shutdown_tx, _): (Sender<ShutdownMessage>, Receiver<ShutdownMessage>) = channel(5);
    let ctrlc_transmitter = shutdown_tx.clone();

    let _task_results = join!(
        get_main_task(shutdown_tx, stream),
        get_ctrlc_listener(ctrlc_transmitter)
    );
    comm_send.send(CommMessage::CTRLCExit).unwrap();
}

fn get_main_task(shutdown_tx: Sender<ShutdownMessage>, stream: TcpStream) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut close_rcv = shutdown_tx.subscribe();
        let mut client = Client::new(stream, get_default());
        loop {
            tokio::select! {
                stat = client.update_read() => {
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
                            _ => {panic!()}//this should not happen
                        }
                    }
                }
                _ = close_rcv.recv() => {
                    println!("Disconnecting from {:?}", conf::ADDR);
                    client.close(stati::CloseType::Graceful).await;
                    break;
                }
            };
        }
        println!("Exiting");
    })
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
