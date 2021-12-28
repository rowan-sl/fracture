/// User updates should not be used to maintain a accurate record of what users are connected by a client interface,
/// and should instead be used (for example) to signal chat messages and similar things
/// TODO make this work?

use std::sync::Arc;
use parking_lot::FairMutex;

use uuid::Uuid;
use tokio::sync::broadcast::*;

#[derive(Clone)]
enum UserUpdate {
    Connected ( User ),
    Disconnected ( User ),
}

#[derive(Clone)]
struct User {
    id: uuid::Uuid,
    name: String,
}

impl User {
    fn new_unique(name: String) -> User {
        User {
            id: uuid::Uuid::new_v4(),
            name: name,
        }
    }

    fn new(name: String, id: uuid::Uuid) -> User {
        User {
            id: id,
            name: name,
        }
    }

    fn id(&self) -> &uuid::Uuid {
        return &self.id;
    }

    fn id_cpy(&self) -> Uuid {
        return self.id.clone();
    }

    fn name(&self) -> &String {
        return &self.name;
    }

    fn name_cpy(&self) -> String {
        return self.name.clone();
    }
}

pub struct Subscription {
    users: Arc<FairMutex<Vec<User>>>,
    update_channel: Receiver<UserUpdate>,
}

/// Tracks all connected clients, and provides a way of subscribing to it
struct ClientTracker {
    core_update_tx: Sender<UserUpdate>,
    users: Arc<FairMutex<Vec<User>>>
}

impl ClientTracker {
    fn new(update_channel_limit: usize) -> ClientTracker {
        let (update_tx, _): (Sender<UserUpdate>, Receiver<UserUpdate>) = channel(update_channel_limit);
        ClientTracker {
            core_update_tx: update_tx,
            users: Arc::new(FairMutex::new(Vec::new()))//let the chain b e g i n
        }
    }

    fn user_connects(&mut self, user: User) {
        let _ = self.core_update_tx.send(UserUpdate::Connected(user.clone()));//we dont care about the result bc it could only tell us that no one is listening

    }
}
