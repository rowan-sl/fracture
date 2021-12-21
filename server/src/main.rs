mod client_handler;
mod client_interface;
mod client_tracker;
mod conf;

use tokio::io::{self};
use tokio::net::TcpListener;
use tokio::sync::broadcast::*;
use tokio::task;

use api::utils::*;

use client_handler::*;
use conf::*;

#[tokio::main]
async fn main() -> io::Result<()> {
    let (shutdown_tx, mut accepter_shutdown_rx): (
        Sender<ShutdownMessage>,
        Receiver<ShutdownMessage>,
    ) = channel(5);
    let ctrlc_transmitter = shutdown_tx.clone();

    let accepter_task: task::JoinHandle<io::Result<()>> = tokio::spawn(async move {
        // TODO make address configurable
        let listener = TcpListener::bind(ADDR).await?;
        println!(
            "Started listening on {}, join this server with code {}",
            ADDR,
            ipencoding::ip_to_code(ADDR.parse::<std::net::SocketAddrV4>().unwrap())
        );
        let mut tasks: Vec<task::JoinHandle<()>> = vec![];

        loop {
            tokio::select! {
                accepted_sock = listener.accept() => {
                    match accepted_sock {
                        Ok(socket_addr) => {
                            let (socket, addr) = socket_addr;

                            tasks.push(handle_client(socket, addr, &shutdown_tx).await);
                        },
                        Err(err) => {
                            println!("Error while accepting a client {:?}", err);
                        }
                    };
                }
                _ = accepter_shutdown_rx.recv() => {
                    for task in tasks.iter_mut() {
                        let stat = task.await;
                        match stat {
                            Ok(_) => {},
                            Err(err) => {
                                println!("Connection handler closed with error {:#?}", err);
                            }
                        };
                    }
                    break;
                }
            };
        }
        Ok(())
    });

    let wait_for_ctrlc: task::JoinHandle<io::Result<()>> = tokio::spawn(async move {
        let sig_res = tokio::signal::ctrl_c().await;
        println!("\nRecieved ctrl+c, shutting down");
        if ctrlc_transmitter.receiver_count() == 0 {
            return sig_res;
        }
        ctrlc_transmitter
            .send(ShutdownMessage {
                reason: String::from("Server closed"),
            })
            .unwrap();
        sig_res
    });

    let (_ctrlc_res, _accepter_res) = tokio::join!(wait_for_ctrlc, accepter_task);
    Ok(())
}
