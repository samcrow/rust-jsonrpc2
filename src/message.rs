use serde_json::builder::ObjectBuilder;
use std::collections::BTreeMap;

pub use serde_json::Value;

///
/// The parameters of a request
///
#[derive(Debug,PartialEq)]
pub enum Params {
    /// Named parameters. Parameters are stored in a map.
    Named(BTreeMap<String, Value>),
    /// Positional parameters
    Positional(Vec<Value>),
}

impl Params {
    pub fn to_json(&self) -> Value {
        match *self {
            Params::Named(ref map) => Value::Object(map.clone()),
            Params::Positional(ref vec) => Value::Array(vec.clone()),
        }
    }

    pub fn from_json(json: Value) -> Result<Params, Error> {
        match json {
            Value::Object(map) => Ok(Params::Named(map)),
            Value::Array(vec) => Ok(Params::Positional(vec)),
            _ => Err(Error::invalid_request()),
        }
    }
}

///
/// A JSON RPC request or notification
///
/// A request has an ID, a notification does not.
///
#[derive(Debug)]
pub struct Request {
    pub method: String,
    pub params: Option<Params>,
    pub id: Option<Value>,
}

impl Request {
    /// Creates a new request
    pub fn new(method: &str, params: Option<Params>) -> Request {
        Request {
            method: method.to_string(),
            params: params,
            id: None,
        }
    }

    /// Sets the ID of this request
    pub fn set_id(&mut self, id: Value) {
        self.id = Some(id);
    }

    pub fn to_json(&self) -> Value {
        let mut builder = ObjectBuilder::new()
            .insert("jsonrpc", "2.0")
            .insert("method", self.method.clone());
        if let Some(ref params_value) = self.params {
            builder = builder.insert("params", params_value.to_json());
        }
        if let Some(ref id_value) = self.id {
            builder = builder.insert("id", id_value.clone());
        }
        builder.unwrap()
    }

    pub fn from_json(json: Value) -> Result<Request, Error> {
        let err = Error::invalid_request();
        match json {
            Value::Object(map) => {
                let method = try!(try!(map.get("method").ok_or(err.clone())).as_string().ok_or(err.clone()));
                let id = match map.get("id") {
                    None => None,
                    Some(id) => Some(id.clone()),
                };
                let params = match map.get("params") {
                    Some(params_json) => Some(try!(Params::from_json(params_json.clone()))),
                    None => None,
                };
                Ok(Request {
                    method: method.to_string(),
                    params: params,
                    id: id,
                })
            },
            _ => Err(err.clone()),
        }
    }
}

///
/// A JSON RPC response
///
#[derive(Debug)]
pub struct Response {
    /// The payload (result or error) of this response
    pub payload: Result<Value, Error>,
    /// The ID of this response
    pub id: Option<Value>,
}

impl Response {
    pub fn new(payload: Result<Value, Error>) -> Response {
        Response {
            payload: payload,
            id: None,
        }
    }

    /// Sets the ID of this response
    pub fn set_id(&mut self, id: Value) {
        self.id = Some(id);
    }

    pub fn to_json(&self) -> Value {
        let mut builder = ObjectBuilder::new()
            .insert("jsonrpc", "2.0");
        match self.id {
            Some(ref id) => builder = builder.insert("id", id.clone()),
            None => builder = builder.insert("id", Value::Null),
        }
        match self.payload {
            Ok(ref result) => {
                builder = builder.insert("result", result.clone());
            },
            Err(ref error) => {
                builder = builder.insert("error", error.to_json());
            }
        }
        builder.unwrap()
    }
    pub fn from_json(map: BTreeMap<String, Value>) -> Result<Response, Error> {
        let err = Error::invalid_request();
        let id = try!(map.get("id").ok_or(err.clone())).clone();
        let has_result = map.contains_key("result");
        let has_error = map.contains_key("error");
        let payload: Result<Value, Error> = try!(match (has_result, has_error) {
            (true, false) => Ok(Ok(map.get("result").unwrap().clone())),
            (false, true) => Ok(Err(try!(Error::from_json(map.get("error").unwrap().clone())))),
            _ => Err(err.clone()),
        });
        Ok(Response {
            payload: payload,
            id: Some(id),
        })
    }
}

///
/// A JSON RPC error
///
#[derive(Debug, Clone)]
pub struct Error {
    /// Error code
    code: i64,
    /// Error message
    message: String,
    /// Optional additional error data
    data: Option<Value>,
}

impl Error {
    pub fn new(code: i64, message: &str, data: Option<Value>) -> Error {
        Error {
            code: code,
            message: message.to_string(),
            data: data,
        }
    }

    pub fn to_json(&self) -> Value {
        let mut builder = ObjectBuilder::new()
            .insert("code", self.code)
            .insert("message", self.message.clone());
        if let Some(ref data_value) = self.data {
            builder = builder.insert("data", data_value.clone());
        }
        builder.unwrap()
    }

    pub fn from_json(json: Value) -> Result<Error, Error> {
        let err = Error::invalid_request();
        let map = try!(json.as_object().ok_or(err.clone()));
        let code = try!(try!(map.get("id").ok_or(err.clone())).as_i64().ok_or(err.clone()));
        let message = try!(try!(map.get("message").ok_or(err.clone())).as_string().ok_or(err.clone()));
        let data = match map.get("data") {
            Some(data) => Some(data.clone()),
            None => None,
        };
        Ok(Error {
            code: code,
            message: message.to_string(),
            data: data,
        })
    }

    /// Returns a standard error that indicates that the requested method was not found
    pub fn method_not_found() -> Error {
        Error {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
        }
    }
    /// Return a standard error that indicates a parsing failure
    pub fn parse_error() -> Error {
        Error {
            code: -32700,
            message: "Parse error".to_string(),
            data: None,
        }
    }
    /// Return a standard error that indicates an invalid request was sent
    pub fn invalid_request() -> Error {
        Error {
            code: -32600,
            message: "Invalid request".to_string(),
            data: None,
        }
    }
    /// Return a standard error that indicates invalid parameters
    pub fn invalid_params() -> Error {
        Error {
            code: -32602,
            message: "Invalid params".to_string(),
            data: None,
        }
    }
    /// Return a standard error that indicates an internal error
    pub fn internal_error() -> Error {
        Error {
            code: -32603,
            message: "Internal error".to_string(),
            data: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;
    use serde_json::Value;
    use std::collections::BTreeMap;

    #[test]
    fn params_named() {
        let json_text = "{\"key_1\":\"value_1\", \"key_2\": 39}";
        let json = serde_json::from_str(json_text).unwrap();
        let params = Params::from_json(json).unwrap();

        let mut expected_map: BTreeMap<String, Value> = BTreeMap::new();
        expected_map.insert("key_1".to_string(), Value::String("value_1".to_string()));
        expected_map.insert("key_2".to_string(), Value::U64(39));
        let expected_params = Params::Named(expected_map);

        assert_eq!(params, expected_params);
    }
    #[test]
    fn params_positional() {
        let json_text = "[1, 2, 3, \"Pie\", -3.14]";
        let json = serde_json::from_str(json_text).unwrap();
        let params = Params::from_json(json).unwrap();

        let expected_vec: Vec<Value> = vec![Value::U64(1), Value::U64(2), Value::U64(3),
            Value::String("Pie".to_string()), Value::F64(-3.14)];
        let expected_params = Params::Positional(expected_vec);

        assert_eq!(params, expected_params);
    }
    #[test]
    fn params_number() {
        let json_text = "2465";
        let json = serde_json::from_str(json_text).unwrap();
        let params = Params::from_json(json);
        assert!(params.is_err());
    }
    #[test]
    fn params_string() {
        let json_text = "\"Lasagna is delicious\"";
        let json = serde_json::from_str(json_text).unwrap();
        let params = Params::from_json(json);
        assert!(params.is_err());
    }
}
