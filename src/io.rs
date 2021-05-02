//! I/O utilities.
use std::io;
use std::io::prelude::*;

/// Converts LF to CRLF in the inner stream.
pub struct ConvertLFtoCRLF<R> {
    inner: R,
}

impl<R> ConvertLFtoCRLF<R> {
    pub fn new(inner: R) -> Self {
        Self { inner }
    }
    fn convert_lf_to_crlf(data: &[u8], out: &mut [u8]) -> usize {
        let mut out_idx = 0;
        for char in data {
            if char == &b'\n' {
                out[out_idx] = b'\r';
                out[out_idx + 1] = b'\n';
                out_idx += 2;
            } else {
                out[out_idx] = *char;
                out_idx += 1;
            }
        }
        out_idx
    }
}

impl<R: Read> Read for ConvertLFtoCRLF<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Buffer for inner stream has to be half the size, because conversion
        // amplifies the size by up to 2 in worse case where every byte is \n
        let mut read_buf = vec![0; buf.len() / 2];
        let read_size = self.inner.read(&mut read_buf[..])?;
        let conv_size = Self::convert_lf_to_crlf(&read_buf[0..read_size], buf);
        Ok(conv_size)
    }
}

/// Combine a read-only stream and a write-only stream into one read-write stream.
pub struct ReadWriteAdapter<R: Read, W: Write> {
    reader: R,
    writer: W,
}

impl<R: Read, W: Write> ReadWriteAdapter<R, W> {
    pub fn new(reader: R, writer: W) -> Self {
        Self { reader, writer }
    }
}

impl<R: Read, W: Write> Read for ReadWriteAdapter<R, W> {
    fn read(&mut self, buf: &mut [u8]) -> std::result::Result<usize, std::io::Error> {
        self.reader.read(buf)
    }
}
impl<R: Read, W: Write> Write for ReadWriteAdapter<R, W> {
    fn write(&mut self, buf: &[u8]) -> std::result::Result<usize, std::io::Error> {
        self.writer.write(buf)
    }
    fn flush(&mut self) -> std::result::Result<(), std::io::Error> {
        self.writer.flush()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_converter() {
        let data = b"this is a test\nreally\n";
        let mut converted = ConvertLFtoCRLF::new(&data[..]);

        let mut buf = vec![0; 1024];
        let read_size = converted.read(&mut buf[..]).unwrap();

        assert_eq!(24, read_size);

        let expected = b"this is a test\r\nreally\r\n".to_vec();
        assert_eq!(expected[..], buf[0..read_size]);
    }

    #[test]
    fn test_adapter_read() {
        let data = b"I love spaghetti";
        let mut adapter = ReadWriteAdapter::new(&data[..], vec![]);

        let mut buf = vec![0; 1024];
        let read_size = adapter.read(&mut buf).unwrap();
        assert_eq!(16, read_size);
        assert_eq!(data[..], buf[0..read_size]);
    }

    #[test]
    fn test_adapter_write() {
        let data = b"I love spaghetti";
        let readbuf = vec![];
        let mut writebuf = vec![0; 1024];

        let mut adapter = ReadWriteAdapter::new(&readbuf[..], &mut writebuf[..]);
        let write_size = adapter.write(&data[..]).unwrap();

        assert_eq!(16, write_size);
        assert_eq!(data[..], writebuf[0..write_size]);
    }
}
