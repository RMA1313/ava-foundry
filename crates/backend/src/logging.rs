use std::io::{self, IsTerminal};

use tracing_subscriber::{filter::EnvFilter, fmt, prelude::*};

pub fn init() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let console = io::stdout().is_terminal();
    let format = std::env::var("DATA_STUDIO_LOG_FORMAT").unwrap_or_else(|_| {
        if console {
            "console".to_string()
        } else {
            "json".to_string()
        }
    });

    let subscriber = tracing_subscriber::registry().with(filter);
    let result = match format.as_str() {
        "json" => subscriber
            .with(
                fmt::layer()
                    .json()
                    .with_current_span(true)
                    .with_span_list(true),
            )
            .try_init(),
        _ => subscriber.with(fmt::layer().compact()).try_init(),
    };

    let _ = result;
}
