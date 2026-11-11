use crate::{Error, QmpError, Result};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::any::TypeId;

pub(crate) fn request(name: &str, args: Value, oob: bool) -> Result<Value> {
    if name.is_empty() || !args.is_object() {
        return Err(Error::InvalidRequest(
            "command name must be nonempty and arguments must be an object".into(),
        ));
    }
    let verb = if oob { "exec-oob" } else { "execute" };
    let mut value = json!({verb: name});
    if !args.as_object().unwrap().is_empty() {
        value["arguments"] = args;
    }
    Ok(value)
}

pub(crate) fn validate_request(value: &Value, oob: bool) -> Result<()> {
    let verb = if oob { "exec-oob" } else { "execute" };
    let object = value
        .as_object()
        .ok_or_else(|| Error::InvalidRequest("QMP request must be an object".into()))?;
    if !object
        .get(verb)
        .and_then(Value::as_str)
        .is_some_and(|s| !s.is_empty())
        || object.keys().any(|k| k != verb && k != "arguments")
        || object.get("arguments").is_some_and(|v| !v.is_object())
    {
        return Err(Error::InvalidRequest(format!(
            "expected {verb} and optional object arguments; IDs are owned by the connection"
        )));
    }
    Ok(())
}

/// Missing return is never success, and null is a real return value. An error
/// envelope must preserve both string fields, including unknown class names.
pub(crate) fn response(mut value: Value) -> Result<Value> {
    let object = value
        .as_object_mut()
        .ok_or_else(|| Error::Protocol("response must be an object".into()))?;
    if object.contains_key("event")
        || object.contains_key("QMP")
        || object.contains_key("return") == object.contains_key("error")
    {
        return Err(Error::Protocol(
            "response must contain exactly one of return or error".into(),
        ));
    }
    if let Some(error) = object.remove("error") {
        let error: QmpError = serde_json::from_value(error)
            .map_err(|e| Error::Protocol(format!("invalid error response: {e}")))?;
        return Err(Error::Qmp(error));
    }
    Ok(object.remove("return").unwrap())
}

pub(crate) fn decode<T: DeserializeOwned + 'static>(value: Value) -> Result<T> {
    if TypeId::of::<T>() == TypeId::of::<()>() && value.as_object().is_some_and(|o| o.is_empty()) {
        return Ok(serde_json::from_value(Value::Null)?);
    }
    // Do not turn failed {} decoding into null: Option<T> would silently become None.
    Ok(serde_json::from_value(value)?)
}
