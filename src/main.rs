#[allow(unused_imports)]
use std::net::TcpListener;
use std::{
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

        let request_str = String::from_utf8_lossy(&buffer);

        let mut lines = request_str.lines();

        if let Some(request_line) = lines.next() {
            let parts: Vec<&str> = request_line.split_whitespace().collect();
            if parts.len() >= 2 {
                let method = match parts[0] {
                    "GET" => HttpMethod::GET,
                    "POST" => HttpMethod::POST,
                    _ => return Err("Unsuported method".to_string()),
                };
                let uri = parts[1].to_string();
                return Ok(Some(HttpRequest { method, uri: uri }));
            }
        }
        Ok(None)
    }
}

fn handle_connection(mut stream: TcpStream) {
    if let Ok(Some(request)) = HttpRequest::from_tcp_stream(&mut stream) {
        let uri = request.uri;
        match uri.as_str() {
            "/" => stream.write("HTTP/1.1 200 OK\r\n\r\n".as_bytes()).unwrap(),
            _ if uri.starts_with("/echo/") => {
                let content = &uri["/echo/".len()..];
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n{}",
                    content.len(),
                    content
                );
                stream.write(response.as_bytes()).unwrap()
            }
            _ => stream
                .write("HTTP/1.1 404 Not Found\r\n\r\n".as_bytes())
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
