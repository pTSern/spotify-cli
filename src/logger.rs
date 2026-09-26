use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

fn get_timestamp() -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    format!("{}", now)
}

pub fn log_error(context: &str, error_msg: &str, details: Option<&str>) {
    let _ = create_dir_all("logs");
    let filename = "logs/error.log";

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(filename) {
        let ts = get_timestamp();
        let _ = writeln!(file, "==================================================");
        let _ = writeln!(file, "[Timestamp: {}] ERROR in Context: {}", ts, context);
        let _ = writeln!(file, "Error: {}", error_msg);
        if let Some(d) = details {
            let _ = writeln!(file, "Details:\n{}", d);
        }
        let _ = writeln!(file, "==================================================\n");
    }
}

pub fn log_api_call(
    method: &str,
    url: &str,
    request_body: Option<&str>,
    status: Option<u16>,
    response_body: &str,
) {
    let _ = create_dir_all("logs");
    let filename = "logs/api_errors.log";

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(filename) {
        let ts = get_timestamp();
        let _ = writeln!(file, "[Timestamp: {}] {} {}", ts, method, url);
        if let Some(b) = request_body {
            let _ = writeln!(file, "Request Body: {}", b);
        }
        if let Some(st) = status {
            let _ = writeln!(file, "Response Status: {}", st);
        }
        let _ = writeln!(file, "Response Body:\n{}", response_body);
        let _ = writeln!(file, "--------------------------------------------------");
    }
}
