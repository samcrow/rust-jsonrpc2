//!
//! A JSON RPC 2.0 implementation
//!

pub mod client;
pub mod server;
pub mod transport;
pub mod message;
use message::*;

extern crate serde_json;
extern crate chrono;

use std::sync::mpsc::Sender;

///
/// Trait for a function object that can respond to a request and provide a response.
///
/// Implementations are provided for tuples of two closures.
///
pub trait RequestHandler : 'static + Send + Sync {
    /// Handles a JSON RPC request. Returns the result or an error.
    fn handle_request(&mut self, request: Request) -> Result<Value, Error>;
    /// Handles a JSON RPC notification
    fn handle_notification(&mut self, notification: Request);
}

/// Implementation of RequestHandler for a tuple of closures
impl<F, G> RequestHandler for (F, G) where F: Fn(Request) -> Result<Value, Error>,
    F: 'static + Send + Sync, G: Fn(Request), G: 'static + Send + Sync {

    fn handle_request(&mut self, request: Request) -> Result<Value, Error> {
        self.0(request)
    }
    fn handle_notification(&mut self, notification: Request) {
        self.1(notification)
    }
}

///
/// Trait for a function object that can receive a response
///
/// Implementations are provided for closures and for Sender<Response>.
///
pub trait ResponseHandler : 'static + Send {
    /// Handles a response that has been received
    fn handle_response(&mut self, response: Result<Value, Error>);
}

/// Implementation of ResponseHandler for closures
impl<F> ResponseHandler for F where F: Fn(Result<Value, Error>), F: 'static + Send {
    fn handle_response(&mut self, response: Result<Value, Error>) {
        self(response)
    }
}
/// Implementation of ResponseHandler for Sender
impl ResponseHandler for Sender<Result<Value, Error>> {
    fn handle_response(&mut self, response: Result<Value, Error>) {
        let _ = self.send(response);
    }
}
