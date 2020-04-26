use std::sync::{Mutex, Arc};
use std::collections::HashMap;
use std::net::TcpListener;
use std::thread;
use crate::client::Client;
use crate::common::PlayerData;
use crate::modules::{Base, house, avatar};

pub struct Server {
    pub modules: Arc<Mutex<HashMap<String, Box<dyn Base>>>>,
    pub player_data: Arc<Mutex<HashMap<String, PlayerData>>>
}

impl Server {
    pub fn listen(self) {
        let listener = TcpListener::bind("0.0.0.0:8123").unwrap();
        for stream in listener.incoming() {
            println!("new connection");
            let modules = Arc::clone(&self.modules);
            let player_data = Arc::clone(&self.player_data);
            thread::spawn(move || {
                let mut client = Client::new(stream.unwrap(), modules, player_data);
                client.handle();
            });
        }
    }

    pub fn new() -> Server {
        let player_data = Arc::new(Mutex::new(HashMap::new()));
        let modules: Arc<Mutex<HashMap<String, Box<dyn Base>>>> = Arc::new(Mutex::new(HashMap::new()));
        let mut lock = modules.lock().unwrap();
        let module = house::House::new();
        lock.insert(module.prefix.to_owned(), Box::new(module));
        let module = avatar::Avatar::new();
        lock.insert(module.prefix.to_owned(), Box::new(module));
        drop(lock);
        Server {
            modules,
            player_data
        }
    }
}
