#[macro_export]
macro_rules! kilobytes {
    ($x:expr) => {
        $x * 1024
    };
}

#[macro_export]
macro_rules! megabytes {
    ($x:expr) => {
        $crate::kilobytes!($x) * 1024
    };
}

#[macro_export]
macro_rules! gigabytes {
    ($x:expr) => {
        $crate::megabytes!($x) * 1024
    };
}

#[macro_export]
macro_rules! terabytes {
    ($x:expr) => {
        $crate::gigabytes!($x) * 1024
    };
}
