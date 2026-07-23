use crate::context::{build_context, render_json as render_context_json, ContextOptions};
use crate::model::{Error, GraphIndex, Result};
use crate::search::find_symbols;
use crate::storage;
use crate::util::json_escape;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;

pub fn serve(root: &Path, _graph: GraphIndex, address: &str) -> Result<()> {
    let listener = TcpListener::bind(address)?;
    eprintln!("CodeSpace REST listening on http://{address}");
    let root = Arc::new(root.to_path_buf());
    for stream_result in listener.incoming() {
        match stream_result {
            Ok(stream) => {
                let root = Arc::clone(&root);
                thread::spawn(move || {
                    if let Err(error) = handle_connection(stream, &root) {
                        eprintln!("REST request failed: {error}");
                    }
                });
            }
            Err(error) => eprintln!("REST accept failed: {error}"),
        }
    }
    Ok(())
}

fn handle_connection(mut stream: TcpStream, root: &PathBuf) -> Result<()> {
    stream.set_read_timeout(Some(std::time::Duration::from_secs(3)))?;
    let mut buffer = [0_u8; 16_384];
    let read = stream.read(&mut buffer)?;
    if read == 0 {
        return Ok(());
    }
    let request = String::from_utf8_lossy(&buffer[..read]);
    let first_line = request.lines().next().unwrap_or_default();
    let mut parts = first_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let target = parts.next().unwrap_or("/");
    if method != "GET" {
        return write_response(&mut stream, 405, "application/json", "{\"error\":\"method not allowed\"}");
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let params = parse_query(query);
    let graph = match storage::load(root) {
        Ok(graph) => graph,
        Err(error) => {
            let body = format!(
                "{{\"error\":\"index unavailable\",\"detail\":\"{}\"}}",
                json_escape(&error.to_string())
            );
            return write_response(&mut stream, 500, "application/json", &body);
        }
    };
    match path {
        "/health" | "/v1/health" => write_response(
            &mut stream,
            200,
            "application/json",
            &format!(
                "{{\"status\":\"ok\",\"files\":{},\"symbols\":{},\"edges\":{}}}",
                graph.files.len(),
                graph.symbols.len(),
                graph.edges.len()
            ),
        ),
        "/v1/search" => {
            let needle = params.get("q").map_or("", String::as_str).trim();
            if needle.is_empty() {
                return write_response(
                    &mut stream,
                    400,
                    "application/json",
                    "{\"error\":\"missing non-empty q parameter\"}",
                );
            }
            let limit = params
                .get("limit")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(20)
                .min(200);
            let hits = find_symbols(&graph, needle, None, limit);
            let body = hits
                .iter()
                .filter_map(|hit| graph.symbols.get(&hit.symbol_id).map(|symbol| (hit, symbol)))
                .map(|(hit, symbol)| {
                    let path = graph.file_for_symbol(symbol).map_or("", |file| file.path.as_str());
                    format!(
                        "{{\"id\":{},\"name\":\"{}\",\"qualified_name\":\"{}\",\"kind\":\"{}\",\"path\":\"{}\",\"line\":{},\"score_milli\":{}}}",
                        symbol.id,
                        json_escape(&symbol.name),
                        json_escape(&symbol.qualified_name),
                        symbol.kind.as_str(),
                        json_escape(path),
                        symbol.line_start,
                        hit.score_milli
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            write_response(&mut stream, 200, "application/json", &format!("[{body}]"))
        }
        "/v1/context" => {
            let needle = params.get("q").map_or("", String::as_str).trim();
            if needle.is_empty() {
                return write_response(
                    &mut stream,
                    400,
                    "application/json",
                    "{\"error\":\"missing non-empty q parameter\"}",
                );
            }
            let mut options = ContextOptions::default();
            if let Some(tokens) = params.get("max_tokens").and_then(|value| value.parse::<usize>().ok()) {
                options.max_tokens = tokens.clamp(128, 32_000);
            }
            match build_context(root, &graph, needle, &options) {
                Ok(bundle) => write_response(
                    &mut stream,
                    200,
                    "application/json",
                    &render_context_json(&bundle),
                ),
                Err(error) => {
                    let body = format!(
                        "{{\"error\":\"context generation failed\",\"detail\":\"{}\"}}",
                        json_escape(&error.to_string())
                    );
                    write_response(&mut stream, 500, "application/json", &body)
                }
            }
        }
        _ => write_response(&mut stream, 404, "application/json", "{\"error\":\"not found\"}"),
    }
}

fn parse_query(query: &str) -> std::collections::BTreeMap<String, String> {
    query
        .split('&')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let (key, value) = part.split_once('=').unwrap_or((part, ""));
            (url_decode(key), url_decode(value))
        })
        .collect()
}

fn url_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                output.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                if let (Some(high), Some(low)) =
                    (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
                {
                    output.push((high << 4) | low);
                    index += 3;
                } else {
                    output.push(bytes[index]);
                    index += 1;
                }
            }
            byte => {
                output.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&output).to_string()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn write_response(stream: &mut TcpStream, status: u16, content_type: &str, body: &str) -> Result<()> {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        500 => "Internal Server Error",
        _ => "Error",
    };
    let response = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).map_err(Error::Io)
}
