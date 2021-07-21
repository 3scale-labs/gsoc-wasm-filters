use proxy_wasm::types::LogLevel;

#[macro_export(local_inner_macros)]
macro_rules! log {
    ($context:expr, $lvl:expr, $($arg:tt)+) => ({
        $crate::log::__custom_log($context, std::format_args!($($arg)+), $lvl)
    })
}

#[macro_export(local_inner_macros)]
macro_rules! debug {
    ($context:expr, $($arg:tt)+) => (
        log!($context, proxy_wasm::types::LogLevel::Debug, $($arg)+)
    )
}

#[macro_export(local_inner_macros)]
macro_rules! info {
    ($context:expr, $($arg:tt)+) => (
        log!($context, proxy_wasm::types::LogLevel::Info, $($arg)+)
    )
}

#[macro_export(local_inner_macros)]
macro_rules! warn {
    ($context:expr, $($arg:tt)+) => (
        log!($context, proxy_wasm::types::LogLevel::Warn, $($arg)+)
    )
}

#[cfg(feature = "visible_logs")]
pub mod visible_logs {
    use std::cell::RefCell;
    use std::collections::HashMap;

    const LOGS_HEADER: &str = "filter-logs";

    thread_local! {
        pub static STORED_LOGS: RefCell<HashMap<u32,Vec<String>>> = RefCell::new(HashMap::new());
    }

    pub fn store_logs(context: u32, message: &str) {
        STORED_LOGS.with(|refcell| {
            let mut inner_map = refcell.borrow_mut();
            let stored_logs = (*inner_map).entry(context).or_insert_with(Vec::new);
            (*stored_logs).push(message.to_string());
        });
    }

    pub fn get_logs_header_pair(context_id: u32) -> (String, String) {
        STORED_LOGS.with(|refcell| {
            let mut inner_map = refcell.borrow_mut();
            let header_key = LOGS_HEADER.to_string();
            let stored_logs = (*inner_map).get(&context_id);
            if stored_logs.is_none() {
                return (
                    header_key,
                    "couldn't find logs in the hashmap for this context".to_string(),
                );
            }

            let serialized_logs = serde_json::to_string(stored_logs.unwrap());
            if let Err(e) = serialized_logs {
                return (header_key, format!("failed to serialize logs: {:?}", e));
            }

            // clear logs for the current context.
            (*inner_map).get_mut(&context_id).unwrap().clear();

            (header_key, serialized_logs.unwrap())
        })
    }
}

pub fn __custom_log(context: u32, args: std::fmt::Arguments, level: LogLevel) {
    let message = format!("context# {}: {}", context, args.to_string());
    #[cfg(feature = "visible_logs")]
    visible_logs::store_logs(context, &message);
    proxy_wasm::hostcalls::log(level, &message).unwrap();
}
