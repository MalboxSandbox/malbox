use tracing::level_filters::LevelFilter;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

/// Parse a log-level string ("error" | "warn" | "info" | "debug" | "trace",
/// case-insensitive) into a `LevelFilter`.
pub fn parse_log_level(s: &str) -> Result<LevelFilter, String> {
    match s.to_ascii_lowercase().as_str() {
        "error" => Ok(LevelFilter::ERROR),
        "warn" => Ok(LevelFilter::WARN),
        "info" => Ok(LevelFilter::INFO),
        "debug" => Ok(LevelFilter::DEBUG),
        "trace" => Ok(LevelFilter::TRACE),
        other => Err(format!(
            "invalid log level `{other}`: expected error|warn|info|debug|trace"
        )),
    }
}

/// Initialise the global tracing subscriber.
///
/// `default_level` is the fallback filter used when `RUST_LOG` is not set.
/// `RUST_LOG` always wins when present.
pub fn init_tracing(default_level: LevelFilter) {
    use tracing_error::ErrorLayer;

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_ansi(true)
        .with_file(true)
        .with_line_number(true);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // Guest plugins can be chatty; their full output is written to a per-plugin log
        // file, so we only surface warn/error in the daemon log by default. Override with
        // `RUST_LOG=guest=<lvl>,...` for verbose forwarding.
        EnvFilter::new(format!(
            "malbox={lvl},libvirt={lvl},guest=warn",
            lvl = default_level
        ))
    });

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(ErrorLayer::default())
        .init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_log_level_accepts_standard_levels() {
        assert_eq!(parse_log_level("error").unwrap(), LevelFilter::ERROR);
        assert_eq!(parse_log_level("warn").unwrap(), LevelFilter::WARN);
        assert_eq!(parse_log_level("info").unwrap(), LevelFilter::INFO);
        assert_eq!(parse_log_level("debug").unwrap(), LevelFilter::DEBUG);
        assert_eq!(parse_log_level("trace").unwrap(), LevelFilter::TRACE);
    }

    #[test]
    fn parse_log_level_is_case_insensitive() {
        assert_eq!(parse_log_level("INFO").unwrap(), LevelFilter::INFO);
        assert_eq!(parse_log_level("Warn").unwrap(), LevelFilter::WARN);
    }

    #[test]
    fn parse_log_level_rejects_garbage() {
        assert!(parse_log_level("banana").is_err());
        assert!(parse_log_level("").is_err());
    }

    #[test]
    fn span_trace_captures_active_span_when_error_layer_is_installed() {
        use tracing_error::{ErrorLayer, SpanTrace};
        use tracing_subscriber::prelude::*;

        let subscriber = tracing_subscriber::registry().with(ErrorLayer::default());
        let _guard = tracing::subscriber::set_default(subscriber);

        let span = tracing::info_span!("outer", key = "value");
        let _enter = span.enter();

        let trace = SpanTrace::capture();
        let rendered = format!("{trace}");
        assert!(
            rendered.contains("outer"),
            "span trace should mention `outer` span, got: {rendered}"
        );
    }
}
