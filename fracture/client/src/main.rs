mod args;
mod client;
mod handlers;
// mod mpscwatcher;
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
        button, executor, scrollable, text_input, time, Align, Application, Button, Clipboard,
        Column, Command, Element, Length, Row, Scrollable, Settings, Space, Subscription, Text,
        TextInput,
    };

    pub use uuid::Uuid;

    pub use fracture_core::msg;
    pub use fracture_core::stat;
    pub use fracture_core::utils::wait_update_time;

    pub use args::get_args;
    pub use client::Client;
    pub use handlers::get_default;
    pub use types::{stati, ChatMessage, ShutdownMessage};
}
use imports::*; //just so i can collapse it

const GUI_BUSYLOOP_SLEEP_TIME_MS: u64 = 300;

#[derive(Debug, Clone)]
enum CommMessage {
    SendChat(ChatMessage),   //send to server
    HandleChat(ChatMessage), //handle by gui
}

struct CommChannels {
    sending: MPSCSender<CommMessage>,
    receiving: MPSCReceiver<CommMessage>,
}

fn main() -> Result<(), ()> {
    let args = get_args()?;

    println!("Connecting to: {:?} as {}", args.addr, args.name);
    let name_clone = args.name.clone();

    let (comm_incoming_send, comm_incoming_recv) = std_channel::<CommMessage>();
    let (comm_outgoing_send, comm_outgoing_recv) = std_channel::<CommMessage>();
    let gui_comm_channels = CommChannels {
        sending: comm_outgoing_send,
        receiving: comm_incoming_recv,
    };

    let comm_res = thread::spawn(move || {
        let comm_ctx = Builder::new_current_thread().enable_all().build().unwrap();
        comm_ctx
            .block_on(comm_main(
                comm_incoming_send,
                comm_outgoing_recv,
                args.addr,
                args.name.clone(),
            ))
            .expect("Connected sucsessuflly to server");
    });

    FractureClientGUI::run(Settings::with_flags(FractureGUIFlags::new(
        gui_comm_channels,
        name_clone,
    )))
    .unwrap();
    comm_res.join().unwrap();
    Ok(())
}

#[derive(Clone, Debug)]
pub enum GUIMessage {
    SubmitMessage,
    TextInputChanged(String),
    Ticked,
}

struct FractureGUIFlags {
    comm: CommChannels,
    name: String,
}

impl FractureGUIFlags {
    fn new(comm: CommChannels, name: impl Into<String>) -> Self {
        FractureGUIFlags {
            comm,
            name: name.into(),
        }
    }
}

struct FractureClientGUI {
    send_button: button::State,
    msg_input: text_input::State,
    scroll_state: scrollable::State,
    comm: CommChannels,
    username: String,
    messages: Vec<ChatMessage>,
    current_input: String,
    exit: bool,
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
                username: flags.name,
                messages: vec![],
                current_input: String::new(),
                exit: false,
            },
            Command::none(),
        )
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        time::every(std::time::Duration::from_millis(GUI_BUSYLOOP_SLEEP_TIME_MS))
            .map(|_| GUIMessage::Ticked)
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
                    let chat_msg =
                        ChatMessage::new(self.current_input.clone(), self.username.clone());
                    self.comm
                        .sending
                        .send(CommMessage::SendChat(chat_msg.clone()))
                        .expect("Sent message to comm thread");
                    self.messages.push(chat_msg);
                    self.current_input = "".to_string();
                }
            }
            GUIMessage::TextInputChanged(new_content) => {
                self.current_input = new_content;
            }
            GUIMessage::Ticked => match self.comm.receiving.try_recv() {
                Ok(msg) => match msg {
                    CommMessage::HandleChat(chat_msg) => {
                        self.messages.push(chat_msg);
                    }
                    _ => panic!("GUI side received a message that it should not have!"),
                },
                Err(err) => match err {
                    TryRecvError::Disconnected => {
                        println!("Connection to server closed, closing GUI");
                        self.exit = true;
                    }
                    TryRecvError::Empty => {}
                },
            },
        }
        Command::none()
    }

    fn should_exit(&self) -> bool {
        self.exit.clone()
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

#[derive(Debug)]
enum CommMainError {
    ConnectionRefused(std::io::Error),
    GenericConnectionError(std::io::Error),
}

async fn comm_main(
    comm_send: MPSCSender<CommMessage>,
    comm_recv: MPSCReceiver<CommMessage>,
    addr: std::net::SocketAddrV4,
    name: String,
) -> Result<(), CommMainError> {
    let stream = match TcpStream::connect(addr).await {
        Ok(st) => st,
        Err(err) => {
            use std::io::ErrorKind::ConnectionRefused;

            let return_val = if err.kind() == ConnectionRefused {
                eprintln!("Connection Refused! The server may not be online, or there may be a problem with the network. Error folows:\n{:#?}", err);
                Err(CommMainError::ConnectionRefused(err))
            } else {
                eprintln!("Error while connecting:\n{:#?}", err);
                Err(CommMainError::GenericConnectionError(err))
            };
            eprintln!("Aborting!");
            return return_val;
        }
    };
    let (shutdown_tx, _): (Sender<ShutdownMessage>, Receiver<ShutdownMessage>) = channel(5);
    let ctrlc_transmitter = shutdown_tx.clone();

    let _task_results = join!(
        get_main_task(shutdown_tx, stream, name, comm_send, comm_recv),
        get_ctrlc_listener(ctrlc_transmitter)
    );
    println!("Exited");
    Ok(())
}

fn get_main_task(
    shutdown_tx: Sender<ShutdownMessage>,
    stream: TcpStream,
    name: String,
    comm_send: MPSCSender<CommMessage>,
    comm_recv: MPSCReceiver<CommMessage>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut close_rcv = shutdown_tx.subscribe();
        let mut client = Client::new(stream, name, get_default());
        loop {
            tokio::select! {
                stat = client.update_read() => {//TODO make it so that update read is not cancled, because with update loop that could cause corruption (just wait for readable, not actualy to read teh whole thing)
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
                    while let Ok(cmsg) = comm_recv.try_recv() {
                        match cmsg {
                            CommMessage::SendChat(msg) => {
                                client.manual_handler_operation(types::HandlerOperation::ServerMsg{msg: msg.into()})
                            }
                            other => {panic!("Client handler received unexpected CommMessage\n{:#?}", other)}
                        }
                    }
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
                            Ok(pos_msg) => {
                                use types::{HandlerOperation, InterfaceOperation};
                                if let Some(msg) = pos_msg {
                                    match msg {
                                        HandlerOperation::InterfaceOperation (oper) => {
                                            match oper {
                                                InterfaceOperation::ReceivedChat (msg) => {
                                                    //Ignore messages from self
                                                    if msg.author_uuid.expect("message has a uuid") == client.server_info.clone().expect("glient has a uuid").client_uuid {
                                                        println!("Ignoring {:?} because it was sent by this client", msg);
                                                    } else {
                                                        comm_send.send(CommMessage::HandleChat(msg)).expect("GUI received CommMessage");
                                                    }
                                                }
                                                #[allow(unreachable_patterns)]//not a problem
                                                unexpected => {panic!("unhandled InterfaceOperation:\n{:#?}", unexpected)}
                                            }
                                        }
                                        unexpected_msg => {panic!("client handler was asked to handle a operation it did not know about!\n{:#?}", unexpected_msg)}
                                    }
                                }
                            }
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
