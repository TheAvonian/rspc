use std::{error, fmt};

use rspc::{Config, Error, ErrorCode, ExecError, RequestKind, Router};
use serde_json::json;

pub enum MyCustomError {
    IAmBroke,
}

impl From<MyCustomError> for Error {
    fn from(_: MyCustomError) -> Self {
        Error::new(ErrorCode::InternalServerError, "I am broke".into())
    }
}

#[derive(Debug)]
pub enum CustomRustError {
    GenericError,
}

impl fmt::Display for CustomRustError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "some Rust error!")
    }
}

impl error::Error for CustomRustError {}

#[tokio::main]
async fn main() {
    let r =
        <Router>::new()
            .config(Config::new().export_ts_bindings("./bindings.ts"))
            .query("ok", |t| {
                t(|_, _args: ()| Ok("Hello World".into()) as Result<String, Error>)
            })
            .query("err", |t| {
                t(|_, _args: ()| {
                    Err(Error::new(
                        ErrorCode::BadRequest,
                        "This is a custom error!".into(),
                    )) as Result<String, _>
                })
            })
            .query("errWithCause", |t| {
                t(|_, _args: ()| {
                    Err(Error::with_cause(
                        ErrorCode::BadRequest,
                        "This is a custom error!".into(),
                        CustomRustError::GenericError,
                    )) as Result<String, Error>
                })
            })
            .query("customErr", |t| {
                t(|_, _args: ()| Ok(Err(MyCustomError::IAmBroke)?))
            })
            .query("customErrUsingInto", |t| {
                t(|_, _args: ()| Err(MyCustomError::IAmBroke.into()) as Result<String, Error>)
            })
            .query("asyncCustomError", |t| {
                t(
                    |_, _args: ()| async move {
                        Err(MyCustomError::IAmBroke.into()) as Result<String, _>
                    },
                )
            })
            .build();

    // You usually don't use this method directly. An integration will handle this for you. Check out the Axum and Tauri integrations to see how to use them!
    let v = r
        .execute((), RequestKind::Query, "ok".into(), None)
        .await
        .unwrap();
    println!("{:?}", v);
    assert_eq!(v, json!("Hello World"));

    let v = r.execute((), RequestKind::Query, "err".into(), None).await;
    println!("{:?}", v);
    assert!(matches!(v, Err(ExecError::ErrResolverError(_))));
}
