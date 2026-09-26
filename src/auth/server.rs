use anyhow::{bail, Context, Result};
use std::collections::HashMap;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

pub async fn wait_for_callback(expected_state: &str, port: u16) -> Result<String> {
    let addr = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(&addr)
        .await
        .with_context(|| format!("Failed to bind local loopback server to {}", addr))?;

    println!("Waiting for Spotify authorization via browser callback...");

    let (mut stream, _) = listener.accept().await?;
    let mut buffer = [0u8; 4096];
    let bytes_read = stream.read(&mut buffer).await?;
    let request_str = String::from_utf8_lossy(&buffer[..bytes_read]);

    // Parse the HTTP request line, e.g. "GET /callback?code=AQD...&state=xyz HTTP/1.1"
    let first_line = request_str.lines().next().unwrap_or("");
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");

    if method != "GET" {
        send_response(&mut stream, 405, "Method Not Allowed", "Only GET allowed").await?;
        bail!("Expected GET request from browser redirect");
    }

    let url_to_parse = format!("http://127.0.0.1:{}{}", port, target);
    let parsed_url = url::Url::parse(&url_to_parse)
        .with_context(|| format!("Failed to parse redirect URL: {}", url_to_parse))?;

    let params: HashMap<String, String> = parsed_url.query_pairs().into_owned().collect();

    if let Some(err) = params.get("error") {
        let msg = format!("Authorization denied or failed: {}", err);
        send_response(&mut stream, 400, "Bad Request", &msg).await?;
        bail!(msg);
    }

    let state = params.get("state").cloned().unwrap_or_default();
    if state != expected_state {
        let msg = "State mismatch. Potential CSRF detected.";
        send_response(&mut stream, 400, "Bad Request", msg).await?;
        bail!(msg);
    }

    let code = match params.get("code") {
        Some(c) => c.clone(),
        None => {
            let msg = "Authorization code missing in callback.";
            send_response(&mut stream, 400, "Bad Request", msg).await?;
            bail!(msg);
        }
    };

    let success_html = r#"<!DOCTYPE html>
<html>
<head>
    <title>Spotify CLI - Logged In</title>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; display: flex; align-items: center; justify-content: center; height: 100vh; margin: 0; background: #121212; color: #FFFFFF; }
        .card { text-align: center; background: #181818; padding: 40px 60px; border-radius: 12px; box-shadow: 0 8px 24px rgba(0,0,0,0.5); }
        h1 { color: #1DB954; margin-bottom: 8px; font-size: 28px; }
        p { color: #B3B3B3; font-size: 16px; margin-top: 0; }
        .badge { background: #282828; display: inline-block; padding: 6px 14px; border-radius: 20px; font-size: 14px; margin-top: 15px; color: #1DB954; }
    </style>
</head>
<body>
    <div class="card">
        <h1>Authorization Successful!</h1>
        <p>Your Spotify account is now connected to <b>spotify-cli</b>.</p>
        <div class="badge">&#10004; You can safely close this browser window.</div>
    </div>
</body>
</html>"#;

    send_response(&mut stream, 200, "OK", success_html).await?;
    Ok(code)
}

async fn send_response(
    stream: &mut tokio::net::TcpStream,
    status_code: u16,
    status_text: &str,
    body: &str,
) -> Result<()> {
    let is_html = body.trim_start().starts_with("<!DOCTYPE") || body.trim_start().starts_with("<html");
    let content_type = if is_html { "text/html; charset=utf-8" } else { "text/plain; charset=utf-8" };
    let response = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status_code,
        status_text,
        content_type,
        body.len(),
        body
    );
    stream.write_all(response.as_bytes()).await?;
    stream.flush().await?;
    Ok(())
}
