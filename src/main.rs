use nu_plugin::{serve_plugin, EvaluatedCall, JsonSerializer};
use nu_plugin::{EngineInterface, Plugin, PluginCommand};

use nu_protocol::{LabeledError, PipelineData, Signature, SyntaxShape, Type, Value};

use hyper_util::rt::TokioIo;

use tokio::runtime::{Builder, Runtime};

mod traits;

struct HTTPPlugin {
    runtime: Runtime,
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
    async fn process_url(&self, url: String) -> Result<(), Box<dyn std::error::Error>> {
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

impl Plugin for HTTPPlugin {
    fn commands(&self) -> Vec<Box<dyn PluginCommand<Plugin = Self>>> {
        vec![Box::new(HTTPGet)]
    }
}

struct HTTPGet;

impl PluginCommand for HTTPGet {
    type Plugin = HTTPPlugin;

    fn name(&self) -> &str {
        "httpx get"
    }

    fn usage(&self) -> &str {
        "Perform a HTTP get request"
    }

    fn signature(&self) -> Signature {
        Signature::build(PluginCommand::name(self))
            .required("url", SyntaxShape::String, "The url to GET")
            .optional(
                "closure",
                SyntaxShape::Closure(Some(vec![SyntaxShape::Record(vec![])])),
                "The closure to evaluate",
            )
            .input_output_type(Type::Nothing, Type::Any)
    }

    fn run(
        &self,
        plugin: &HTTPPlugin,
        engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let url = call.req::<String>(0)?;

        plugin.runtime.block_on(async move {
            let _ = plugin.process_url(url).await;
        });

        let url = call.req::<String>(0)?;
        let engine = engine.clone();

        let closure = call.opt(1)?;
        let span = call.head;

        let resp = reqwest::blocking::get(url)
            .map_err(|err| LabeledError::new(format!("reqwest error: {}", err.to_string())))?;

        let status = Value::int(resp.status().as_u16().into(), span);

        let mut r = nu_protocol::Record::new();
        r.insert("status", status);
        let r = Value::record(r, span);

        let body = traits::read_to_pipeline_data(resp, span);

        if let Some(closure) = closure {
            let res = engine
                .eval_closure_with_stream(&closure, vec![r], body, true, false)
                .map_err(|err| LabeledError::new(format!("shell error: {}", err)))?;

            return Ok(res);
        }

        Ok(body)
    }
}

fn main() {
    let plugin = HTTPPlugin::new();
    serve_plugin(&plugin, JsonSerializer)
}
