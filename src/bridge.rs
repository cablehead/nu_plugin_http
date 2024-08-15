use std::io::Read;
use std::pin::Pin;

use bytes::Bytes;

use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

use http_body_util::BodyExt;
use http_body_util::StreamBody;
use hyper::body::Frame;

use nu_protocol::{LabeledError, PipelineData, Value};

pub enum Body {
    Empty,
    Full(Vec<u8>),
    Stream(tokio::sync::mpsc::Receiver<Vec<u8>>),
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
            PipelineData::ListStream(_, _) => {
                panic!("ListStream not supported")
            }

            PipelineData::ByteStream(stream, _) => {
                if let Some(mut reader) = stream.reader() {
                    let (tx, rx) = tokio::sync::mpsc::channel(100); // Adjust buffer size as needed
                    std::thread::spawn(move || {
                        let mut buffer = [0; 4096]; // Adjust buffer size as needed
                        loop {
                            match reader.read(&mut buffer) {
                                Ok(0) => break, // End of stream
                                Ok(n) => {
                                    if tx.blocking_send(buffer[..n].to_vec()).is_err() {
                                        break; // Channel closed, stop sending
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Error reading from ByteStream: {:?}", e);
                                    break;
                                }
                            }
                        }
                    });

                    Ok(Body::Stream(rx))
                } else {
                    Ok(Body::Empty) // Stream is empty
                }
            }

            PipelineData::Empty => Ok(Body::Empty),
        }
    }

    pub fn into_http_body(
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

            Body::Stream(rx) => {
                let stream = ReceiverStream::new(rx);
                let stream = stream.map(|data| Ok(Frame::data(bytes::Bytes::from(data))));
                let body = StreamBody::new(stream).boxed();
                Box::pin(body)
            }
        }
    }
}
