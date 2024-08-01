use std::path::Path;

use std::str::FromStr;

use bytes::Bytes;

use http::response::Parts;

use tokio::sync::mpsc::Receiver;

use hyper::Error;
use hyper_util::rt::TokioIo;

use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{
    ByteStream, ByteStreamType, HandlerGuard, LabeledError, PipelineData, Record, ShellError,
    Signature, SyntaxShape, Type, Value,
};

use crate::bridge;
use crate::HTTPPlugin;

pub struct HTTPRequest;

impl PluginCommand for HTTPRequest {
    type Plugin = HTTPPlugin;

    fn name(&self) -> &str {
        "h. request"
    }

    fn usage(&self) -> &str {
        "Perform a HTTP client request"
    }

    fn signature(&self) -> Signature {
        Signature::build(PluginCommand::name(self))
            .required("method", SyntaxShape::String, "The request method")
            .required("uri", SyntaxShape::String, "The request uri")
            .optional(
                "closure",
                SyntaxShape::Closure(Some(vec![SyntaxShape::Record(vec![])])),
                "The closure to evaluate",
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
        let method = call.req::<String>(0)?;

        let url = call.req::<String>(1)?;
        let cwd = engine.get_current_dir()?;
        let url = Path::new(&cwd)
            .join(Path::new(&url))
            .to_string_lossy()
            .into_owned();

        let body = bridge::Body::from_pipeline_data(input)?;

        let (ctrlc_tx, ctrlc_rx) = tokio::sync::watch::channel(false);

        let _guard = engine.register_signal_handler(Box::new(move |_| {
            let _ = ctrlc_tx.send(true);
        }))?;

        let (meta, mut rx) = plugin
            .runtime
            .block_on(async move { request(ctrlc_rx, _guard, method, url, body).await })
            .unwrap();

        let closure = call.opt(2)?;
        let span = call.head;

        let mut headers = Record::new();
        for (key, value) in meta.headers.iter() {
            headers.insert(
                key.to_string(),
                Value::string(value.to_str().unwrap().to_string(), span),
            );
        }

        let status = Value::int(meta.status.as_u16().into(), span);

        let mut r = Record::new();
        r.insert("headers", Value::record(headers, span));
        r.insert("status", status);
        let r = Value::record(r, span);

        let stream = ByteStream::from_fn(
            span,
            engine.signals().clone(),
            ByteStreamType::Unknown,
            move |buffer: &mut Vec<u8>| match rx.blocking_recv() {
                Some(Ok(bytes)) => {
                    buffer.extend_from_slice(&bytes);
                    Ok(true)
                }
                Some(Err(err)) => Err(ShellError::LabeledError(Box::new(LabeledError::new(
                    format!("Read error: {}", err),
                )))),
                None => Ok(false),
            },
        );

        if let Some(closure) = closure {
            let res = engine
                .eval_closure_with_stream(&closure, vec![r], stream.into(), true, false)
                .map_err(|err| LabeledError::new(format!("shell error: {}", err)))?;

            return Ok(res);
        }

        Ok(stream.into())
    }
}

async fn request(
    mut ctrlc_rx: tokio::sync::watch::Receiver<bool>,
    _guard: HandlerGuard,
    method: String,
    url: String,
    body: bridge::Body,
) -> Result<(Parts, Receiver<Result<Bytes, Error>>), Box<dyn std::error::Error>> {
    // TODO: bring back TCP support (and TLS :/)
    let (path, url) = split_unix_socket_url(&url);
    let path = Path::new(path).canonicalize().unwrap();
    let stream = tokio::net::UnixStream::connect(path)
        .await
        .expect("Failed to connect to server");
    let io = TokioIo::new(stream);

    use http_body_util::BodyExt;
    use hyper::client::conn;
    use hyper::Request;

    let (mut request_sender, connection) = conn::http1::handshake(io).await.unwrap();

    // spawn a task to poll the connection and drive the HTTP state
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Error in connection: {}", e);
        }
    });

    let method = http::method::Method::from_str(&method.to_uppercase())?;
    let body = body.into_http_body();
    let req = Request::builder().method(method).uri(url).body(body)?;

    let res = request_sender.send_request(req).await?;
    let (meta, mut body) = res.into_parts();

    let (tx, rx) = tokio::sync::mpsc::channel(32);

    tokio::spawn(async move {
        let _guard = _guard;
        loop {
            tokio::select! {
                _ = ctrlc_rx.changed() => {
                    // The close signal has been received, break the loop
                    break;
                }
                frame = body.frame() => {
                    match frame {
                        Some(Ok(frame)) => {
                            if let Some(chunk) = frame.data_ref() {
                                if tx.send(Ok(chunk.clone())).await.is_err() {
                                    break;
                                }
                            }
                        }
                        Some(Err(e)) => {
                            if tx.send(Err(e)).await.is_err() {
                                break;
                            }
                        }
                        None => break, // No more frames
                    }
                }
            }
        }
    });

    Ok((meta, rx))
}

fn split_unix_socket_url(url: &str) -> (&str, &str) {
    if let Some(pos) = url.rfind("//") {
        let (path, url) = url.split_at(pos);
        let url = &url[1..]; // Skip the first '/'
        (path, url)
    } else {
        (url, "/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_url() {
        let url = "./store/sock//?follow";
        let (path, url) = split_unix_socket_url(url);
        assert_eq!(path, "./store/sock");
        assert_eq!(url, "/?follow");
    }

    #[test]
    fn test_no_url() {
        let url = "./store/sock";
        let (path, url) = split_unix_socket_url(url);
        assert_eq!(path, "./store/sock");
        assert_eq!(url, "/");
    }

    #[test]
    fn test_trailing_slash() {
        let url = "./store/sock//";
        let (path, url) = split_unix_socket_url(url);
        assert_eq!(path, "./store/sock");
        assert_eq!(url, "/");
    }
}
