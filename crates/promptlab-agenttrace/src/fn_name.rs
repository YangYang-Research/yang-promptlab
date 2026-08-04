//! Capture the Rust function name of the call site (no hard-coded span labels).

/// Strip the nested marker / path noise down to the callable leaf name.
///
/// Examples:
/// - `crate::mod::my_fn::f` → `my_fn`
/// - `crate::mod::<impl Trait for T>::completion::f` → `completion`
/// - `crate::mod::async_fn::{{closure}}::__marker` → `async_fn`
pub fn short_function_name(full: &'static str) -> &'static str {
    let trimmed = full
        .strip_suffix("::__agenttrace_fn_marker")
        .or_else(|| full.strip_suffix("::f"))
        .unwrap_or(full);

    // Async fn / nested closures append `{{closure}}` segments — skip them.
    let mut end = trimmed.len();
    loop {
        let head = &trimmed[..end];
        let Some((before, last)) = head.rsplit_once("::") else {
            return head;
        };
        if last.starts_with("{{closure}}") {
            end = before.len();
            continue;
        }
        return last;
    }
}

/// Return `type_name` of a nested marker fn so macros can derive the caller name.
pub fn type_name_of<F>(_: F) -> &'static str {
    std::any::type_name::<F>()
}

/// Expand to the short name of the function that invoked this macro.
#[macro_export]
macro_rules! caller_fn_name {
    () => {{
        fn __agenttrace_fn_marker() {}
        $crate::fn_name::short_function_name($crate::fn_name::type_name_of(
            __agenttrace_fn_marker,
        ))
    }};
}

/// Soft-start a span, filling empty `name` from the caller's Rust function name.
///
/// ```ignore
/// start_span!(trace.as_ref(), SpanStart {
///     name: String::new(), // → auto: this_function
///     kind: SpanKind::Llm,
///     ..
/// }).await;
/// ```
///
/// Pass an explicit `name` to override (e.g. tool call ids inside closures).
#[macro_export]
macro_rules! start_span {
    ($trace:expr, $start:expr) => {{
        let mut __start: $crate::SpanStart = $start;
        if __start.name.trim().is_empty() {
            __start.name = $crate::caller_fn_name!().to_string();
        }
        $crate::soft_start_span($trace, __start)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_fn() -> &'static str {
        caller_fn_name!()
    }

    #[test]
    fn caller_fn_name_captures_sample_fn() {
        assert_eq!(sample_fn(), "sample_fn");
    }

    #[test]
    fn short_function_name_strips_marker() {
        assert_eq!(
            short_function_name("crate::mod::completion::__agenttrace_fn_marker"),
            "completion"
        );
        assert_eq!(
            short_function_name("crate::mod::<impl Foo>::completion::f"),
            "completion"
        );
        assert_eq!(
            short_function_name(
                "crate::mod::start_end_list_get_delete::{{closure}}::__agenttrace_fn_marker"
            ),
            "start_end_list_get_delete"
        );
    }

    async fn async_sample() -> &'static str {
        caller_fn_name!()
    }

    #[tokio::test]
    async fn caller_fn_name_captures_async_fn() {
        assert_eq!(async_sample().await, "async_sample");
    }
}
