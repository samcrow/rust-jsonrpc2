//!
//! Provides server endpoints
//!

pub mod stream;

use serde_json;
use transport::ServerTransport;
use transport::ServerCallback;
use super::RequestHandler;
use message::{Request, Response, Error, Value};

///
/// A server endpoint
///
pub struct ServerEndpoint<T> where T: ServerTransport {
    /// The transport layer
    /// Although this field is never accessed, it must be here so that it will be dropped
    /// when the endpoint is dropped.
    _transport: T,
}

impl<T> ServerEndpoint<T> where T: ServerTransport {
    pub fn new<H>(transport: T, handler: H) -> ServerEndpoint<T> where H: RequestHandler {
        let responder = Responder::new(handler);
        let mut transport = transport;
        transport.set_callback(responder);
        ServerEndpoint {
            _transport: transport,
        }
    }
}

/// Interfaces between the transport mechanism and the application logic
struct Responder<H> where H: RequestHandler {
    handler: H,
}

impl<H> Responder<H> where H: RequestHandler {
    pub fn new(handler: H) -> Responder<H> {
        Responder { handler: handler }
    }

    /// Takes a JSON value, interprets it as a request or notification, and returns
    /// an optional reply
    fn handle_json(&mut self, json: Value) -> Option<Value> {
        match Request::from_json(json) {
            Ok(request) => {
                let response = self.handle_request(request);
                response.map(|response|{ response.to_json() })
            },
            Err(rpc_error) => {
                // Send an error to the server
                let response = Response::new(Err(rpc_error));
                let json = response.to_json();
                Some(json)
            }
        }
    }

    fn handle_request(&mut self, request: Request) -> Option<Response> {
        match request.id {
            Some(_) => {
                let result = self.handler.handle_request(request);
                Some(Response::new(result))
            },
            None => {
                self.handler.handle_notification(request);
                None
            },
        }
    }
}

impl<H> ServerCallback for Responder<H> where H: RequestHandler {
    fn handle_request(&mut self, request: String) -> Option<String> {
        match serde_json::from_str(&request) {
            Ok(json) => {
                // Extract the ID from the request for later use
                let request_id: Option<Value> = match json {
                    Value::Object(ref map) => map.get(&"id".to_string()).map(|id| id.clone()),
                    _ => None,
                };
                let result = self.handle_json(json);
                match result {
                    Some(mut json) => {
                        // If the request JSON has an ID and the response has no ID or a null ID,
                        // assign the request's ID to the response
                        if let Value::Object(ref mut map) = json {
                            if let Some(id) = request_id {
                                map.insert("id".to_string(), id);
                            }
                        }
                        let response_text = serde_json::to_string(&json).ok();
                        response_text
                    },
                    None => None,
                }
            },
            Err(_) => {
                let err = Error::parse_error();
                let response = Response::new(Err(err));
                let response_text = serde_json::to_string(&response.to_json()).ok();
                response_text
            }
        }
    }
}
