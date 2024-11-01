use anyhow::Result;
//use qapi::versions::v1_0_0::commands::{Cont, Memsave};
use qapi::traits::Command;
use qapi::commands::QueryStatus as QueryStatusTrait;
use qapi::versions::v1_1_0::commands::QueryStatus;

fn main() -> Result<()>{
    let q = QueryStatus::new()?;
    dbg![&q];
    let c = serde_json::to_string(&q).unwrap();
    dbg![c];

    //let q = QMP::memsave::<qapi::versions::v1_0_0::commands::Memsave>()?
    //    .val(123)?
    //    .size(124)?
    //    .filename("test".into())?;
    //dbg![&q];
    //let c = serde_json::to_string(&q).unwrap();
    //dbg![c];

    Ok(())
}
