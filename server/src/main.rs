use tokio::io::{self};
use tokio::net::TcpListener;
use tokio::task;
use tokio::{ join, select };
use tokio::sync::broadcast;

mod client_interface;
use client_interface::*;
mod types;
use types::*;

#[tokio::main]
async fn main() -> io::Result<()> {
    let (progsig_tx, _progsig_rx): (broadcast::Sender<ProgramMessage>, broadcast::Receiver<ProgramMessage>) = broadcast::channel(10);
    let ctrlc_transmitter = progsig_tx.clone();

    let accepter_task: task::JoinHandle<io::Result<()>> = tokio::spawn(async move {
        // TODO make address configurable
        let listener = TcpListener::bind("127.0.0.1:6142").await?;
        let mut sysmsg_rcv = progsig_tx.subscribe();
        let mut tasks: Vec<task::JoinHandle<()>> = vec![];

        loop {
            println!("E");
            select! {
                accepted_sock = listener.accept() => {
                    match accepted_sock {
                        Ok(socket_addr) => {
                            let reciever = progsig_tx.subscribe();
                            let (socket, _addr) = socket_addr;

                            tasks.push(
                                tokio::spawn(
                                    async move {
                                        let mut interface = ClientInterface::new(socket, reciever);
                                        println!("Connected to {:?}", interface.get_client_addr().unwrap());
                                        loop {
                                            println!("Reading from sock");
                                            // TODO fix as the get_message_from_socket will block untill it gets a new message
                                            let stat = interface.update().await;
                                            println!("{:?}", stat);
                                            match stat {
                                                Ok(stat) => {
                                                    match stat {
                                                        UpdateStatus::Shutdown => {
                                                            break;
                                                        },
                                                        _ => {}
                                                    }
                                                },
                                                Err(err) => {
                                                    println!("Update client error: {:#?}", err);
                                                }
                                            }
                                            println!("Read from sock");
                                        }
                                    }
                                )
                            );
                        },
                        Err(err) => {
                            println!("Error while looking for a client {:?}", err);
                        }
                    }
                }
                recieved = sysmsg_rcv.recv() => {
                    println!("Recieved message");
                    match recieved {
                        Ok(msg) => {
                            match msg {
                                ProgramMessage::Shutdown => {
                                    //TODO shutdown logic, make this wait untill all client things shutdown corectly
                                    for task in tasks.iter_mut() {
                                        println!("Shutting down {:?}", task);
                                        let status = task.abort();
                                        println!("Shut down with status: {:?}", status);
                                        // match status {
                                        //     Ok(stat) => {
                                        //         println!("{:?}", stat);
                                        //     },
                                        //     Err(err) => {
                                        //         panic!("Failed to cancel client {:?}", err);
                                        //     }
                                        // }
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            println!("Error reading system message channel\n{:?}", err);
                        }
                    }
                    println!("Done");
                    break;
                }
            };
        }
        println!("returing");
        return Ok(());
    });
    let wait_for_ctrlc: task::JoinHandle<io::Result<()>> = tokio::spawn(async move {
        let sig_res = tokio::signal::ctrl_c().await;
        println!("Recieved ctrl+c");
        ctrlc_transmitter.send(ProgramMessage::Shutdown).unwrap();
        println!("Sent shutdown msg");
        sig_res
    });
    tokio::join!(accepter_task, wait_for_ctrlc);
    println!("program done");
    return Ok(());
}