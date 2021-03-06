use std::collections::{HashMap, HashSet};
use std::error::Error;
use redis::Commands;
use crate::client::Client;
use crate::common::Value;
use crate::inventory;
use crate::modules::{Base, get_plr, notify, location, campaign, send_to, passport::TROPHIES};

pub struct House {
    pub prefix: &'static str,
}

pub fn get_all_rooms(uid: &str, redis: &redis::Client) -> Result<Vec<Value>, Box<dyn Error>> {
    let mut con = redis.get_connection()?;
    let rooms: HashSet<String> = con.smembers(format!("rooms:{}", uid))?;
    let mut out = Vec::new();
    for room in rooms {
        out.push(Value::Object(get_room(uid, &room, redis)?));
    }
    Ok(out)
}

pub fn get_room(uid: &str, room: &str, redis: &redis::Client) -> Result<HashMap<String, Value>, Box<dyn Error>> {
    let mut con = redis.get_connection()?;
    let mut out_room = HashMap::new();
    let data: Vec<String> = con.lrange(format!("rooms:{}:{}", uid, &room), 0, -1)?;
    out_room.insert("w".to_owned(), Value::I32(13));
    out_room.insert("id".to_owned(), Value::String(room.to_owned()));
    out_room.insert("lev".to_owned(), Value::I32(data[1].parse::<i32>()?));
    out_room.insert("l".to_owned(), Value::I32(13));
    out_room.insert("nm".to_owned(), Value::String(data[0].clone()));
    let items: HashSet<String> = con.smembers(format!("rooms:{}:{}:items", uid, room))?;
    let mut room_items = Vec::new();
    for item in items {
        let data: Vec<String> = con.lrange(format!("rooms:{}:{}:items:{}", uid, room, &item), 0, -1)?;
        let mut out_item: HashMap<String, Value> = HashMap::new();
        let splitted: Vec<&str> = item.split("_").collect();
        out_item.insert("tpid".to_owned(), Value::String(splitted[0].to_owned()));
        out_item.insert("lid".to_owned(), Value::I32(splitted[1].parse::<i32>()?));
        out_item.insert("x".to_owned(), Value::F64(data[0].parse::<f64>()?));
        out_item.insert("y".to_owned(), Value::F64(data[1].parse::<f64>()?));
        out_item.insert("z".to_owned(), Value::F64(data[2].parse::<f64>()?));
        out_item.insert("d".to_owned(), Value::I32(data[3].parse::<i32>()?));
        room_items.push(Value::Object(out_item));
    }
    out_room.insert("f".to_owned(), Value::Vector(room_items));
    Ok(out_room)
}

impl House {
    pub fn new() -> House {
        House {
            prefix: "h"
        }
    }

    fn get_my_info(&self, client: &Client, msg: &Vec<Value>) -> Result<(), Box<dyn Error>> {
        let mut v: Vec<Value> = Vec::new();
        v.push(Value::String("h.minfo".to_owned()));
        let mut out_data: HashMap<String, Value> = HashMap::new();
        let data = msg[2].get_object()?;
        let onl = data.get("onl").ok_or("err")?.get_bool()?;
        if onl {
            out_data.insert("scs".to_owned(), Value::Boolean(true));
            v.push(Value::Object(out_data));
            client.send(&v, 34)?;
            return Ok(())
        }
        let player_data = client.player_data.read().unwrap();
        match get_plr(&client.uid, &player_data, &client.redis)? {
            Some(mut plr) => {
                let res = notify::get_res(&client.uid, &client.redis)?;
                plr.insert("res".to_owned(), Value::Object(res));
                let mut hs = HashMap::new();
                hs.insert("r".to_owned(), Value::Vector(get_all_rooms(&client.uid, &client.redis)?));
                hs.insert("lt".to_owned(), Value::I32(0));
                plr.insert("hs".to_owned(), Value::Object(hs));
                plr.insert("inv".to_owned(), Value::Object(inventory::get(&client.uid, &client.redis)?));
                plr.insert("cs".to_owned(), Value::Object(inventory::get_all_collections(&client.uid, &client.redis)?));
                let mut tr = HashMap::new();
                for trophy in TROPHIES.iter() {
                    let mut item = HashMap::new();
                    item.insert("trrt".to_owned(), Value::I32(0));
                    item.insert("trcd".to_owned(), Value::I32(0));
                    item.insert("trid".to_owned(), Value::String(trophy.to_owned()));
                    tr.insert(trophy.to_owned(), Value::Object(item));
                }
                let mut achc = HashMap::new();
                achc.insert("ac".to_owned(), Value::Object(HashMap::new()));
                achc.insert("tr".to_owned(), Value::Object(tr));
                plr.insert("achc".to_owned(), Value::Object(achc));
                out_data.insert("plr".to_owned(), Value::Object(plr));
                out_data.insert("tm".to_owned(), Value::I32(1));
            }
            None => {
                out_data.insert("has.avtr".to_owned(), Value::Boolean(false));
            }
        }
        v.push(Value::Object(out_data));
        client.send(&v, 34)?;
        campaign::new(client)?;
        Ok(())
    }

    fn get_owner_info(&self, client: &Client, msg: &Vec<Value>) -> Result<(), Box<dyn Error>> {
        let data = msg[2].get_object()?;
        let uid = data.get("uid").ok_or("err")?.get_string()?;
        let player_data = client.player_data.read().unwrap();
        let plr = get_plr(&uid, &player_data, &client.redis)?.ok_or("err")?;
        let mut hs = HashMap::new();
        hs.insert("r".to_owned(), Value::Vector(get_all_rooms(&uid, &client.redis)?));
        hs.insert("lt".to_owned(), Value::I32(0));
        let mut out_data = HashMap::new();
        out_data.insert("ath".to_owned(), Value::Boolean(false));
        out_data.insert("plr".to_owned(), Value::Object(plr));
        out_data.insert("hs".to_owned(), Value::Object(hs));
        let mut v = Vec::new();
        v.push(Value::String("h.oinfo".to_owned()));
        v.push(Value::Object(out_data));
        client.send(&v, 34)?;
        Ok(())
    }

    fn init_owner_info(&self, client: &Client, msg: &Vec<Value>) -> Result<(), Box<dyn Error>> {
        let data = msg[2].get_object()?;
        let uid = data.get("uid").ok_or("err")?.get_string()?;
        let player_data = client.player_data.read().unwrap();
        let plr = get_plr(&uid, &player_data, &client.redis)?.ok_or("err")?;
        let mut out_data = HashMap::new();
        out_data.insert("ath".to_owned(), Value::Boolean(false));
        out_data.insert("plr".to_owned(), Value::Object(plr));
        out_data.insert("tids".to_owned(), Value::Vector(Vec::new()));
        let mut v = Vec::new();
        v.push(Value::String("h.ioinfo".to_owned()));
        v.push(Value::Object(out_data));
        client.send(&v, 34)?;
        Ok(())
    }

    fn get_room(&self, client: &Client, msg: &Vec<Value>) -> Result<(), Box<dyn Error>> {
        let data = msg[2].get_object()?;
        let lid = data.get("lid").ok_or("key not found")?.get_string()?;
        let gid = data.get("gid").ok_or("key not found")?.get_string()?;
        let rid = data.get("rid").ok_or("key not found")?.get_string()?;
        let room = format!("{}_{}_{}", lid, gid, rid);
        location::leave_room(client)?;
        location::join_room(client, &room)?;
        let mut out_data = HashMap::new();
        out_data.insert("rid".to_owned(), Value::String(room));
        let mut v = Vec::new();
        v.push(Value::String("h.gr".to_owned()));
        v.push(Value::Object(out_data));
        client.send(&v, 34)?;
        Ok(())
    }

    fn room(&self, client: &Client, msg: &Vec<Value>) -> Result<(), Box<dyn Error>> {
        let tmp = msg[1].get_string()?;
        let splitted: Vec<&str> = tmp.split(".").collect();
        let command = splitted[2];
        match command {
            "info" => self.room_info(client, msg)?,
            "rfr" => self.room_refresh(client, msg)?,
            _ => location::room(client, msg)?
        }
        Ok(())
    }

    fn room_info(&self, client: &Client, msg: &Vec<Value>) -> Result<(), Box<dyn Error>> {
        let data = msg[2].get_object()?;
        let uid = data.get("uid").ok_or("err")?.get_string()?;
        let rid = data.get("rid").ok_or("err")?.get_string()?;
        let room = get_room(&uid, &rid, &client.redis)?;
        let room_name = format!("house_{}_{}", &uid, &rid);
        let mut rmmb = Vec::new();
        let player_data = client.player_data.read().unwrap();
        for player_uid in player_data.keys() {
            let player = player_data.get(&player_uid.clone()).ok_or("player not found")?;
            if player.room == room_name {
                match get_plr(&player_uid, &player_data, &client.redis)? {
                    Some(plr) => rmmb.push(Value::Object(plr)),
                    None => continue
                }
            }
        }
        let mut out_data = HashMap::new();
        out_data.insert("rm".to_owned(), Value::Object(room));
        out_data.insert("rmmb".to_owned(), Value::Vector(rmmb));
        out_data.insert("evn".to_owned(), Value::None);
        let mut v = Vec::new();
        v.push(Value::String("h.r.info".to_owned()));
        v.push(Value::Object(out_data));
        client.send(&v, 34)?;
        Ok(())
    }

    fn room_refresh(&self, client: &Client, msg: &Vec<Value>) -> Result<(), Box<dyn Error>> {
        let room_name = msg[0].get_string()?;
        let splitted: Vec<&str> = room_name.split("_").collect();
        let room = get_room(&client.uid, &splitted[2], &client.redis)?;
        let mut out_data = HashMap::new();
        out_data.insert("rm".to_owned(), Value::Object(room));
        let mut v = Vec::new();
        v.push(Value::String("h.r.rfr".to_owned()));
        v.push(Value::Object(out_data));
        let player_data = client.player_data.read().unwrap();
        for player_uid in player_data.keys() {
            let player = player_data.get(&player_uid.clone()).ok_or("player not found")?;
            if player.room == room_name {
                send_to(&player.stream, &v, 34)?;
            }
        }
        Ok(())
    }
}

impl Base for House {
    fn handle(&self, client: &Client, msg: &Vec<Value>) -> Result<(), Box<dyn Error>> {
        let tmp = msg[1].get_string()?;
        let splitted: Vec<&str> = tmp.split(".").collect();
        let command = splitted[1];
        match command {
            "minfo" => self.get_my_info(client, msg)?,
            "oinfo" => self.get_owner_info(client, msg)?,
            "ioinfo" => self.init_owner_info(client, msg)?,
            "gr" => self.get_room(client, msg)?,
            "r" => self.room(client, msg)?,
            _ => println!("Command {} not found", tmp)
        }
        Ok(())
    }
}
