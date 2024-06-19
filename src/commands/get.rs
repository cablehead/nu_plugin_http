use std::path::Path;

use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{
    ByteStream, ByteStreamType, LabeledError, PipelineData, Record, ShellError, Signature,
    SyntaxShape, Type, Value,
};

use crate::bridge;
use crate::HTTPPlugin;

pub struct HTTPGet;

impl PluginCommand for HTTPGet {
    type Plugin = HTTPPlugin;

    fn name(&self) -> &str {
        "h. request"
    }

    fn usage(&self) -> &str {
        "Perform a HTTP get request"
    }

    fn signature(&self) -> Signature {
        Signature::build(PluginCommand::name(self))
            .required("method", SyntaxShape::String, "The request method")
            .required("url", SyntaxShape::String, "The url to GET")
            .optional(
                "closure",
                SyntaxShape::Closure(Some(vec![SyntaxShape::Record(vec![])])),
                "The closure to evaluate",
            )
            .input_output_type(Type::String, Type::Any)
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

        eprintln!("input: {:?}", &input);

        let body = bridge::Body::from_pipeline_data(input)?;

        let (ctrlc_tx, ctrlc_rx) = tokio::sync::watch::channel(());
        //
        // spawn an os thread to send on ctrlc_tx in 1 second
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let _ = ctrlc_tx.send(());
        });

        let (meta, mut rx) = plugin
            .runtime
            .block_on(async move { plugin.request(ctrlc_rx, method, url, body).await })
            .unwrap();

        eprintln!("meta: {:?}", &meta);

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
            None,
            ByteStreamType::Unknown,
            move |buffer: &mut Vec<u8>| match rx.blocking_recv() {
                Some(Ok(bytes)) => {
                    eprintln!("bytes: {:?}", &bytes);
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
