use std::time::Duration;

pub(crate) mod welcome;

pub(crate) fn format_duration(duration: Duration) -> String {
    format!("{} ms", duration.as_millis())
}
