#[allow(unused_imports)]
use std::net::TcpListener;
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::TcpStream,
};

enum HttpMethod {
    GET,
    POST,
}
struct HttpRequest {
    method: HttpMethod,
    uri: String,
    headers: HashMap<String, String>,
}

impl HttpRequest {
    fn from_tcp_stream(stream: &mut TcpStream) -> Result<Option<Self>, String> {
        let mut buffer: [u8; 512] = [0; 512];

        let bytes = match stream.read(&mut buffer) {
            Ok(bytes) => bytes,
            Err(e) => return Err(format!("Error reading stream: {}", e)),
        };

        if bytes == 0 {
            return Ok(None);
        }

        let request_str = String::from_utf8_lossy(&buffer[..bytes]);

        let mut lines = request_str.lines();

        let mut headers: HashMap<String, String> = HashMap::new();

        let request_line = lines.next().ok_or("Empty request")?;
        let mut parts = request_line.split_whitespace();
        let method_str = parts.next().ok_or("Missing method")?;
        let uri_str = parts.next().ok_or("Missing uri")?;

        let method = match method_str {
            "GET" => HttpMethod::GET,
            "POST" => HttpMethod::POST,
            _ => return Err("Unsupported method".to_string()),
        };

        let uri = uri_str.to_string();

        for header_line in lines {
            if header_line.trim().is_empty() {
                break;
            }
            if let Some((key, value)) = header_line.split_once(':') {
                headers.insert(key.trim().to_lowercase(), value.trim().to_string());
            } else {
                return Err("Malformed header".to_string());
            }
        }

        Ok(Some(HttpRequest {
            method,
            uri,
            headers,
        }))
    }
}

fn handle_connection(mut stream: TcpStream) {
    if let Ok(Some(request)) = HttpRequest::from_tcp_stream(&mut stream) {
        let uri = request.uri;
        match uri.as_str() {
            "/" => stream
                .write_all("HTTP/1.1 200 OK\r\n\r\n".as_bytes())
                .unwrap(),
            "/user-agent" => {
                let user_agent_header = request.headers.get(&"User-agent".to_lowercase()).unwrap();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                    user_agent_header.len(),
                    user_agent_header.to_string()
                );
                stream.write_all(response.as_bytes()).unwrap()
            }
            _ if uri.starts_with("/echo/") => {
                let content = &uri["/echo/".len()..];
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                    content.len(),
                    content
                );
                stream.write_all(response.as_bytes()).unwrap()
            }
            _ => stream
                .write_all("HTTP/1.1 404 Not Found\r\n\r\n".as_bytes())
                .unwrap(),
        };
    }
}

fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    println!("Logs from your program will appear here!");

    // Uncomment this block to pass the first stage

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    for stream in listener.incoming() {
        match stream {
            Ok(_stream) => {
                println!("accepted new connection");
                handle_connection(_stream);
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
