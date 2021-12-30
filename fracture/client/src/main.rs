mod args;
mod client;
mod handlers;
mod mpscwatcher;
mod types;

mod imports {
    use super::*;
    pub use std::sync::mpsc::{
        channel as std_channel, Receiver as MPSCReceiver, Sender as MPSCSender, TryRecvError,
    };
    pub use std::thread;

    pub use tokio::io;
    pub use tokio::join;
    pub use tokio::net::TcpStream;
    pub use tokio::runtime::Builder;
    pub use tokio::sync::broadcast::{channel, Receiver, Sender};
    pub use tokio::task;
    pub use tokio::task::JoinHandle;

    pub use iced::{
        button, executor, scrollable, text_input, Align, Application, Button, Clipboard, Column,
        Command, Element, Length, Row, Scrollable, Settings, Space, Subscription, Text, TextInput,
    };

    pub use uuid::Uuid;

    pub use fracture_core::msg;
    pub use fracture_core::stat;
    pub use fracture_core::utils::wait_update_time;

    pub use args::get_args;
    pub use client::Client;
    pub use handlers::get_default;
    pub use types::{stati, ShutdownMessage};
}
use imports::*; //just so i can collapse it

#[derive(Debug, Clone)]
enum CommMessage {
    ConnectionFailed,
    ConnectionRefused,
    CTRLCExit,
}

struct CommChannels {
    sending: MPSCSender<CommMessage>,
    receiving: mpscwatcher::MPSCWatcherSubscription<CommMessage>,
}

fn main() -> Result<(), ()> {
    let args = get_args()?;

    println!("Connecting to: {:?} as {}", args.addr, args.name);
    let also_name = args.name.clone();

    let (comm_incoming_send, comm_incoming_recv) = std_channel::<CommMessage>();
    let (comm_outgoing_send, comm_outgoing_recv) = std_channel::<CommMessage>();
    let mut gui_comm_channels = CommChannels {
        sending: comm_outgoing_send,
        receiving: mpscwatcher::watch_dis(Uuid::new_v4().as_u128(), comm_incoming_recv),
    };

    let comm_res = thread::spawn(move || {
        let comm_ctx = Builder::new_current_thread().enable_all().build().unwrap();
        comm_ctx.block_on(comm_main(
            comm_incoming_send,
            comm_outgoing_recv,
            args.addr,
            args.name.clone(),
        ));
    });

    // let printer = thread::spawn(move || {
    //     loop {
    //         match comm_incoming_recv.try_recv() {
    //             Ok(msg) => {
    //                 println!("{:#?}", msg);
    //             }
    //             Err(err) => {
    //                 if err == TryRecvError::Disconnected {
    //                     // fun trick here, when the client closes, the sender is dropped, and it will automatically close
    //                     break;
    //                 }
    //             }
    //         }
    //     }
    // });
    FractureClientGUI::run(Settings::with_flags(FractureGUIFlags::new(
        gui_comm_channels,
    )))
    .unwrap();
    comm_res.join().unwrap();
    // printer.join().unwrap();
    Ok(())
}

#[derive(Clone, Debug)]
enum GUIMessage {
    SubmitMessage,
    TextInputChanged(String),
    ReceivedMessage(ChatMessage),
    ReceivedCommMessage(mpscwatcher::MPSCWatcherUpdate<CommMessage>)
}

impl From<mpscwatcher::MPSCWatcherUpdate<CommMessage>> for GUIMessage {
    fn from(item: mpscwatcher::MPSCWatcherUpdate<CommMessage>) -> GUIMessage {
        GUIMessage::ReceivedCommMessage (item)
    }
}

struct FractureGUIFlags {
    comm: CommChannels,
}

impl FractureGUIFlags {
    fn new(comm: CommChannels) -> Self {
        FractureGUIFlags { comm }
    }
}

struct FractureClientGUI {
    send_button: button::State,
    msg_input: text_input::State,
    scroll_state: scrollable::State,
    comm: CommChannels,
    messages: Vec<ChatMessage>,
    current_input: String,
}

impl Application for FractureClientGUI {
    type Executor = executor::Default;
    type Message = GUIMessage;
    type Flags = FractureGUIFlags;

    fn new(flags: FractureGUIFlags) -> (Self, Command<Self::Message>) {
        (
            Self {
                send_button: button::State::new(),
                msg_input: text_input::State::new(),
                scroll_state: scrollable::State::new(),
                comm: flags.comm,
                messages: vec![],
                current_input: String::new(),
            },
            Command::none(),
        )
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        self.comm.receiving
    }

    fn title(&self) -> String {
        "Fracture -- GUI testing".to_string()
    }

    fn update(
        &mut self,
        message: Self::Message,
        _clipboard: &mut Clipboard,
    ) -> Command<Self::Message> {
        match message {
            GUIMessage::SubmitMessage => {
                if self.current_input != "" {
                    println!("Sent msg: \"{}\"", self.current_input);
                    self.current_input = "".to_string();
                }
            }
            GUIMessage::ReceivedMessage(msg) => self.messages.push(msg),
            GUIMessage::TextInputChanged(new_content) => {
                self.current_input = new_content;
            }
            GUIMessage::ReceivedCommMessage(msg) => {
                todo!()
            }
        }
        Command::none()
    }

    fn view(&mut self) -> Element<Self::Message> {
        Column::new()
            .padding(2)
            .align_items(Align::Center)
            .push(
                self.messages.iter_mut().fold(
                    Scrollable::new(&mut self.scroll_state)
                        .padding(5)
                        .spacing(3)
                        .align_items(Align::Start)
                        .width(Length::Fill)
                        .height(Length::Fill),
                    |scroll, msg| scroll.push(msg.view()),
                ),
            )
            .push(Space::with_height(Length::from(5)))
            .push(
                Row::new()
                    .padding(10)
                    .align_items(Align::Center)
                    .push(
                        TextInput::new(&mut self.msg_input, "", &self.current_input, |content| {
                            GUIMessage::TextInputChanged(content)
                        })
                        .width(Length::Fill)
                        .on_submit(GUIMessage::SubmitMessage)
                        .padding(6),
                    )
                    .push(Space::with_width(Length::from(5)))
                    .push(
                        Button::new(&mut self.send_button, Text::new("Send"))
                            .on_press(GUIMessage::SubmitMessage),
                    ),
            )
            .into()
    }
}

#[derive(Clone, Debug)]
struct ChatMessage {
    msg_text: String,
    author_name: String,
    // authors_uuid: Uuid,
}

impl ChatMessage {
    fn view(&mut self) -> Element<GUIMessage> {
        Row::new()
            .align_items(Align::Start)
            .spacing(4)
            .padding(3)
            .push(Text::new(self.author_name.clone() + ": "))
            .push(Text::new(self.msg_text.clone()))
            .into()
    }
}

async fn comm_main(
    comm_send: MPSCSender<CommMessage>,
    _comm_rcvr: MPSCReceiver<CommMessage>,
    addr: std::net::SocketAddrV4,
    name: String,
) {
    let stream = match TcpStream::connect(addr).await {
        Ok(st) => st,
        Err(err) => {
            use std::io::ErrorKind::ConnectionRefused;

            let return_val = if err.kind() == ConnectionRefused {
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
        get_main_task(shutdown_tx, stream, name),
        get_ctrlc_listener(ctrlc_transmitter)
    );
    comm_send.send(CommMessage::CTRLCExit).unwrap();
    println!("Exited")
}

fn get_main_task(
    shutdown_tx: Sender<ShutdownMessage>,
    stream: TcpStream,
    name: String,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut close_rcv = shutdown_tx.subscribe();
        let mut client = Client::new(stream, name, get_default());
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
                            _ => {unreachable!()}//this should not happen
                        }
                    }
                }
                _ = close_rcv.recv() => {
                    println!("Disconnecting");
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
