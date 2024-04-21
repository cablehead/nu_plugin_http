use nu_plugin::{serve_plugin, EvaluatedCall, JsonSerializer};
use nu_plugin::{EngineInterface, Plugin, PluginCommand};

use nu_protocol::{LabeledError, PipelineData, Signature, SyntaxShape, Type, Value};

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
        "http get"
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

        let url: Value = call.req(0)?;
        let closure = call.req(1)?;
        let span = call.head;

        let res = engine
            .eval_closure(&closure, vec![url], None)
            .unwrap_or_else(|err| Value::error(err, span));
        Ok(PipelineData::Value(res, None))
    }
}

fn main() {
    serve_plugin(&HTTPPlugin, JsonSerializer)
}
