use clap::Parser;
use reqwest::Client;
use serde_json::json;
use std::io::{self, Write};
/// cargo run --bin terminal_client -- --content "Hi, I want to check my bank account."
/// Simple terminal client for the chat service
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The first message content to send
    #[arg(short, long)]
    content: String,

    /// Optional session ID (if you want to resume a session)
    #[arg(short, long)]
    session_id: Option<String>,

    /// Server URL (default: http://localhost:3000/execute)
    #[arg(short, long, default_value = "http://localhost:3000/execute")]
    url: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let client = Client::new();
    let mut session_id = args.session_id;
    let _first = true;
    let mut content = args.content;
    let url = args.url;

    loop {
        let mut body = json!({
            "content": content,
        });
        if let Some(ref sid) = session_id {
            body["session_id"] = json!(sid);
        }

        // Display the request being sent
        println!("\n--- REQUEST ---");
        println!("URL: {}", url);
        println!("Body: {}", serde_json::to_string_pretty(&body)?);
        println!("--- END REQUEST ---\n");

        let resp = client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;
        println!("--- RESPONSE ---");
        println!("Status: {}", status);
        println!("Body:\n{}", text);
        println!("--- END RESPONSE ---\n");

        // Try to extract session_id from response if not set
        if session_id.is_none() {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(sid) = val.get("session_id").and_then(|v| v.as_str()) {
                    session_id = Some(sid.to_string());
                    println!("[Session started: {}]", sid);
                }
            }
        }

        // Prompt for next input
        print!("You: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();
        if input.eq_ignore_ascii_case("exit") {
            println!("Exiting chat.");
            break;
        }
        
        // Check for session info keywords
        if input.eq_ignore_ascii_case("session")
            || input.eq_ignore_ascii_case("session_info")
            || input.eq_ignore_ascii_case("session_data") {
            match &session_id {
                Some(sid) => println!("Current session ID: {}", sid),
                None => println!("No active session (session_id is None)"),
            }
            continue;
        }
        
        content = input.to_string();
    }

    Ok(())
}
