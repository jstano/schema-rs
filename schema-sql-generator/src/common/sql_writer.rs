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
