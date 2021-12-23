mod conf;
mod interface;

use tokio::{io, net::TcpListener, sync::broadcast, task};

use api::utils::ipencoding;

use conf::ADDR;
use interface::{handler::handle_client, handler::ShutdownMessage};

#[tokio::main]
async fn main() -> io::Result<()> {
    let (shutdown_tx, _): (
        broadcast::Sender<ShutdownMessage>,
        broadcast::Receiver<ShutdownMessage>,
    ) = broadcast::channel(5);

    let ctrlc_transmitter = shutdown_tx.clone();

    let accepter_task = get_client_listener(shutdown_tx);
    let wait_for_ctrlc = get_ctrlc_listener(ctrlc_transmitter);

    // wait for ctrl+c, and then send the shutdown message, then wait for the other task to finish
    let (_ctrlc_res, _accepter_res) = tokio::join!(wait_for_ctrlc, accepter_task);
    Ok(())
}

fn get_client_listener(
    shutdown_tx: broadcast::Sender<ShutdownMessage>,
) -> task::JoinHandle<io::Result<()>> {
    tokio::spawn(async move {
        let mut accepter_shutdown_rx = shutdown_tx.subscribe();
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
                    for task in &mut tasks {
                        let res = task.await;
                        if res.is_err() {
                            println!("Connection handler closed with error {:#?}", res);
                        }
                    }
                    break;
                }
            };
        }

        Ok(())
    })
}

fn get_ctrlc_listener(
    ctrlc_transmitter: broadcast::Sender<ShutdownMessage>,
) -> task::JoinHandle<io::Result<()>> {
    tokio::spawn(async move {
        let sig_res = tokio::signal::ctrl_c().await;
        println!("\nRecieved ctrl+c, shutting down");
        if ctrlc_transmitter.receiver_count() == 0 {
            ctrlc_transmitter
                .send(ShutdownMessage {
                    reason: String::from("Server closed"),
                })
                .unwrap();
        }
        sig_res
    })
}
