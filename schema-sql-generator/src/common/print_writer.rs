use std::io::{BufWriter, ErrorKind, Write};

pub struct PrintWriter {
    writer: BufWriter<Box<dyn Write>>,
    auto_flush: bool,
    exit_quietly_on_broken_pipe: bool,
}

fn is_broken_pipe(e: &std::io::Error) -> bool {
    e.kind() == ErrorKind::BrokenPipe
}

impl PrintWriter {
    pub fn new(writer: Box<dyn Write>) -> Self {
        Self {
            writer: BufWriter::new(writer),
            auto_flush: false,
            exit_quietly_on_broken_pipe: false,
        }
    }

    pub fn new_auto_flush(writer: Box<dyn Write>) -> Self {
        Self {
            writer: BufWriter::new(writer),
            auto_flush: true,
            exit_quietly_on_broken_pipe: false,
        }
    }

    /// Opts into treating a broken pipe as "exit the process quietly" instead of
    /// panicking. Only appropriate for a writer that's actually connected to something
    /// pipe-like (e.g. a CLI's stdout, which may be piped into `head`/`less` and closed
    /// early - a routine condition every other Unix-style text-producing tool exits from
    /// silently, and Rust ignoring `SIGPIPE` by default turns into an `io::Error` here
    /// instead of the process dying outright). This is opt-in and off by default because
    /// `PrintWriter` is also used to write to plain files (which can't produce a broken
    /// pipe in practice) and in-process buffers - an embedder writing to one of those
    /// should never have the *whole process* exited out from under it by a shared
    /// low-level writer type on some other, unexpected I/O condition.
    pub fn exit_quietly_on_broken_pipe(mut self, value: bool) -> Self {
        self.exit_quietly_on_broken_pipe = value;
        self
    }

    /// Handles a write/flush failure, following the `exit_quietly_on_broken_pipe`
    /// policy for a broken pipe specifically. Any other I/O error (disk full,
    /// permission denied, ...) is always unexpected here - there's no meaningful
    /// recovery for a generator not built around `Result`-returning writes, so it
    /// always fails loudly via panic.
    fn handle_write_error(&self, context: &str, e: std::io::Error) -> ! {
        if self.exit_quietly_on_broken_pipe && is_broken_pipe(&e) {
            std::process::exit(0);
        }
        panic!("Error while {}: {}", context, e)
    }

    pub fn print(&mut self, text: &str) {
        if let Err(e) = write!(self.writer, "{}", text) {
            self.handle_write_error("writing", e);
        }
    }

    pub fn println(&mut self, text: &str) {
        if let Err(e) = writeln!(self.writer, "{}", text) {
            self.handle_write_error("writing", e);
        }
        if self.auto_flush {
            self.flush();
        }
    }

    pub fn printf(&mut self, args: std::fmt::Arguments) {
        if let Err(e) = write!(self.writer, "{}", args) {
            self.handle_write_error("writing", e);
        }
        if self.auto_flush {
            self.flush();
        }
    }

    pub fn newline(&mut self) {
        if let Err(e) = writeln!(self.writer) {
            self.handle_write_error("writing", e);
        }
        if self.auto_flush {
            self.flush();
        }
    }

    pub fn flush(&mut self) {
        if let Err(e) = self.writer.flush() {
            self.handle_write_error("flushing", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    /// A `Write` impl that always fails with a configurable error kind, used to drive
    /// `PrintWriter` down its error-handling paths without needing a real closed pipe.
    struct FailingWriter(ErrorKind);

    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(self.0))
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::from(self.0))
        }
    }

    #[test]
    fn is_broken_pipe_classifies_error_kinds_correctly() {
        assert!(is_broken_pipe(&io::Error::from(ErrorKind::BrokenPipe)));
        assert!(!is_broken_pipe(&io::Error::from(ErrorKind::PermissionDenied)));
        assert!(!is_broken_pipe(&io::Error::from(ErrorKind::OutOfMemory)));
    }

    #[test]
    #[should_panic(expected = "Error while flushing")]
    fn flush_panics_on_a_non_broken_pipe_error() {
        // Unexpected I/O errors (not a broken pipe) must still fail loudly, since
        // nothing downstream is built to recover from a partially-written script.
        // (`print` alone doesn't reach the underlying `Write` here - `BufWriter`
        // buffers a short string rather than flushing it immediately - so this drives
        // the error path via an explicit `flush()`, which always reaches it.)
        let mut writer = PrintWriter::new(Box::new(FailingWriter(ErrorKind::PermissionDenied)));
        writer.print("hello");
        writer.flush();
    }

    #[test]
    #[should_panic(expected = "Error while flushing")]
    fn flush_panics_on_a_broken_pipe_by_default() {
        // `PrintWriter` is also used to write to plain files (schema-installer's temp
        // file, the CLI's output file) and in-process buffers - neither can produce a
        // broken pipe in practice, but if one somehow did, a shared low-level writer
        // type silently exiting the whole process out from under an arbitrary embedder
        // would be far more surprising than a panic. Quietly exiting on a broken pipe
        // is opt-in (`exit_quietly_on_broken_pipe(true)`) for exactly this reason - the
        // default must still panic.
        let mut writer = PrintWriter::new(Box::new(FailingWriter(ErrorKind::BrokenPipe)));
        writer.print("hello");
        writer.flush();
    }

    // Note: the broken-pipe-exits-quietly path itself calls `std::process::exit(0)`,
    // which can't be exercised in-process without terminating the test runner -
    // `is_broken_pipe`'s classification (tested above) is what determines whether that
    // path is taken once a caller has opted in via `exit_quietly_on_broken_pipe(true)`.
}
