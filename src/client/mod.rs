//!
//! Provdes a client endpoint
//!
pub mod stream;
use std::collections::HashMap;
use transport::{ClientTransport, PayloadHandler};
use transport::TransportError;
use message::*;
use serde_json;
use chrono::{Duration, Local};
use std::sync::{Arc, Mutex};
use std::thread;
use std::sync::mpsc::{channel, Sender, Receiver, TryRecvError};

///
/// Trait for things that can receive responses from the server
///
pub trait ResponseHandler: 'static + Send {
    /// Called with a response from the server
    fn response_received(&mut self, response: Response);
}

/// ResponseHandler implementation for closures
impl<F> ResponseHandler for F where F: Fn(Response), F: 'static + Send {
    fn response_received(&mut self, response: Response) {
        self(response)
    }
}

/// The type used to identify requests
type RequestID = u64;

///
/// A client endpoint, which can be used to send requests
///
pub struct ClientEndpoint {
    /// Channel used to send payloads to the transport thread
    send_channel: Sender<String>,
    /// A mapping from request IDs to response handlers
    handlers: Arc<Mutex<HashMap<RequestID, Box<ResponseHandler>>>>,
    /// The next ID to assign to a request
    next_id: RequestID,
}

impl ClientEndpoint {
    ///
    /// Creates a new ClientEndpoint
    ///
    /// transport: The transport layer to use
    ///
    pub fn new<T>(transport: T) -> ClientEndpoint where T: ClientTransport {
        let mut transport = transport;

        let handlers = Arc::new(Mutex::new(HashMap::new()));
        let payload_handler = StreamPayloadHandler::new(handlers.clone());

        transport.set_payload_handler(payload_handler);

        // Start a thread to write payloads
        let (tx, rx) = channel();
        let mut writer = StreamWriter::new(transport, rx);
        thread::Builder::new().name("ClientEndpoint writer".to_string()).spawn(move || {
            writer.run();
        }).unwrap();

        ClientEndpoint {
            send_channel: tx,
            handlers: handlers,
            next_id: 0,
        }
    }

    ///
    /// Sends a request
    ///
    /// If the request could not be sent, returns an error.
    ///
    /// The provided response handler will be called if a response is received.
    ///
    pub fn send_request<R>(&mut self, request: Request, response_handler: R) -> Result<(), TransportError> where R: ResponseHandler {
        // Get the ID to assign
        let id = self.next_id;
        self.next_id += 1;
        let mut request = request;
        request.set_id(Value::U64(id));
        try!(self.send(request));
        // Store the handler if the request was sent
        let mut handlers = self.handlers.lock().ok().expect("Handler mutex poisoned");
        assert!(!handlers.contains_key(&id));
        handlers.insert(id, Box::new(response_handler));
        Ok(())
    }

    ///
    /// Sends a request synchronously and returns the result
    ///
    pub fn send_request_sync(&mut self, request: Request, timeout: &Duration) -> Result<Response, TransportError> {
        let end = Local::now() + *timeout;
        let (tx, rx): (Sender<Response>, Receiver<Response>) = channel();
        let callback = move |response: Response| {
            match tx.send(response) {
                Ok(()) => {},
                Err(_) => println!("ClientEndpoint::send_request_sync: Client thread has hung up"),
            };
        };
        try!(self.send_request(request, callback));
        loop {
            match rx.try_recv() {
                Ok(response) => return Ok(response),
                Err(TryRecvError::Empty) => {}, // continue
                Err(TryRecvError::Disconnected) => return Err(TransportError::TimedOut),
            }
            if Local::now() > end {
                return Err(TransportError::TimedOut);
            }
        }
    }

    ///
    /// Sends a notification
    ///
    /// If the notification could not be sent, returns an error.
    ///
    pub fn send_notification(&mut self, notification: Request) -> Result<(), TransportError> {
        self.send(notification)
    }

    /// Sends a request
    fn send(&mut self, request: Request) -> Result<(), TransportError> {
        // Convert to JSON
        let json_text = match serde_json::to_string(&request.to_json()) {
            Ok(text) => text,
            Err(_) => return Err(TransportError::EncodeError),
        };
        // Send
        match self.send_channel.send(json_text) {
            Ok(()) => Ok(()),
            Err(_) => Err(TransportError::EndOfFile),
        }
    }

}

/// Handles payloads received from the transport layer
struct StreamPayloadHandler {
    /// Maps from response IDs to handlers
    response_handlers: Arc<Mutex<HashMap<RequestID, Box<ResponseHandler>>>>,
}

impl StreamPayloadHandler {
    pub fn new(handlers: Arc<Mutex<HashMap<RequestID, Box<ResponseHandler>>>>) -> StreamPayloadHandler {
        StreamPayloadHandler {
            response_handlers: handlers,
        }
    }

    fn handle_payload(&mut self, payload: &str) {
        match serde_json::from_str(payload) {
            Ok(json) => self.handle_json(json),
            Err(_) => println!("StreamPayloadHandler: Could not parse response"),
        }
    }

    fn handle_json(&mut self, json: Value) {
        match json {
            Value::Object(map) => match Response::from_json(map) {
                Ok(response) => self.handle_response(response),
                Err(_) => println!("StreamPayloadHandler: Response invalid"),
            },
            _ => println!("StreamPayloadHandler: Response invalid"),
        }
    }

    fn handle_response(&mut self, response: Response) {
        match response.id.clone() {
            Some(value) => match value.as_u64() {
                Some(id) => self.handle_response_with_id(response, id),
                None => println!("StreamPayloadHandler: Response has an ID that is not an integer"),
            },
            None => println!("StreamPayloadHandler: Response has no ID"),
        }
    }

    fn handle_response_with_id(&mut self, response: Response, id: u64) {
        let mut handlers = self.response_handlers.lock().unwrap();
        match handlers.remove(&id) {
            Some(mut handler) => handler.response_received(response),
            None => println!("StreamPayloadHandler: No handler for response"),
        }
    }
}

impl PayloadHandler for StreamPayloadHandler {
    fn payload_received(&mut self, result: Result<String, TransportError>) {
        match result {
            Ok(payload) => self.handle_payload(&payload),
            Err(e) => println!("Client transport receive error: {:?}", e),
        }
    }
}

///
/// Writes messages to a transport mechanism
///
/// For every string that is received over the channel, the received string is sent to the
/// transport mechanism.
///
struct StreamWriter<T> where T: ClientTransport {
    transport: T,
    channel: Receiver<String>,
}

impl<T> StreamWriter<T> where T: ClientTransport {
    pub fn new(transport: T, channel: Receiver<String>) -> StreamWriter<T> {
        StreamWriter {
            transport: transport,
            channel: channel,
        }
    }

    fn send_payload(&mut self, payload: &str) {
        match self.transport.send(payload) {
            Ok(()) => {},
            Err(e) => match e {
                TransportError::TimedOut
                | TransportError::Interrupted
                | TransportError::EncodeError => println!("StreamWriter: Failed to write: {:?}", e),
                _ => panic!("StreamWriter: Failed to write: {:?}", e),
            }
        }
    }

    pub fn run(&mut self) {
        loop {
            match self.channel.recv() {
                Ok(payload) => self.send_payload(&payload),
                Err(_) => {
                    println!("StreamWriter: Client has hung up; stopping");
                    return;
                },
            }
        }
    }
}
