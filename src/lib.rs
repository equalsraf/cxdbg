
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
use serde_json::Value;
extern crate serde;
use serde::de::{DeserializeOwned};
use serde::Serialize;

extern crate reqwest;
extern crate websocket;
use websocket::{Message, OwnedMessage, ClientBuilder, WebSocketError};
use websocket::client::sync::Client;
use websocket::stream::sync::TcpStream;

#[macro_use]
extern crate log;

use std::fmt::Debug;

#[derive(Debug)]
pub enum Error {
    WebSocket(WebSocketError),
    Json(serde_json::Error),
    CallError(Option<ErrorInfo>),
}

impl From<WebSocketError> for Error {
    fn from(e: WebSocketError) -> Self {
        Error::WebSocket(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Json(e)
    }
}

pub struct DebugClient {
    client: Client<TcpStream>,
    id: usize,
    pub pending_events: Vec<proto::Event>,
    pending_responses: Vec<Response<Value>>,
}

impl DebugClient {
    pub fn connect(port: u16) -> Self {
        // Get debug URL from json endpoint
        let r: Value = reqwest::get(&format!("http://localhost:{}/json", port))
            .expect("Unable to get /json")
            .json().expect("Invalid JSON data from /json");

        let attr = r.get(0)
            .expect("/json payload is empty")
            .get("webSocketDebuggerUrl")
            .expect(&format!("/json response is missing webSocketDebuggerUrl: {:?}", r));

        if let &Value::String(ref s) = attr {
            let client = ClientBuilder::new(s)
                .expect("Failed to open ws connection")
                .connect_insecure()
                .expect("Failed to open ws connection");

            DebugClient {
                id: 1,
                client,
                pending_events: Vec::new(),
                pending_responses: Vec::new(),
            }
        } else {
            panic!("webSocketDebuggerUrl is not a string");
        }

    }

    fn call<C: Serialize+Debug, R: DeserializeOwned>(&mut self, method: &str, params: C) -> Result<R, Error> {
        let reqid = self.id;
        self.id += 1;
        let r = Request {
            id: reqid,
            method,
            params: params,
        };
        let raw = serde_json::to_string(&r)?;
        debug!("--> {:#?}", raw);
        self.client.send_message(&Message::text(raw))?;

        loop {
            self.poll()?;

            if let Some(resp) = self.pending_responses.pop() {
                if resp.id == reqid {
                    if let Some(result) = resp.result {
                        let r = serde_json::from_value(result)?;
                        return Ok(r);
                    } else {
                        return Err(Error::CallError(resp.error));
                    }
                } else {
                    self.pending_responses.push(resp);
                }
            }
        }
    }

    pub fn poll(&mut self) -> Result<(), Error> {
        let frame = self.client.recv_message()?;

        let v: Value = match frame {
            OwnedMessage::Text(ref s) => serde_json::from_str(s)?,
            _ => panic!("Unexpected websocket msg type"),
        };
        debug!("<- {:#?}", v);

        if let Some(_) = v.get("id") {
            let r = serde_json::from_value(v)?;
            debug!("<- {:#?}", r);
            self.pending_responses.push(r);
            Ok(())
        } else {
            let m = serde_json::from_value(v)?;
            debug!("<- {:#?}", m);
            self.pending_events.push(m);
            Ok(())
        }
    }
}

#[allow(non_snake_case, non_camel_case_types)]
pub mod proto {
    include!(concat!(env!("OUT_DIR"), "/proto.rs"));
}

/// A request message sent by the client
#[derive(Serialize, Debug)]
struct Request<'s, A> {
    id: usize,
    method: &'s str,
    params: A,
}

/// A response to a request
#[derive(Deserialize, Debug)]
struct Response<R> {
    id: usize,
    result: Option<R>,
    error: Option<ErrorInfo>
}

#[derive(Deserialize, Debug)]
pub struct ErrorInfo {
    code: f64,
    message: String,
    data: Option<String>,
}

#[cfg(test)]
#[macro_use]
extern crate test_logger;

#[cfg(test)]
mod tests {
    use super::*;
    use proto::{PageApi, DOMApi, NetworkApi, InspectorApi};
    #[test]
    test!(it_works, {
        let mut c = DebugClient::connect(9222);

        PageApi::enable(&mut c).unwrap();
        DOMApi::enable(&mut c).unwrap();
        InspectorApi::enable(&mut c).unwrap();
        NetworkApi::enable(&mut c, None, None).unwrap();
        let doc = DOMApi::getDocument(&mut c, None, None).unwrap();
    });
}
