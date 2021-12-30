/// a iced::Subscription to watch for incoming messages on a std::mpsc
use iced_futures::futures;
use iced_native::subscription::Recipe;
use std::hash::{Hash, Hasher};
use std::sync::mpsc::Receiver;
use futures::stream::BoxStream;

pub type MPSCWatcherSubscription<T> = iced::Subscription<MPSCWatcherUpdate<T>>;

pub fn watch<H: 'static + Hash + Copy + Send, MessageType: 'static + Sync + Send + Clone>
(
    id: H,
    channel: Receiver<MessageType>
) -> MPSCWatcherSubscription<MessageType> {
    iced::Subscription::from_recipe(MPSCWatcher {
        id,
        channel
    })
}

pub struct MPSCWatcher<Message, IdentifierType>
where
    IdentifierType: Hash + Send,
{
    id: IdentifierType,
    channel: Receiver<Message>,
}

impl<HasherType, EventType, MessageType, IdentifierType>
    Recipe<HasherType, EventType>
    for MPSCWatcher<MessageType, IdentifierType>
where
    HasherType: Hasher,
    IdentifierType: 'static + Hash + Send,
    MessageType: 'static + Send + Clone + std::marker::Sync,
{
    type Output = MPSCWatcherUpdate<MessageType>;

    fn hash(&self, hasher: &mut HasherType) {
        self.id.hash(hasher);
    }

    fn stream(
        self: Box<Self>,
        _input: BoxStream<EventType>,
    ) -> BoxStream<Self::Output> {
        Box::pin(
            futures::stream::unfold(
                MPSCWatcherState::Ready(self.channel),
                move |state: MPSCWatcherState<MessageType> | async move {
                    match state {
                        MPSCWatcherState::Ready(ch) => {
                            match ch.try_recv() {
                                Ok(val) => {
                                    Some((
                                        MPSCWatcherUpdate::Received(val),
                                        MPSCWatcherState::Ready(ch),
                                    ))
                                }
                                Err(err) => {
                                    use std::sync::mpsc::TryRecvError;
                                    match err {
                                        TryRecvError::Empty => {
                                            Some((
                                                MPSCWatcherUpdate::Nothing,
                                                MPSCWatcherState::Ready(ch)
                                            ))
                                        }
                                        TryRecvError::Disconnected => {
                                            Some((
                                                MPSCWatcherUpdate::Closed,
                                                MPSCWatcherState::Done,
                                            ))
                                        }
                                    }
                                }
                            }
                        }
                        MPSCWatcherState::Done => {
                            let _: () = iced::futures::future::pending().await;
                            None
                        }
                    }
                }
            )
        )
    }
}

#[derive(Debug, Clone)]
pub enum MPSCWatcherUpdate<Msg> {
    Nothing,
    Received(Msg),
    Closed,
}

pub enum MPSCWatcherState<Message> {
    Ready(Receiver<Message>),
    Done,
}

// // Just a little utility function
// pub fn file<I: 'static + hash::Hash + Copy + Send, T: ToString>(
//     id: I,
//     url: T,
// ) -> iced::Subscription<(I, Progress)> {
//     iced::Subscription::from_recipe(Download {
//         id,
//         url: url.to_string(),
//     })
// }

// pub struct Download<I> {
//     id: I,
//     url: String,
// }

// // Make sure iced can use our download stream
// impl<H, I, T> iced_native::subscription::Recipe<H, I> for Download<T>
// where
//     T: 'static + hash::Hash + Copy + Send,
//     H: hash::Hasher,
// {
//     type Output = (T, Progress);

//     fn hash(&self, state: &mut H) {
//         struct Marker;
//         std::any::TypeId::of::<Marker>().hash(state);

//         self.id.hash(state);
//     }

//     fn stream(
//         self: Box<Self>,
//         _input: futures::stream::BoxStream<'static, I>,
//     ) -> futures::stream::BoxStream<'static, Self::Output> {
//         let id = self.id;

//         Box::pin(futures::stream::unfold(
//             State::Ready(self.url),
//             move |state| async move {
//                 match state {
//                     State::Ready(url) => {
//                         let response = reqwest::get(&url).await;

//                         match response {
//                             Ok(response) => {
//                                 if let Some(total) = response.content_length() {
//                                     Some((
//                                         (id, Progress::Started),
//                                         State::Downloading {
//                                             response,
//                                             total,
//                                             downloaded: 0,
//                                         },
//                                     ))
//                                 } else {
//                                     Some((
//                                         (id, Progress::Errored),
//                                         State::Finished,
//                                     ))
//                                 }
//                             }
//                             Err(_) => {
//                                 Some(((id, Progress::Errored), State::Finished))
//                             }
//                         }
//                     }
//                     State::Downloading {
//                         mut response,
//                         total,
//                         downloaded,
//                     } => match response.chunk().await {
//                         Ok(Some(chunk)) => {
//                             let downloaded = downloaded + chunk.len() as u64;

//                             let percentage =
//                                 (downloaded as f32 / total as f32) * 100.0;

//                             Some((
//                                 (id, Progress::Advanced(percentage)),
//                                 State::Downloading {
//                                     response,
//                                     total,
//                                     downloaded,
//                                 },
//                             ))
//                         }
//                         Ok(None) => {
//                             Some(((id, Progress::Finished), State::Finished))
//                         }
//                         Err(_) => {
//                             Some(((id, Progress::Errored), State::Finished))
//                         }
//                     },
//                     State::Finished => {
//                         // We do not let the stream die, as it would start a
//                         // new download repeatedly if the user is not careful
//                         // in case of errors.
//                         let _: () = iced::futures::future::pending().await;

//                         None
//                     }
//                 }
//             },
//         ))
//     }
// }

// #[derive(Debug, Clone)]
// pub enum Progress {
//     Started,
//     Advanced(f32),
//     Finished,
//     Errored,
// }

// pub enum State {
//     Ready(String),
//     Downloading {
//         response: reqwest::Response,
//         total: u64,
//         downloaded: u64,
//     },
//     Finished,
// }
