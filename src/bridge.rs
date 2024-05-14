use std::pin::Pin;

use bytes::Bytes;

use http_body_util;
use http_body_util::BodyExt;

use tokio::sync::mpsc::Receiver;

use nu_protocol::{LabeledError, PipelineData, Value};

pub enum Body {
    Empty,
    Full(Vec<u8>),
    Stream(Receiver<Vec<u8>>),
}

impl Body {
    pub fn from_pipeline_data(input: PipelineData) -> Result<Body, LabeledError> {
        match input {
            PipelineData::Value(value, _) => match value {
                Value::String { val, .. } => Ok(Body::Full(val.as_bytes().to_vec())),
                Value::Nothing { .. } => Ok(Body::Empty),
                _ => Err(LabeledError::new(format!(
                    "Unsupported body type: {:?}",
                    &value
                ))),
            },
            PipelineData::ListStream(stream, _) => {
                panic!()
            }
            PipelineData::ExternalStream { stdout, .. } => {
                panic!()
            }
            PipelineData::Empty => Ok(Body::Empty),
        }
    }

    pub fn to_http_body(
        self,
    ) -> Pin<Box<dyn http_body::Body<Data = Bytes, Error = hyper::Error> + Send>> {
        match self {
            Body::Empty => {
                let body = http_body_util::Empty::<Bytes>::new().map_err(|never| match never {});
                Box::pin(body)
            }

            Body::Full(data) => {
                let body =
                    http_body_util::Full::new(Bytes::from(data)).map_err(|never| match never {});
                Box::pin(body)
            }
            _ => todo!(),
        }
    }
}
