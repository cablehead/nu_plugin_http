use std::str::FromStr;

use bytes::Bytes;

use http::response::Parts;

use tokio::runtime::{Builder, Runtime};
use tokio::sync::mpsc::Receiver;

use hyper::Error;
use hyper_util::rt::TokioIo;

use crate::bridge;

pub struct HTTPPlugin {
    pub runtime: Runtime,
}

impl HTTPPlugin {
    pub fn new() -> Self {
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create Tokio runtime");
        HTTPPlugin { runtime }
    }
}

impl HTTPPlugin {
    pub async fn request(
        &self,
        mut ctrlc_rx: tokio::sync::watch::Receiver<()>,
        method: String,
        url: String,
        body: bridge::Body,
    ) -> Result<(Parts, Receiver<Result<Bytes, Error>>), Box<dyn std::error::Error>> {
        // TODO: bring back TCP support (and TLS :/)

        let (path, url) = split_unix_socket_url(&url);
        // method, path, url
        eprintln!("REQUEST: {} {} {}", method, path, url);

        eprintln!("{}", std::env::current_dir().unwrap().display());

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
}

impl Default for HTTPPlugin {
    fn default() -> Self {
        Self::new()
    }
}

fn split_unix_socket_url(url: &str) -> (&str, &str) {
    let (path, url) = url.split_at(url.rfind("//").unwrap());
    let url = &url[1..];
    (path, url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_unix_socket_url() {
        let url = "./store/sock//?follow";
        let (path, url) = split_unix_socket_url(url);
        assert_eq!(path, "./store/sock");
        assert_eq!(url, "/?follow");
    }
}
