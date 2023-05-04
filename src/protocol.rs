use serde::{Deserialize, Serialize};
use serde_repr::*;

#[derive(Deserialize, Serialize, Debug)]
pub struct Request {
    c: Option<u128>
}

impl Request {
    pub fn process(&mut self, now: u128) -> Response {
        Response {
            c: self.c,
            s: now,
            e: None, // TODO
            l: Some(LeapSecond::NotAvailable) // TODO
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Response {
    c: Option<u128>,
    s: u128,
    e: Option<u8>,
    l: Option<LeapSecond>
}

#[derive(Serialize_repr, Deserialize_repr, PartialEq, Debug)]
#[repr(u8)]
pub enum LeapSecond {
    None = 0,
    Plus = 1,
    Minus = 2,
    NotAvailable = 3
}
