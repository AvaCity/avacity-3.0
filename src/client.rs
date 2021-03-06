extern crate hex;
extern crate redis;
use std::collections::HashMap;
use std::error::Error;
use std::io::{Read, Write, Cursor};
use std::sync::{Mutex, RwLock, Arc};
use std::net::{TcpStream, Shutdown};
use bytes::{BytesMut, BufMut};
use crc::{crc32, Hasher32};
use redis::Commands;
use crate::decoder;
use crate::encoder;
use crate::base_messages;
use crate::common::{PlayerData, Value};
use crate::modules::{Base, location, send_to};

static XML: &'static str = "<?xml version=\"1.0\"?>
<cross-domain-policy>
<allow-access-from domain=\"*\" to-ports=\"*\" />
</cross-domain-policy>
";
static STRING_END: &'static [u8] = &[0];

pub struct Client {
    pub stream: Mutex<TcpStream>,
    pub uid: String,
    pub modules: Arc<RwLock<HashMap<String, Box<dyn Base>>>>,
    pub player_data: Arc<RwLock<HashMap<String, PlayerData>>>,
    pub redis: redis::Client,
    pub encrypted: bool,
    pub compressed: bool,
    pub checksummed: bool,
}

impl Client {
    pub fn start(&mut self) {
        match self.handle() {
            Ok(()) => {},
            Err(error) => println!("Critical client error: {}", error)
        }
        println!("drop connection");
        let lock = self.stream.lock().unwrap();
        lock.shutdown(Shutdown::Both).ok();
        location::leave_room(self).ok();
        let mut player_data = self.player_data.write().unwrap();
        if player_data.contains_key(&self.uid) {
            player_data.remove(&self.uid);
        }
    }
    fn handle(&mut self) -> Result<(), Box<dyn Error>> {
        let mut buffer = [0 as u8; 1024];
        loop {
            let mut read_lock = self.stream.lock().unwrap();
            let size = read_lock.read(&mut buffer)?;
            drop(read_lock);
            let hex_string = hex::encode(&buffer[..size]);
            if size == 0 {
                break
            }
            if hex_string == "3c706f6c6963792d66696c652d726571756573742f3e00" {
                let bytes = &[XML.as_bytes(), STRING_END].concat()[..];
                let mut lock = self.stream.lock().unwrap();
                lock.write(bytes)?;
                break
            }
            let data = &buffer[..size];
            let mut cur = Cursor::new(data);
            while data.len() as i32 - cur.position() as i32 > 4 {
                let mut tmp = [0; 4];
                cur.read_exact(&mut tmp).unwrap();
                let length = i32::from_be_bytes(tmp);
                if data.len() as i32 - (cur.position() as i32) < length {
                    break
                }
                let pos = cur.position() as usize;
                let tmp_data = &data[pos..pos+(length as usize)];
                cur.set_position(cur.position() + (length as u64));
                let message: HashMap<String, Value>;
                match decoder::decode(&tmp_data) {
                    Ok(value) => message = value,
                    Err(_) => break
                }
                let type_ = message.get("type").ok_or("err")?.get_u8()?;
                let msg = message.get("msg").ok_or("err")?.get_vector()?;
                println!("type - {}, msg - {:?}", type_, msg);
                if type_ == 1 && self.uid == "0".to_owned() {
                    match self.auth(msg) {
                        Ok(()) => {},
                        Err(error) => println!("Error: {:?}", error)
                    }
                }
                else if self.uid == "0".to_owned() {
                    return Ok(())
                }
                else if type_ == 2 {
                    return Ok(())
                }
                else if type_ == 34 {
                    let tmp = msg[1].get_string()?;
                    let splitted: Vec<&str> = tmp.split(".").collect();
                    let module_name = splitted[0].to_owned();
                    let lock = self.modules.read().unwrap();
                    if !lock.contains_key(&module_name) {
                        println!("Command {} not found", tmp);
                        continue
                    }
                    let module = lock.get(&module_name).unwrap();
                    match module.handle(self, msg) {
                        Ok(()) => {},
                        Err(error) => println!("Error: {}", error)
                    }
                }
            }
            buffer = [0 as u8; 1024];
        }
        Ok(())
    }

    pub fn send(&self, msg: &Vec<Value>, type_: u8) -> Result<(), Box<dyn Error>> {
        println!("send - {:?}", msg);
        let data = encoder::encode(msg, type_)?;
        let mut length = data.len() as i32 + 1;
        let mut mask = 0;
        let mut buf = BytesMut::new();
        let mut checksum: u32 = 0;
        if self.checksummed {
            mask = mask | (1 << 3);
            length = length + 4;
            let mut digest = crc32::Digest::new(crc32::IEEE);
            digest.write(&data[..]);
            checksum = digest.sum32();
        }
        buf.put_i32(length);
        buf.put_u8(mask);
        if self.checksummed {
            buf.put_u32(checksum);
        }
        buf.extend(&data[..]);
        let mut lock = self.stream.lock().unwrap();
        lock.write(&buf[..])?;
        Ok(())
    }

    fn auth(&mut self, msg: &Vec<Value>) -> Result<(), Box<dyn Error>> {
        let zone = msg[0].get_string()?;
        let uid = msg[1].get_string()?;
        if &zone == "account" {
            let mut v: Vec<Value> = Vec::new();
            v.push(Value::String(uid));
            v.push(Value::String("".to_owned()));
            v.push(Value::Boolean(true));
            v.push(Value::Boolean(false));
            v.push(Value::Boolean(false));
            self.send(&v, 1)?;
            return Ok(())
        }
        let token = msg[2].get_string()?;
        let auth_data = msg[3].get_object()?;
        let version = auth_data.get("v").ok_or("err")?.get_i32()?;
        let mut con = self.redis.get_connection()?;
        match con.get(format!("auth:{}", token)) {
            Ok(value) => {
                let real_uid: String = value;
                if uid != real_uid {
                    let msg = base_messages::wrong_pass();
                    self.send(&msg, 2)?;
                    let lock = self.stream.lock().unwrap();
                    lock.shutdown(Shutdown::Both).expect("Shutdown failed!");
                    return Ok(())
                }
                let mut player_data = self.player_data.write().unwrap();
                if player_data.contains_key(&real_uid) {
                    let player = player_data.get(&real_uid).unwrap();
                    send_to(&player.stream, &base_messages::kick_join(), 3).ok(); // kick old player
                    let lock = player.stream.lock().unwrap();
                    lock.shutdown(Shutdown::Both).ok();
                    self.send(&base_messages::already_joined(), 2).ok(); // say new player to reconnect
                    let lock = self.stream.lock().unwrap();
                    lock.shutdown(Shutdown::Both).ok();
                    return Ok(())
                }
                let lock = self.stream.lock().unwrap();
                player_data.insert(real_uid.clone(), PlayerData::new(Arc::new(Mutex::new(lock.try_clone()?)),
                                                                     String::new(), [0.0, 0.0], 4, 0, String::new()));
                drop(player_data);
                drop(lock);
                self.uid = real_uid.clone();
                let mut v: Vec<Value> = Vec::new();
                v.push(Value::String(real_uid));
                if version >= 3 {
                    v.push(Value::String("".to_owned()));
                }
                v.push(Value::Boolean(true));
                v.push(Value::Boolean(false));
                v.push(Value::Boolean(false));
                self.send(&v, 1)?;
                self.checksummed = true;
            }
            Err(_) => {
                let msg = base_messages::wrong_pass();
                self.send(&msg, 2)?;
                let lock = self.stream.lock().unwrap();
                lock.shutdown(Shutdown::Both)?;
                return Ok(())
            }
        }
        Ok(())
    }

    pub fn new(stream: TcpStream, modules: Arc<RwLock<HashMap<String, Box<dyn Base>>>>,
               player_data: Arc<RwLock<HashMap<String, PlayerData>>>) -> Client {
        Client {
            stream: Mutex::new(stream),
            uid: String::from("0"),
            modules: modules,
            player_data: player_data,
            redis: redis::Client::open("redis://127.0.0.1/").unwrap(),
            checksummed: false,
            compressed: false,
            encrypted: false
        }
    }
}
