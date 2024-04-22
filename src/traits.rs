use std::io::Read;
use std::iter::Iterator;

use nu_protocol::{LabeledError, PipelineData, RawStream, ShellError, Span};

struct ReadIterator<R: Read + Send + 'static> {
    reader: R,
    buffer: Vec<u8>,
}

impl<R: Read + Send + 'static> ReadIterator<R> {
    pub fn new(reader: R, buf_size: usize) -> Self {
        ReadIterator {
            reader,
            buffer: vec![0; buf_size],
        }
    }
}

impl<R: Read + Send + 'static> Iterator for ReadIterator<R> {
    type Item = Result<Vec<u8>, ShellError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.reader.read(&mut self.buffer) {
            Ok(0) => None,
            Ok(bytes_read) => {
                let mut result = vec![0; bytes_read];
                result.copy_from_slice(&self.buffer[..bytes_read]);
                Some(Ok(result))
            }
            Err(err) => Some(Err(ShellError::LabeledError(Box::new(LabeledError::new(
                format!("Read error: {}", err),
            ))))),
        }
    }
}

pub fn read_to_pipeline_data<R: Read + Send + 'static>(reader: R, span: Span) -> PipelineData {
    let iter = Box::new(ReadIterator::new(reader, 4096));
    let stream = RawStream::new(iter, None, span.clone(), None);
    PipelineData::ExternalStream {
        stdout: Some(stream),
        stderr: None,
        exit_code: None,
        span: span,
        metadata: None,
        trim_end_newline: false,
    }
}
