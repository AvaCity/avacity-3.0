use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::{Mutex, Arc};

pub struct PlayerData {
    pub stream: Arc<Mutex<TcpStream>>,
    pub room: String,
    pub position: [f32; 2],
    pub dimention: i32,
    pub state: i32,
    pub action_tag: String
}

impl PlayerData {
    pub fn new(stream: Arc<Mutex<TcpStream>>, room: String, position: [f32; 2],
               dimention: i32, state: i32, action_tag: String) -> PlayerData {
        PlayerData {
            stream,
            room,
            position,
            dimention,
            state,
            action_tag
        }
    }
}

#[derive(Debug)]
pub enum Value {
    None,
    Boolean(bool),
    String(String),
    U8(u8),
    I32(i32),
    I64(i64),
    F32(f32),
    Vector(Vec<Value>),
    Object(HashMap<String, Value>)
}

impl Value {
    pub fn get_vector(&self) -> Result<&Vec<Value>, &'static str> {
        if let Value::Vector(v) = self {
            return Ok(v);
        } else {
            return Err("Can't convert");
        }
    }
    pub fn get_u8(&self) -> Result<u8, &'static str> {
        if let Value::U8(v) = self {
            return Ok(*v);
        } else {
            return Err("Can't convert");
        }
    }
    pub fn get_i32(&self) -> Result<i32, &'static str> {
        if let Value::I32(v) = self {
            return Ok(*v);
        } else {
            return Err("Can't convert");
        }
    }
    pub fn get_string(&self) -> Result<String, &'static str> {
        if let Value::String(v) = self {
            return Ok(v.clone());
        } else {
            return Err("Can't convert");
        }
    }
    pub fn get_object(&self) -> Result<&HashMap<String, Value>, &'static str> {
        if let Value::Object(v) = self {
            return Ok(v);
        } else {
            return Err("Can't convert");
        }
    }
}
