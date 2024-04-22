use nu_plugin::{serve_plugin, EvaluatedCall, JsonSerializer};
use nu_plugin::{EngineInterface, Plugin, PluginCommand};

use nu_protocol::{LabeledError, PipelineData, Signature, SyntaxShape, Type, Value};

mod traits;

struct HTTPPlugin;

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
            .required(
                "closure",
                SyntaxShape::Closure(Some(vec![SyntaxShape::Record(vec![])])),
                "The closure to evaluate",
            )
            .input_output_type(Type::Nothing, Type::Any)
    }

    fn run(
        &self,
        _plugin: &HTTPPlugin,
        engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let engine = engine.clone();

        let url = call.req::<String>(0)?;
        let closure = call.req(1)?;
        let span = call.head;

        let resp = reqwest::blocking::get(url)
            .map_err(|err| LabeledError::new(format!("reqwest error: {}", err.to_string())))?;

        let status = Value::int(resp.status().as_u16().into(), span);

        let mut r = nu_protocol::Record::new();
        r.insert("status", status);
        let r = Value::record(r, span);

        let body = traits::read_to_pipeline_data(resp, span);
        let res = engine
            .eval_closure_with_stream(&closure, vec![r], body, true, false)
            .map_err(|err| LabeledError::new(format!("shell error: {}", err)))?;

        Ok(res)
    }
}

fn main() {
    serve_plugin(&HTTPPlugin, JsonSerializer)
}
