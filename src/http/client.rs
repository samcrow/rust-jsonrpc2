//!
//! HTTP client transport implementation
//!

use transport::{ClientTransport, TransportError, PayloadHandler};
use hyper;
use hyper::client::Client;
use hyper::client::IntoUrl;
use hyper::Url;
use hyper::status::StatusCode;
use hyper::header::ContentType;
use hyper::mime::{Mime, TopLevel, SubLevel};
use std::thread;
use std::sync::{Arc, Mutex};
use std::io::Read;

///
/// An HTTP-based client transport implementation
pub struct HTTPClientTransport {
    /// The URL of the server endpoint
    url: Url,
    /// The payload handler
    payload_handler: Option<Arc<Mutex<Box<PayloadHandler>>>>,
}

impl HTTPClientTransport {
    pub fn new<U>(url: U) -> Result<HTTPClientTransport, ()> where U: IntoUrl {
        match url.into_url() {
            Ok(url) => Ok(HTTPClientTransport {
                url: url,
                payload_handler: None,
            }),
            Err(_) => Err(()),
        }
    }
}

impl ClientTransport for HTTPClientTransport {
    fn set_payload_handler<H>(&mut self, handler: H) where H: PayloadHandler {
        self.payload_handler = Some(Arc::new(Mutex::new(Box::new(handler))));
    }

    fn send(&mut self, payload: &str) -> Result<(), TransportError> {
        match self.payload_handler {
            Some(ref handler) => {
                let requestor = Requestor::new(self.url.clone(), String::from(payload), handler.clone());
                let _ = thread::spawn(move || {
                    requestor.run();
                });

                Ok(())
            },
            None => Err(TransportError::MissingCallback),
        }
    }
}

/// Sends an HTTP request and processes the response
struct Requestor {
    /// The URL of the endpoint
    url: Url,
    /// The payload to send
    payload: String,
    /// The handler to
    handler: Arc<Mutex<Box<PayloadHandler>>>,
}

impl Requestor {
    /// Creates a new Requestor
    pub fn new(url: Url, payload: String, handler: Arc<Mutex<Box<PayloadHandler>>>) -> Requestor {
        Requestor {
            url: url,
            payload: payload,
            handler: handler,
        }
    }

    /// Sends the request and processes the response
    pub fn run(self) {
        let client = Client::new();
        let result = client.post(self.url)
            .header(ContentType(Mime(TopLevel::Application, SubLevel::Json, vec![])))
            .body(&self.payload)
            .send();

        match result {
            Ok(mut response) => {
                match response.status {
                    StatusCode::Ok => {

                    },
                    _ => {

                    },
                };

                let mut response_string = String::new();
                match response.read_to_string(&mut response_string) {
                    Ok(_) => Self::call_handler(self.handler, Ok(response_string)),
                    Err(e) => Self::call_handler(self.handler, Err(TransportError::from(e))),
                }
            },
            Err(http_err) => {
                let err = match http_err {
                    hyper::error::Error::Uri(_) => TransportError::NotFound,
                    hyper::error::Error::Io(io_err) => TransportError::from(io_err),
                    _ => TransportError::Other,
                };
                Self::call_handler(self.handler, Err(err));
            }
        };
    }

    fn call_handler(handler: Arc<Mutex<Box<PayloadHandler>>>, result: Result<String, TransportError>) {
        let mut handler = handler.lock().expect("Payload handler mutex poisoned");
        handler.payload_received(result);
    }
}
