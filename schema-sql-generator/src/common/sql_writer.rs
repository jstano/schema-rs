use crate::common::print_writer::PrintWriter;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone)]
pub struct SqlWriter {
    print_writer: Rc<RefCell<PrintWriter>>,
}

impl SqlWriter {
    pub fn new(writer: Rc<RefCell<PrintWriter>>) -> Self {
        Self {
            print_writer: writer,
        }
    }

    pub fn print(&mut self, text: &str) {
        self.print_writer.borrow_mut().print(text)
    }

    pub fn println(&mut self, text: &str) {
        self.print_writer.borrow_mut().println(text)
    }

    pub fn printf(&mut self, args: std::fmt::Arguments) {
        self.print_writer.borrow_mut().printf(args)
    }

    pub fn newline(&mut self) {
        self.print_writer.borrow_mut().newline()
    }
}

#[macro_export]
macro_rules! sql_print {
    ($writer:expr, $($arg:tt)*) => {
        $writer.printf(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! sql_println {
    ($writer:expr, $($arg:tt)*) => {
        $writer.printf(format_args!($($arg)*));
        $writer.newline();
    };
}

#[macro_export]
macro_rules! sql_newline {
    ($writer:expr) => {
        $writer.newline();
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::test_support::SharedBuffer;

    fn make_writer() -> (SqlWriter, SharedBuffer) {
        let buffer = SharedBuffer::new();
        let print_writer = PrintWriter::new_auto_flush(Box::new(buffer.clone()));
        (SqlWriter::new(Rc::new(RefCell::new(print_writer))), buffer)
    }

    // `print`/`printf` don't auto-flush (only println/newline do; see PrintWriter), so an
    // explicit flush is needed to observe their output through the underlying buffered writer.
    fn flush(writer: &SqlWriter) {
        writer.print_writer.borrow_mut().flush();
    }

    #[test]
    fn print_writes_without_trailing_newline() {
        let (mut writer, buffer) = make_writer();
        writer.print("hello");
        flush(&writer);
        assert_eq!(buffer.contents(), "hello");
    }

    #[test]
    fn println_appends_a_newline() {
        let (mut writer, buffer) = make_writer();
        writer.println("hello");
        assert_eq!(buffer.contents(), "hello\n");
    }

    #[test]
    fn newline_writes_a_bare_newline() {
        let (mut writer, buffer) = make_writer();
        writer.print("a");
        writer.newline();
        writer.print("b");
        flush(&writer);
        assert_eq!(buffer.contents(), "a\nb");
    }

    #[test]
    fn printf_formats_arguments() {
        let (mut writer, buffer) = make_writer();
        writer.printf(format_args!("count = {}", 42));
        flush(&writer);
        assert_eq!(buffer.contents(), "count = 42");
    }

    #[test]
    fn sql_print_macro_writes_without_newline() {
        let (mut writer, buffer) = make_writer();
        sql_print!(writer, "value {}", 1);
        flush(&writer);
        assert_eq!(buffer.contents(), "value 1");
    }

    #[test]
    fn sql_println_macro_appends_newline() {
        let (mut writer, buffer) = make_writer();
        sql_println!(writer, "value {}", 1);
        assert_eq!(buffer.contents(), "value 1\n");
    }

    #[test]
    fn sql_newline_macro_writes_bare_newline() {
        let (mut writer, buffer) = make_writer();
        sql_print!(writer, "a");
        sql_newline!(writer);
        sql_print!(writer, "b");
        flush(&writer);
        assert_eq!(buffer.contents(), "a\nb");
    }
}
