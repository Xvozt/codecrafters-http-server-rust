#[allow(unused_imports)]
use std::net::TcpListener;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Read, Write},
    net::TcpStream,
    path::PathBuf,
    thread,
};

use flate2::{write::GzEncoder, Compression};

enum HttpMethod {
    GET,
    POST,
}
#[allow(dead_code)]
struct HttpRequest {
    method: HttpMethod,
    uri: String,
    headers: HashMap<String, String>,
    body: Option<Vec<u8>>,
}

struct HttpResponse {
    status: StatusCode,
    headers: Vec<(&'static str, String)>,
    body: Option<Vec<u8>>,
}

impl HttpResponse {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buffer: Vec<u8> = Vec::new();
        buffer.extend_from_slice(b"HTTP/1.1 ");
        buffer.extend_from_slice(self.status.code().to_string().as_bytes());
        buffer.extend_from_slice(b" ");
        buffer.extend_from_slice(self.status.reason().as_bytes());
        buffer.extend_from_slice(b"\r\n");
        let mut headers = self.headers.clone();

        if !headers
            .iter()
            .any(|(k, _v)| k.eq_ignore_ascii_case("content-length"))
        {
            let len = self.body.as_ref().map_or(0, |b| b.len());
            headers.push(("Content-Length", len.to_string()));
        }

        for (name, value) in headers {
            buffer.extend_from_slice(name.as_bytes());
            buffer.extend_from_slice(b": ");
            buffer.extend_from_slice(value.as_bytes());
            buffer.extend_from_slice(b"\r\n");
        }

        buffer.extend_from_slice(b"\r\n");

        if let Some(body) = &self.body {
            buffer.extend_from_slice(body);
        };

        buffer
    }
}

enum StatusCode {
    OK,
    Created,
    NotFound,
    InternalError,
}

impl StatusCode {
    pub fn code(&self) -> u16 {
        match self {
            Self::OK => 200,
            Self::Created => 201,
            Self::NotFound => 404,
            Self::InternalError => 500,
        }
    }

    pub fn reason(&self) -> &'static str {
        match self {
            Self::OK => "OK",
            Self::Created => "Created",
            Self::NotFound => "Not Found",
            Self::InternalError => "Internal Server Error",
        }
    }
}

impl Default for HttpResponse {
    fn default() -> Self {
        Self {
            status: StatusCode::OK,
            headers: vec![],
            body: None,
        }
    }
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

        let mut request_body: Option<Vec<u8>> = None;

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

        if let Some(len) = headers.get("content-length") {
            let body_len = len
                .parse::<usize>()
                .map_err(|_| "Invalid content length".to_string())?;
            let header_end_idx = buffer[..bytes]
                .windows(4)
                .position(|w| w == b"\r\n\r\n")
                .ok_or("Header terminator not found")?;
            let body_start = header_end_idx + 4;
            if body_start + body_len <= bytes {
                let body = &buffer[body_start..(body_start + body_len)];
                request_body = Some(body.to_vec())
            } else {
                return Err("Body reading problem".to_string());
            }
        };

        Ok(Some(HttpRequest {
            method,
            uri,
            headers,
            body: request_body,
        }))
    }
}

fn handle_connection(mut stream: TcpStream) {
    if let Ok(Some(request)) = HttpRequest::from_tcp_stream(&mut stream) {
        let mut response = HttpResponse::default();
        let uri = request.uri;
        match uri.as_str() {
            "/" => response.status = StatusCode::OK,
            "/user-agent" => {
                let user_agent_header = request.headers.get(&"user-agent".to_lowercase()).unwrap();
                response.status = StatusCode::OK;
                response
                    .headers
                    .push(("Content-Type", "text/plain".to_string()));
                response
                    .headers
                    .push(("Content-Length", user_agent_header.len().to_string()));
                response.body = Some(user_agent_header.to_string().as_bytes().to_vec());
            }
            _ if uri.starts_with("/files/") => match request.method {
                HttpMethod::GET => {
                    let filename = &uri["/files/".len()..];
                    let env_args: Vec<String> = std::env::args().collect();
                    let dir = PathBuf::from(env_args[2].clone());
                    let path = dir.join(filename);

                    match fs::read(&path) {
                        Ok(bytes) => {
                            response.status = StatusCode::OK;
                            response
                                .headers
                                .push(("Content-Type", "application/octet-stream".to_string()));
                            response.body = Some(bytes);
                            fs::remove_file(&path).unwrap();
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                            response.status = StatusCode::NotFound;
                        }
                        Err(_) => response.status = StatusCode::InternalError,
                    }
                }
                HttpMethod::POST => {
                    let filename = &uri["/files/".len()..];
                    let directory_arg = get_directory_arg().unwrap();
                    let dir = PathBuf::from(directory_arg);
                    let path = dir.join(filename);

                    if let Some(body) = request.body {
                        let mut file = File::create(path).unwrap();
                        file.write_all(&body).unwrap();
                        response.status = StatusCode::Created;
                    }
                }
            },
            _ if uri.starts_with("/echo/") => {
                let content = &uri["/echo/".len()..];
                response.status = StatusCode::OK;
                response
                    .headers
                    .push(("Content-Type", "text/plain".to_string()));
                response.body = Some(content.as_bytes().to_vec());

                if let Some(encoding) = request.headers.get("accept-encoding") {
                    if encoding.split(",").any(|e| e.trim() == "gzip") {
                        response
                            .headers
                            .push(("Content-Encoding", "gzip".to_string()));
                        let encoded_body = gzip_bytes(content.as_bytes());
                        response.body = Some(encoded_body);
                    }
                }
            }
            _ => response.status = StatusCode::NotFound,
        };
        let response_as_bytes = response.to_bytes();
        stream.write_all(&response_as_bytes).unwrap();
    }
}

fn get_directory_arg() -> Result<String, String> {
    let mut args = std::env::args().skip(1); // skip program name

    while let Some(arg) = args.next() {
        if arg == "--directory" {
            return args
                .next()
                .ok_or("--directory flag provided but no path given".to_string());
        }
    }

    Err("--directory flag not found".to_string())
}

fn gzip_bytes(input: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(input).unwrap();
    encoder.finish().unwrap()
}

fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    println!("Logs from your program will appear here!");

    // Uncomment this block to pass the first stage

    let listener = TcpListener::bind("127.0.0.1:4221").unwrap();

    for stream in listener.incoming() {
        thread::spawn(move || match stream {
            Ok(_stream) => {
                println!("accepted new connection");
                handle_connection(_stream);
            }
            Err(e) => {
                println!("error: {}", e);
            }
        });
    }
}
