#![allow(warnings)]

use std::borrow::Borrow;
use std::error::Error;
use std::path::Path;

use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};

use nu_protocol::engine::Closure;
use nu_protocol::{
    LabeledError, PipelineData, Record, Signature, Span, Spanned, SyntaxShape, Type, Value,
};

// use crate::traits;
use crate::HTTPPlugin;

pub struct HTTPServe;

impl PluginCommand for HTTPServe {
    type Plugin = HTTPPlugin;

    fn name(&self) -> &str {
        "h. serve"
    }

    fn usage(&self) -> &str {
        "Service HTTP requests"
    }

    fn signature(&self) -> Signature {
        Signature::build(PluginCommand::name(self))
            // .required("url", SyntaxShape::String, "the url to service")
            .required(
                "closure",
                SyntaxShape::Closure(Some(vec![SyntaxShape::Record(vec![])])),
                "The closure to evaluate for each connection",
            )
            .input_output_type(Type::Any, Type::Any)
    }

    fn run(
        &self,
        plugin: &HTTPPlugin,
        engine: &EngineInterface,
        call: &EvaluatedCall,
        input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let (tx, mut rx) = watch::channel(false);

        match input {
            PipelineData::ExternalStream { exit_code, .. } => {
                let exit_code = exit_code.unwrap();
                std::thread::spawn(move || {
                    exit_code.stream.for_each(drop);
                    let _ = tx.send(true);
                    eprintln!("i'm outie");
                });
            }
            _ => return Err(LabeledError::new("ExternalStream expected")),
        }

        plugin.runtime.block_on(async move {
            let _ = serve(engine, call, rx).await;
        });

        let span = call.head;
        let value = Value::string("peace", span);
        let body = PipelineData::Value(value, None);
        return Ok(body);
    }
}

use std::convert::Infallible;
use std::net::SocketAddr;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

use tokio::sync::watch;

fn run_eval(
    engine: &EngineInterface,
    call: &EvaluatedCall,
    meta: Record,
) -> Result<(), Box<dyn std::error::Error>> {
    let closure = call.req(0)?;
    let span = call.head;

    let value = Value::string("hello", span);
    let body = PipelineData::Value(value, None);
    let res = engine
        .eval_closure_with_stream(&closure, vec![Value::record(meta, span)], body, true, false)
        .map_err(|err| LabeledError::new(format!("shell error: {}", err)))?;

    eprintln!("res: {:?}", &res);

    Ok(())
}

async fn hello(
    engine: &EngineInterface,
    call: &EvaluatedCall,
    req: Request<hyper::body::Incoming>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let span = call.head;
    let mut headers = Record::new();
    for (key, value) in req.headers() {
        headers.insert(
            key.to_string(),
            Value::string(value.to_str().unwrap().to_string(), span),
        );
        eprintln!("key: {:?} {:?}", &key, &value);
    }

    let mut meta = Record::new();
    meta.insert("headers", Value::record(headers, span));
    meta.insert("method", Value::string(req.method().to_string(), span));

    run_eval(engine, call, meta).unwrap();
    Ok(Response::new(Full::new(Bytes::from("Hello, World!"))))
}

async fn serve(
    engine: &EngineInterface,
    call: &EvaluatedCall,
    mut rx: watch::Receiver<bool>,
) -> Result<(), Box<dyn std::error::Error>> {
    let socket_path = Path::new("./").join("sock");
    let listener = tokio::net::UnixListener::bind(socket_path)?;

    use tokio::sync::watch;

    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, _)) => {
                        tokio::task::spawn(serve_connection(
                            engine.clone(),
                            call.clone(),
                            TokioIo::new(stream),
                        ));
                    },
                    Err(err) => {
                        eprintln!("Error accepting connection: {}", err);
                    },
                }
            },
            _ = rx.changed() => {
                // TODO: graceful shutdown of inflight connections
                break;
            },
        }
    }
    Ok(())
}

async fn serve_connection<T: std::marker::Unpin + tokio::io::AsyncWrite + tokio::io::AsyncRead>(
    engine: EngineInterface,
    call: EvaluatedCall,
    io: TokioIo<T>,
) {
    if let Err(err) = http1::Builder::new()
        .serve_connection(io, service_fn(|req| hello(&engine, &call, req)))
        .await
    {
        // Match against the error kind to selectively ignore `NotConnected` errors
        if let Some(std::io::ErrorKind::NotConnected) = err.source().and_then(|source| {
            source
                .downcast_ref::<std::io::Error>()
                .map(|io_err| io_err.kind())
        }) {
            // Silently ignore the NotConnected error
        } else {
            // Handle or log other errors
            println!("Error serving connection: {:?}", err);
        }
    }
}
