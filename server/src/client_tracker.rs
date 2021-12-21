use queues::{ Queue, IsQueue };
use uuid::Uuid;

#[derive(Clone)]
enum UserStateUpdate {
    Disconnected {
        id: Uuid,
        name: String,
    },
    Connected {
        id: Uuid,
        name: String,
    }
}

struct TrackerSubscriber {
    updates: Queue<UserStateUpdate>,
    users: Vec<User>,
}

impl TrackerSubscriber {
    fn new() -> TrackerSubscriber {
        TrackerSubscriber {
            updates: Queue::new(),
            users: vec![],
        }
    }

    fn get_by_id(&mut self, id: Uuid) -> Option<&User> {
        for u in self.users.iter() {
            if u.id == id {
                return Some(&u);
            }
        }
        return None;
    }

    /// Start tracking a new client
    fn user_connected(&mut self, client: User) {
        self.updates.add(UserStateUpdate::Connected { id: client.id_cpy(), name: client.name_cpy() }).unwrap();
        self.users.push(client);
    }

    /// This will panic if the client is not connected
    fn user_disconnect(&mut self, id: Uuid) {
        let name = self.get_by_id(id).unwrap().name_cpy();
        self.updates.add(UserStateUpdate::Disconnected { id: id, name: name }).unwrap();
        let index = self.users.iter().position(|x| x.id == id).unwrap();
        self.users.remove(index);
    }
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

/// Tracks all connected clients, and provides a way of subscribing to it
struct ClientTracker {}
