#[macro_export]
macro_rules! log_info {
    () => {
        println!("[INFO]: ");
    };
    ($($arg:tt)*) => {{
        print!("[INFO]: ");
        println!($($arg)*);
    }};
}

#[macro_export]
macro_rules! log_warning {
    () => {
        println!("[WARNING]: ");
    };
    ($($arg:tt)*) => {{
        print!("[WARNING]: ");
        println!($($arg)*);
    }};
}

#[macro_export]
macro_rules! log_error {
    () => {
        eprintln!("[ERROR]: ");
    };
    ($($arg:tt)*) => {{
        eprint!("[ERROR]: ");
        eprintln!($($arg)*);
    }};
}
