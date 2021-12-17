use tokio::io::{self};
use tokio::net::TcpListener;
use tokio::task;
use tokio::sync::broadcast::*;

mod client_interface;
use client_interface::*;

#[derive(Clone, Debug)]
struct ShutdownMessage {
    reason: String
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let (shutdown_tx, mut accepter_shutdown_rx): (Sender<ShutdownMessage>, Receiver<ShutdownMessage>) = channel(5);
    let ctrlc_transmitter = shutdown_tx.clone();

    let accepter_task: task::JoinHandle<io::Result<()>> = tokio::spawn(async move {
        // TODO make address configurable
        let listener = TcpListener::bind("127.0.0.1:6142").await?;
        let mut tasks: Vec<task::JoinHandle<()>> = vec![];

        loop {
            tokio::select! {
                accepted_sock = listener.accept() => {
                    let mut client_shutdown_channel = shutdown_tx.subscribe();// make shure to like and

                    match accepted_sock {
                        Ok(socket_addr) => {
                            let (socket, addr) = socket_addr;

                            tasks.push( tokio::spawn( async move {
                                let mut interface = ClientInterface::new(socket);
                                println!("Connected to {:?}", interface.get_client_addr().unwrap());
                                loop {
                                    println!("Reading from sock");
                                    tokio::select! {
                                        // if this is removed, then shutting down works, so thats a thing
                                        stat = interface.update_read() => {
                                            match stat {
                                                Ok(_stat) => {

                                                },
                                                Err(err) => {
                                                    match err {
                                                        UpdateReadError::Disconnected => {
                                                            break;
                                                        }
                                                        _ => {}
                                                    }
                                                }
                                            }
                                        }
                                        smsg = client_shutdown_channel.recv() => {
                                            println!("Sending shutdown msg to client");
                                            interface.update_process_all().await;
                                            interface.close(smsg.unwrap().reason).await;
                                            break;
                                        }
                                    };

                                    interface.update_process_all().await;
                                    interface.send_all_queued().await;
                                    println!("Read from sock");
                                };
                                println!("Connection to {:?} closed", addr);
                            }));
                        },
                        Err(err) => {
                            println!("Error while looking for a client {:?}", err);
                        }
                    };
                }
                _ = accepter_shutdown_rx.recv() => {
                    println!("{:#?}", tasks);
                    for task in tasks.iter_mut() {
                        task.await.unwrap();
                    }
                    println!("{:#?}", tasks);
                    break;
                }
            };
        };
        Ok(())
    });

    let wait_for_ctrlc: task::JoinHandle<io::Result<()>> = tokio::spawn(async move {
        let sig_res = tokio::signal::ctrl_c().await;
        println!("Recieved ctrl+c");
        if ctrlc_transmitter.receiver_count() == 0 {
            return sig_res;
        }
        ctrlc_transmitter.send(ShutdownMessage {reason: String::from("Server closed")}).unwrap();
        println!("Sent shutdown msg");
        sig_res
    });

    let (_ctrlc_res, _accepter_res) = tokio::join!(wait_for_ctrlc, accepter_task);
    Ok(())
}