pub mod style;
pub mod types;
use types::*;

use std::sync::mpsc::TryRecvError;

use iced::{
    button, executor, scrollable, text_input, time, Align, Application, Button, Clipboard, Column,
    Command, Container, Element, Length, Row, Scrollable, Space, Subscription, Text, TextInput,
};

use crate::types::{ChatMessage, ChatViewable, CommChannels, CommMessage};

use crate::conf::GUI_BUSYLOOP_SLEEP_TIME_MS;

pub struct FractureClientGUI {
    send_button: button::State,
    close_button: button::State,
    msg_input: text_input::State,
    scroll_state: scrollable::State,
    comm: CommChannels,
    username: String,
    chat_elems: Vec<Box<dyn ChatViewable<GUIMessage>>>,
    current_input: String,
    exit: bool,
    server_name: Option<String>,
}

impl Application for FractureClientGUI {
    type Executor = executor::Default;
    type Message = GUIMessage;
    type Flags = FractureGUIFlags;

    fn new(flags: FractureGUIFlags) -> (Self, Command<Self::Message>) {
        (
            Self {
                send_button: button::State::new(),
                close_button: button::State::new(),
                msg_input: text_input::State::new(),
                scroll_state: scrollable::State::new(),
                comm: flags.comm,
                username: flags.name,
                chat_elems: vec![],
                current_input: String::new(),
                exit: false,
                server_name: None,
            },
            Command::none(),
        )
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        time::every(std::time::Duration::from_millis(GUI_BUSYLOOP_SLEEP_TIME_MS))
            .map(|_| GUIMessage::Ticked)
    }

    fn title(&self) -> String {
        format!("{} - Fracture v{}", self.username, clap::crate_version!())
    }

    fn update(
        &mut self,
        message: Self::Message,
        _clipboard: &mut Clipboard,
    ) -> Command<Self::Message> {
        match message {
            GUIMessage::SubmitMessage => {
                if !self.current_input.is_empty() {
                    println!("Sent msg: \"{}\"", self.current_input);
                    let chat_msg =
                        ChatMessage::new(self.current_input.clone(), self.username.clone());
                    self.comm
                        .sending
                        .send(CommMessage::SendChat(chat_msg.clone()))
                        .expect("Sent message to comm thread");
                    self.chat_elems.push(Box::new(chat_msg));
                    self.current_input = "".to_string();
                }
            }
            GUIMessage::TextInputChanged(new_content) => {
                self.current_input = new_content;
            }
            GUIMessage::Ticked => match self.comm.receiving.try_recv() {
                Ok(msg) => match msg {
                    CommMessage::HandleChat(chat_msg) => {
                        self.chat_elems.push(Box::new(chat_msg));
                    }
                    CommMessage::RawMessage(raw_msg) => {
                        self.chat_elems.push(Box::new(raw_msg));
                    }
                    CommMessage::ServerInfo { server_name } => {
                        self.server_name = Some(server_name);
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
            GUIMessage::Close => {
                self.exit = true;
            }
        }
        Command::none()
    }

    fn should_exit(&self) -> bool {
        self.exit
    }

    fn view(&mut self) -> Element<Self::Message> {
        get_main_ui(self)
    }
}

fn get_main_ui(
    this: &mut FractureClientGUI,
) -> Element<<FractureClientGUI as Application>::Message> {
    Column::new()
        .padding(0)
        .align_items(Align::Center)
        .push(
            Container::new(
                Row::new()
                    .padding(0)
                    .spacing(0)
                    .align_items(Align::Center)
                    .width(Length::Fill)
                    .push(
                        Button::new(&mut this.close_button, Text::new("close"))
                            .on_press(GUIMessage::Close)
                            .height(Length::Shrink)
                            .style(style::menubar::CloseButton),
                    )
                    .push(Space::with_width(Length::Units(5)))
                    .push(
                        Text::new(format!("{}", this.server_name.as_ref().unwrap_or(&"Unidentified Server".to_string())))
                        .vertical_alignment(iced::VerticalAlignment::Center)
                    )
                    .push(Space::with_width(Length::Fill))
                    .push(
                        Text::new(format!("{}", this.username))
                        .vertical_alignment(iced::VerticalAlignment::Center)
                    )
                    .push(Space::with_width(Length::Units(5))),
            )
            .width(Length::Fill)
            .style(style::menubar::MenuBar),
        )
        .push(
            Container::new(Space::with_height(Length::from(4)))
                .width(Length::Fill)
                .style(style::menubar::Spacer),
        )
        .push(
            this.chat_elems.iter_mut().fold(
                Scrollable::new(&mut this.scroll_state)
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
                    TextInput::new(&mut this.msg_input, "", &this.current_input, |content| {
                        GUIMessage::TextInputChanged(content)
                    })
                    .width(Length::Fill)
                    .on_submit(GUIMessage::SubmitMessage)
                    .padding(6),
                )
                .push(Space::with_width(Length::from(5)))
                .push(
                    Button::new(&mut this.send_button, Text::new("Send"))
                        .on_press(GUIMessage::SubmitMessage),
                ),
        )
        .into()
}
