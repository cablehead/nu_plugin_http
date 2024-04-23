use hyper_util::rt::TokioIo;

use tokio::runtime::{Builder, Runtime};

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
    pub async fn process_url(&self, url: String) -> Result<(), Box<dyn std::error::Error>> {
        let url = url.parse::<hyper::Uri>().unwrap();
        if url.scheme_str() != Some("http") {
            eprintln!("This example only works with 'http' URLs.");
        }

        eprintln!("hello world: {:?}", &url);

        let host = url.host().expect("uri has no host");
        let port = url.port_u16().unwrap_or(80);
        let addr = format!("{}:{}", host, port);
        let stream = tokio::net::TcpStream::connect(addr).await?;
        let io = TokioIo::new(stream);

        use bytes::Bytes;
        use http_body_util::BodyExt;
        use http_body_util::Empty;
        use hyper::client::conn;
        use hyper::Request;

        let (mut request_sender, connection) = conn::http1::handshake(io).await.unwrap();

        // spawn a task to poll the connection and drive the HTTP state
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Error in connection: {}", e);
            }
        });

        let authority = url.authority().unwrap().clone();

        let path = url.path();
        let req = Request::builder()
            .uri(path)
            .header(hyper::header::HOST, authority.as_str())
            .body(Empty::<Bytes>::new())?;

        let mut res = request_sender.send_request(req).await?;

        eprintln!("Response: {}", res.status());
        eprintln!("Headers: {:#?}\n", res.headers());

        // Stream the body, writing each chunk to stdout as we get it
        // (instead of buffering and printing at the end).
        while let Some(next) = res.frame().await {
            let frame = next?;
            if let Some(chunk) = frame.data_ref() {
                eprintln!("chunk: {:?}", &chunk);
            }
        }

        eprintln!("\n\nDone!");

        Ok(())
    }
}
