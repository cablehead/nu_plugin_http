use nu_protocol::{ListStream, PipelineData, Span, Value};
use std::io::{Read, Result};
use std::iter::Iterator;

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
    type Item = Result<Vec<u8>>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.reader.read(&mut self.buffer) {
            Ok(0) => None,
            Ok(bytes_read) => {
                let mut result = vec![0; bytes_read];
                result.copy_from_slice(&self.buffer[..bytes_read]);
                Some(Ok(result))
            }
            Err(e) => Some(Err(e)),
        }
    }
}

fn read_to_pipeline_data<R: Read + Send + 'static>(reader: R, span: Span) -> PipelineData {
    let read_iter = ReadIterator::new(reader, 4096);
    let boxed_iter: Box<dyn Iterator<Item = Result<Vec<u8>>> + Send> = Box::new(read_iter);
    let mapped_iter: Box<dyn Iterator<Item = Result<Value>> + Send> =
        Box::new(boxed_iter.map(move |result| {
            result.map(|val| Value::Binary {
                val,
                internal_span: span,
            })
        }));

    let list_stream = ListStream::from_stream(mapped_iter, None);
    PipelineData::ListStream(list_stream, None)
}
