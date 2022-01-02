mod argparser;
mod handlers;
mod interface;
use fracture_config::server as conf;

use log::{debug, error, info};

use tokio::{io, net::TcpListener, sync::broadcast, task};

use fracture_core::handler::GlobalHandlerOperation;
use fracture_core::utils::ipencoding;

use interface::{handler::handle_client, handler::ShutdownMessage};


#[derive(Debug)]
enum MainErr {
    ArgsError(argparser::GetArgsError),
}

impl From<argparser::GetArgsError> for MainErr {
    fn from(item: argparser::GetArgsError) -> Self {
        Self::ArgsError(item)
    }
}

#[tokio::main]
async fn main() -> Result<(), MainErr> {
    let logger_env = env_logger::Env::default()
    .filter("FRACTURE_LOG_LEVEL")
    .write_style("FRACTURE_LOG_STYLE");

    env_logger::Builder::from_env(logger_env)
        // .filter_level(args.level)
        .init();

    info!("Fracture Startup");

    let args = match argparser::get_args() {
        Ok(a) => a,
        Err(err) => {
            match err {
                argparser::GetArgsError::Exit => {return Ok(())}
                other => {return Err(other.into())}
            }
        }
    };

    println!("Launching server `{}` on `{}`", args.name, args.full_addr);

    let (shutdown_tx, _): (
        broadcast::Sender<ShutdownMessage>,
        broadcast::Receiver<ShutdownMessage>,
    ) = broadcast::channel(5);

    let ctrlc_transmitter = shutdown_tx.clone();

    let accepter_task = get_client_listener(shutdown_tx, args.clone());
    let wait_for_ctrlc = get_ctrlc_listener(ctrlc_transmitter);

    // wait for ctrl+c, and then send the shutdown message, then wait for the other task to finish
    let (_ctrlc_res, _accepter_res) = tokio::join!(wait_for_ctrlc, accepter_task);
    Ok(())
}

fn get_client_listener(
    shutdown_tx: broadcast::Sender<ShutdownMessage>,
    args: argparser::ParsedArgs,
) -> task::JoinHandle<io::Result<()>> {
    tokio::spawn(async move {
        let (global_oper_tx, _): (
            broadcast::Sender<GlobalHandlerOperation>,
            broadcast::Receiver<GlobalHandlerOperation>,
        ) = broadcast::channel(conf::GLOBAL_HANDLER_OP_LIMIT);
        let mut accepter_shutdown_rx = shutdown_tx.subscribe();
        // TODO make address configurable
        let listener = TcpListener::bind(args.full_addr.clone()).await?;
        println!(
            "Started listening on {:?}, join this server with code {:?}",
            listener.local_addr().unwrap().to_string(),
            ipencoding::ip_to_code(match listener.local_addr().unwrap() {
                std::net::SocketAddr::V4(addr) => {
                    addr
                }
                _ => panic!("got SocketAddrV6 instead of v4"),
            })
            .unwrap()
        );
        let mut tasks: Vec<task::JoinHandle<()>> = vec![];

        loop {
            tokio::select! {
                accepted_sock = listener.accept() => {
                    match accepted_sock {
                        Ok(socket_addr) => {
                            let (socket, addr) = socket_addr;

                            tasks.push(handle_client(socket, addr, &shutdown_tx, global_oper_tx.clone(), args.clone()).await);
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
                            eprintln!("Connection handler closed with error {:#?}", res);
                        }
                    }
                    println!("Closed all client interfaces");
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
        if ctrlc_transmitter.receiver_count() != 0 {
            ctrlc_transmitter
                .send(ShutdownMessage {
                    reason: String::from("Server closed"),
                })
                .unwrap();
        }
        sig_res
    })
}
