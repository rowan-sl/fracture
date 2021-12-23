use tokio::net::TcpStream;
use tokio::sync::broadcast::Sender;
use tokio::task;

use api::utils::wait_100ms;

use crate::conf::NAME;
use crate::interface::core::{stati, ClientInterface};

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
        println!("client ID for {:?} is {:?}", addr, interface.uuid());
        loop {
            tokio::select! {
                stat = interface.update_read() => {
                    match stat {
                        stati::UpdateReadStatus::Disconnected => {
                            println!("{:?} disconnected", addr);
                            interface.close(String::from(""), true, false).await;// do not notify the client of disconnecting, as it is already disconnected
                            break;
                        },
                        stati::UpdateReadStatus::ReadError ( err_or_disconnect ) => {
                            use stati::ReadMessageError::{DeserializationError, HeaderParser, ReadError, Disconnected};
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
                            interface.close(String::from(""), true, true).await;
                            break;
                        },
                        stati::UpdateReadStatus::Sucsess => {}
                    };
                }
                _ = wait_100ms() => {//update loop
                    interface.update_process_all().await;
                    if let stati::MultiSendStatus::Failure(err) = interface.send_all_queued().await {
                        match err {
                            stati::SendStatus::Failure (ioerr) => {
                                if ioerr.kind() == std::io::ErrorKind::NotConnected {
                                    println!("{:?} disconnected", addr);
                                    interface.close(String::from(""), true, false).await;// do not notify the client of disconnecting, as it is already disconnected
                                } else {
                                    panic!("Error while sending messages:\n{:#?}", ioerr);
                                }
                                break;
                            }
                            stati::SendStatus::SeriError (serr) => {
                                panic!("Could not serialize message:\n{:#?}", serr);
                            }
                            _ => {}
                        }
                    }
                }
                smsg = client_shutdown_channel.recv() => {
                    println!("Closing connection to {:?}", addr);
                    interface.update_process_all().await;
                    interface.close(smsg.unwrap().reason, false, false).await;
                    break;
                }
            };
        }
        println!("Connection to {:?} closed", addr);
    })
}
