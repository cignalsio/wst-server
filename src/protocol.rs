use serde::{Deserialize, Serialize};

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

#[derive(Deserialize, Serialize, Debug)]
pub enum LeapSecond {
    None = 0,
    Plus,
    Minus,
    NotAvailable
}
