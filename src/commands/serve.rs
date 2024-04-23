use nu_plugin::EvaluatedCall;
use nu_plugin::{EngineInterface, PluginCommand};

use nu_protocol::{LabeledError, PipelineData, Signature, SyntaxShape, Type, Value};

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
            .input_output_type(Type::Nothing, Type::Any)
    }

    fn run(
        &self,
        _plugin: &HTTPPlugin,
        engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let closure = call.req(0)?;
        let span = call.head;

        let value = Value::string("hello", span);
        let body = PipelineData::Value(value, None);

        let res = engine
            .eval_closure_with_stream(&closure, vec![], body, true, false)
            .map_err(|err| LabeledError::new(format!("shell error: {}", err)))?;

        return Ok(res);
    }
}
