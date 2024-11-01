use serde::ser::{SerializeStruct, Serializer};
use serde::Serialize;

pub trait Command: Serialize {
    fn name() -> &'static str;

    fn skip_serializing_arguments(&self) -> bool;

    fn execute<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        if self.skip_serializing_arguments() {
            let mut state = serializer.serialize_struct("Command", 1)?;
            state.serialize_field("execute", Self::name())?;
            state.end()
        } else {
            let mut state = serializer.serialize_struct("Command", 2)?;
            state.serialize_field("execute", Self::name())?;
            state.serialize_field("arguments", self)?;
            state.end()
        }
    }
}
