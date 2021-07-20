use proxy_wasm::types::LogLevel;

#[macro_export(local_inner_macros)]
macro_rules! log {
    (context: $context:expr, $lvl:expr, $($arg:tt)+) => ({
        $crate::log::__custom_log($context, std::format_args!($($arg)+), $lvl)
    })
}

#[macro_export(local_inner_macros)]
macro_rules! debug {
    (context: $context:expr, $($arg:tt)+) => (
        log!(context: $context, proxy_wasm::types::LogLevel::Debug, $($arg)+)
    )
}

#[macro_export(local_inner_macros)]
macro_rules! info {
    (context: $context:expr, $($arg:tt)+) => (
        log!(context: $context, proxy_wasm::types::LogLevel::Info, $($arg)+)
    )
}

#[macro_export(local_inner_macros)]
macro_rules! warn {
    (context: $context:expr, $($arg:tt)+) => (
        log!(context: $context, proxy_wasm::types::LogLevel::Warn, $($arg)+)
    )
}

#[cfg(feature = "visible_logs")]
mod visible_logs {
    use std::cell::RefCell;
    use std::collections::HashMap;

    thread_local! {
        static STORED_LOGS: RefCell<HashMap<usize,Vec<String>>> = RefCell::new(HashMap::new());
    }

    pub fn store_logs(context: usize, message: &str) {
        STORED_LOGS.with(|container| {
            let mut stored_logs = container.borrow_mut();
            (*stored_logs)
                .entry(context)
                .or_insert_with(|| vec![message.to_string()]);
        });
    }
}

pub fn __custom_log(context: usize, args: std::fmt::Arguments, level: LogLevel) {
    let message = format!("context# {}: {}", context, args.to_string());
    #[cfg(feature = "visible_logs")]
    visible_logs::store_logs(context, &message);
    proxy_wasm::hostcalls::log(level, &message).unwrap();
}
