use tokio::io::{self};
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::task;
use tokio::sync::broadcast::*;

use api::utils::*;

mod client_interface;
use client_interface::*;
mod conf;
use conf::*;

#[derive(Clone, Debug)]
struct ShutdownMessage {
    reason: String
}

async fn handle_client(socket: TcpStream, addr: std::net::SocketAddr, shutdown_sender: &Sender<ShutdownMessage>) -> task::JoinHandle<()> {
    let mut client_shutdown_channel = shutdown_sender.subscribe();//make shure to like and
    tokio::spawn( async move {
        let mut interface = ClientInterface::new(socket, String::from(NAME));
        println!("Connected to {:?}, reported ip {:?}", addr, interface.get_client_addr().unwrap());
        loop {
            tokio::select! {
                stat = interface.update_read() => {
                    match stat {
                        stati::UpdateReadStatus::Disconnected => {
                            println!("{:?} disconnected", addr);
                            interface.close(String::from(""), true).await;// do not notify the client of disconnecting, as it is already disconnected
                            break;
                        },
                        stati::UpdateReadStatus::ReadError ( err ) => {
                            match err {
                                stati::ReadMessageError::Disconnected => {}, // already handeled, should never occur
                                oerr => {
                                    panic!("Failed to read message! {:#?}", oerr);
                                }
                            }
                        },
                        stati::UpdateReadStatus::Sucsess => {}
                    }
                }
                smsg = client_shutdown_channel.recv() => {
                    println!("Closing connection to {:?}", addr);
                    interface.update_process_all().await;
                    interface.close(smsg.unwrap().reason, false).await;
                    break;
                }
            };

            interface.update_process_all().await;
            interface.send_all_queued().await;
        };
        println!("Connection to {:?} closed", addr);
    })
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let (shutdown_tx, mut accepter_shutdown_rx): (Sender<ShutdownMessage>, Receiver<ShutdownMessage>) = channel(5);
    let ctrlc_transmitter = shutdown_tx.clone();

    let accepter_task: task::JoinHandle<io::Result<()>> = tokio::spawn(async move {
        // TODO make address configurable
        let listener = TcpListener::bind(ADDR).await?;
        println!("Started listening on {}, join this server with code {}", ADDR, ipencoding::ip_to_code(ADDR.parse::<std::net::SocketAddrV4>().unwrap()));
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
                        task.await.unwrap();
                    }
                    break;
                }
            };
        };
        Ok(())
    });

    let wait_for_ctrlc: task::JoinHandle<io::Result<()>> = tokio::spawn(async move {
        let sig_res = tokio::signal::ctrl_c().await;
        println!("\nRecieved ctrl+c, shutting down");
        if ctrlc_transmitter.receiver_count() == 0 {
            return sig_res;
        }
        ctrlc_transmitter.send(ShutdownMessage {reason: String::from("Server closed")}).unwrap();
        sig_res
    });

    let (_ctrlc_res, _accepter_res) = tokio::join!(wait_for_ctrlc, accepter_task);
    Ok(())
}