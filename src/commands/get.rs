use std::path::Path;

use nu_plugin::{EngineInterface, EvaluatedCall, PluginCommand};
use nu_protocol::{
    LabeledError, PipelineData, RawStream, Record, ShellError, Signature, SyntaxShape, Type, Value,
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
        let url = Path::new(&cwd).join(Path::new(&url)).to_string_lossy().into_owned();

        eprintln!("input: {:?}", &input);

        let body = bridge::Body::from_pipeline_data(input)?;

        let (meta, mut rx) = plugin
            .runtime
            .block_on(async move { plugin.request(method, url, body).await })
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

        let iter = std::iter::from_fn(move || {
            Some(
                rx.blocking_recv()?
                    .map_err(|err| {
                        ShellError::LabeledError(Box::new(LabeledError::new(format!(
                            "Read error: {}",
                            err
                        ))))
                    })
                    .map(|bytes| bytes.to_vec()),
            )
        });

        let stream = RawStream::new(
            Box::new(iter) as Box<dyn Iterator<Item = Result<Vec<u8>, ShellError>> + Send>,
            None,
            span.clone(),
            None,
        );

        let body = PipelineData::ExternalStream {
            stdout: Some(stream),
            stderr: None,
            exit_code: None,
            span,
            metadata: None,
            trim_end_newline: false,
        };

        if let Some(closure) = closure {
            let res = engine
                .eval_closure_with_stream(&closure, vec![r], body, true, false)
                .map_err(|err| LabeledError::new(format!("shell error: {}", err)))?;

            return Ok(res);
        }

        Ok(body)
    }
}
