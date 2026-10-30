//! The dev bridge's server. One thread, one request at a time, like the app's locked service.

use lancelot_bridge::{Bridge, Response};
use tiny_http::{Header, Method, Server};

const ADDRESS: &str = "127.0.0.1:5174";
/// Sent by `app/src/lib/api/dev.ts`. Other pages open in the browser cannot set it without a
/// preflight, which this server never answers, so they cannot call commands that write files.
const GUARD: &str = "x-lancelot-bridge";

fn header(name: &str, value: &str) -> Option<Header> {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).ok()
}

fn main() {
    let server = Server::http(ADDRESS).expect("the bridge's port is taken");
    let dir = std::env::temp_dir().join("lancelot-bridge");
    let mut bridge = Bridge::new(&dir).expect("the example files could not be created");
    println!(
        "Lancelot dev bridge on http://{ADDRESS}, files in {}",
        dir.display()
    );

    for mut request in server.incoming_requests() {
        let post = *request.method() == Method::Post;
        let guarded = request
            .headers()
            .iter()
            .any(|h| h.field.as_str().as_str().eq_ignore_ascii_case(GUARD));
        let mut body = Vec::new();
        let read = request.as_reader().read_to_end(&mut body);
        let url = request.url().to_owned();

        let answer = if read.is_err() || (post && !guarded) {
            Response {
                status: 403,
                mime: "text/plain".to_owned(),
                body: b"not from the dev UI".to_vec(),
            }
        } else {
            let method = if post { "POST" } else { "GET" };
            bridge.handle(method, &url, &body)
        };
        let mut response =
            tiny_http::Response::from_data(answer.body).with_status_code(answer.status);
        if let Some(h) = header("Content-Type", &answer.mime) {
            response.add_header(h);
        }
        if let Err(e) = request.respond(response) {
            eprintln!("{url}: {e}");
        }
    }
}
