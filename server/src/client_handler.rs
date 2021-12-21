use tokio::net::TcpStream;
use tokio::sync::broadcast::*;
use tokio::task;

use crate::client_interface:: { ClientInterface, stati };
use crate::conf::NAME;

#[derive(Clone, Debug)]
pub struct ShutdownMessage {
    pub reason: String,
}

pub async fn handle_client(
    socket: TcpStream,
    addr: std::net::SocketAddr,
    shutdown_sender: &Sender<ShutdownMessage>,
) -> task::JoinHandle<()> {
    let mut client_shutdown_channel = shutdown_sender.subscribe(); //make shure to like and
    tokio::spawn(async move {
        let mut interface = ClientInterface::new(socket, String::from(NAME), vec![]);
        println!(
            "Connected to {:?}, reported ip {:?}",
            addr,
            interface.get_client_addr().unwrap()
        );
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
        }
        println!("Connection to {:?} closed", addr);
    })
}