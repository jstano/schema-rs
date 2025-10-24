use std::io::{BufWriter, Write};

pub struct PrintWriter {
    writer: BufWriter<Box<dyn Write>>,
    auto_flush: bool,
}

impl PrintWriter {
    pub fn new(writer: Box<dyn Write>) -> Self {
        Self {
            writer: BufWriter::new(writer),
            auto_flush: false,
        }
    }

    pub fn new_auto_flush(writer: Box<dyn Write>) -> Self {
        Self {
            writer: BufWriter::new(writer),
            auto_flush: true,
        }
    }

    pub fn print(&mut self, text: &str) {
        write!(self.writer, "{}", text).unwrap_or_else(|e| panic!("Error while writing: {}", e))
    }

    pub fn println(&mut self, text: &str) {
        writeln!(self.writer, "{}", text).unwrap_or_else(|e| panic!("Error while writing: {}", e));
        if self.auto_flush {
            self.flush();
        }
    }

    pub fn printf(&mut self, args: std::fmt::Arguments) {
        write!(self.writer, "{}", args).unwrap_or_else(|e| panic!("Error while writing: {}", e));
        if self.auto_flush {
            self.flush();
        }
    }

    pub fn newline(&mut self) {
        writeln!(self.writer).unwrap_or_else(|e| panic!("Error while writing: {}", e));
        if self.auto_flush {
            self.flush();
        }
    }

    pub fn flush(&mut self) {
        self.writer.flush().unwrap_or_else(|e| panic!("Error while flushing: {}", e))
    }
}
