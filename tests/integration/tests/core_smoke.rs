use aisec_core::{init_logging, AisecError, LogOptions};

#[test]
fn logging_initializes_with_temp_directory() {
    let temp = tempfile::tempdir().expect("tempdir");
    let guard = init_logging(
        LogOptions::bootstrap("integration-test").with_log_dir(temp.path()),
    )
    .expect("logging should initialize");

    tracing::info!("integration test log line");
    drop(guard);
}

#[test]
fn error_codes_map_to_client_messages() {
    let err = AisecError::invalid_input("bad payload");
    assert_eq!(err.code().as_str(), "INVALID_INPUT");
    assert_eq!(err.client_message(), "bad payload");
}
