//! Data sources: where a plugin can pull figures from over the network (technical design §10). The
//! provider shapes its data, so a source is only a URL and some names: a top-level list, and which
//! field of an item holds what. Nothing here knows about any provider, and the details stay out of
//! the repository and out of event files: they are in `data-sources.json` in the app's data folder,
//! and the token is in a [`TokenStore`].
//!
//! Only the fields asked for leave this module, so anything else in a response is never kept
//! (R-TKT-17). The app works without any of this (R-NF-3): a failed pull changes nothing.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use ts_rs::TS;
use ureq::Agent;
use ureq::http::Uri;

use crate::api::ApiError;

const TIMEOUT: Duration = Duration::from_secs(30);
const MAX_RESPONSE: u64 = 32 * 1024 * 1024;

/// One provider's details, by the plugin that uses them in the file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct DataSource {
    /// How the source is named in the app and in what it imports. The URL never is.
    pub label: String,
    pub url: String,
    /// The header the token is sent in.
    pub auth_header: String,
    /// The top-level key of the list of items.
    pub list: String,
    /// What the plugin calls a field, and what the provider calls it.
    pub fields: BTreeMap<String, String>,
}

/// A source as the form shows it: the token only as whether there is one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
#[serde(rename_all = "camelCase")]
pub struct DataSourceView {
    pub source: DataSource,
    pub has_token: bool,
}

/// What a pull read: the fields asked for of every item that has them all.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct Pulled {
    pub label: String,
    /// SHA-256 of the rows, lower-case hex. The same hash means the same figures.
    pub sha256: String,
    pub rows: Vec<BTreeMap<String, String>>,
    pub problems: Vec<PullProblem>,
}

/// An item that was not read: the field it lacks. Listed, never dropped silently (R-TKT-4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct PullProblem {
    /// Its place in the list, starting at 1.
    pub row: u32,
    pub field: String,
}

/// Where tokens are kept. The desktop app uses the system's keychain.
pub trait TokenStore: Send {
    fn get(&self, id: &str) -> Result<Option<String>, String>;
    fn set(&mut self, id: &str, token: &str) -> Result<(), String>;
    fn delete(&mut self, id: &str) -> Result<(), String>;
}

/// Tokens for as long as the process runs: for the dev bridge and tests.
#[derive(Default)]
pub struct MemoryTokens(BTreeMap<String, String>);

impl TokenStore for MemoryTokens {
    fn get(&self, id: &str) -> Result<Option<String>, String> {
        Ok(self.0.get(id).cloned())
    }

    fn set(&mut self, id: &str, token: &str) -> Result<(), String> {
        self.0.insert(id.to_owned(), token.to_owned());
        Ok(())
    }

    fn delete(&mut self, id: &str) -> Result<(), String> {
        self.0.remove(id);
        Ok(())
    }
}

pub(crate) fn load(path: &Path) -> BTreeMap<String, DataSource> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Writes the file atomically: to a temporary file next to it, then renamed into place.
pub(crate) fn save(path: &Path, sources: &BTreeMap<String, DataSource>) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(sources)?)?;
    std::fs::rename(&tmp, path)
}

fn invalid(field: &str, message: &str) -> ApiError {
    let mut error = ApiError::new("invalid_data_source", message);
    error.field = Some(field.to_owned());
    error
}

/// A token goes only to `https`, or to this computer for trying things out.
fn check_url(url: &str) -> Result<(), ApiError> {
    let uri: Uri = url
        .parse()
        .map_err(|_| invalid("url", "enter a full address"))?;
    let host = uri
        .host()
        .ok_or_else(|| invalid("url", "enter a full address"))?;
    match uri.scheme_str() {
        Some("https") => Ok(()),
        Some("http") if matches!(host, "localhost" | "127.0.0.1" | "[::1]") => Ok(()),
        Some("http") => Err(invalid("url", "the address must start with https://")),
        _ => Err(invalid("url", "enter a full address")),
    }
}

/// Trims a source and checks it can be used.
pub(crate) fn clean(source: DataSource) -> Result<DataSource, ApiError> {
    let name = |text: &str, field: &str, message: &str| {
        let text = text.trim();
        if text.is_empty() {
            return Err(invalid(field, message));
        }
        Ok(text.to_owned())
    };
    let url = source.url.trim().to_owned();
    check_url(&url)?;
    let auth_header = name(&source.auth_header, "authHeader", "enter a header name")?;
    if !auth_header
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(invalid(
            "authHeader",
            "use only letters, digits, - and _ here",
        ));
    }
    let mut fields = BTreeMap::new();
    for (key, value) in source.fields {
        let value = name(
            &value,
            &format!("fields.{key}"),
            "enter the provider's field name",
        )?;
        fields.insert(key, value);
    }
    Ok(DataSource {
        label: name(&source.label, "label", "enter a name")?,
        url,
        auth_header,
        list: name(&source.list, "list", "enter the name of the list")?,
        fields,
    })
}

fn text_of(value: Option<&Value>) -> Option<String> {
    let text = match value? {
        Value::String(text) => text.trim().to_owned(),
        Value::Number(number) => number.to_string(),
        _ => return None,
    };
    (!text.is_empty()).then_some(text)
}

/// Takes the fields asked for from a response.
pub(crate) fn read_rows(
    body: &[u8],
    source: &DataSource,
    fields: &[String],
) -> Result<Pulled, ApiError> {
    let shape = |message: String| ApiError::new("data_source_shape", message);
    let value: Value =
        serde_json::from_slice(body).map_err(|_| shape("the response is not JSON".to_owned()))?;
    let items = value
        .get(&source.list)
        .and_then(Value::as_array)
        .ok_or_else(|| shape(format!("the response has no list called `{}`", source.list)))?;
    let names = fields
        .iter()
        .map(|field| {
            source
                .fields
                .get(field)
                .map(|name| (field, name))
                .ok_or_else(|| shape(format!("no provider field is set for `{field}`")))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut rows = Vec::new();
    let mut problems = Vec::new();
    for (item, row) in items.iter().zip(1..) {
        let mut read = BTreeMap::new();
        for (field, name) in &names {
            match text_of(item.get(name.as_str())) {
                Some(text) => {
                    read.insert((*field).clone(), text);
                }
                None => problems.push(PullProblem {
                    row,
                    field: (*field).clone(),
                }),
            }
        }
        if read.len() == names.len() {
            rows.push(read);
        }
    }
    let hash = Sha256::digest(serde_json::to_vec(&rows).unwrap_or_default());
    Ok(Pulled {
        label: source.label.clone(),
        sha256: hash.iter().map(|b| format!("{b:02x}")).collect(),
        rows,
        problems,
    })
}

/// All a pull needs, taken from the service so that the network doesn't hold up the app.
pub struct PullRequest {
    pub(crate) source: DataSource,
    pub(crate) token: Option<String>,
}

impl PullRequest {
    /// Asks the provider. Redirects aren't followed, so the token goes only to the address given.
    pub fn run(&self, fields: &[String]) -> Result<Pulled, ApiError> {
        let source = &self.source;
        check_url(&source.url)?;
        let agent: Agent = Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .max_redirects(0)
            .http_status_as_error(false)
            .build()
            .into();
        let mut request = agent.get(&source.url).header("Accept", "application/json");
        if let Some(token) = &self.token {
            request = request.header(&source.auth_header, token);
        }
        let unreachable = |e: ureq::Error| {
            ApiError::new("data_source_unreachable", format!("{}: {e}", source.label))
        };
        let mut response = request.call().map_err(unreachable)?;
        let status = response.status();
        if !status.is_success() {
            return Err(ApiError::new(
                "data_source_status",
                format!("{} answered {status}", source.label),
            ));
        }
        let body = response
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE)
            .read_to_vec()
            .map_err(unreachable)?;
        read_rows(&body, source, fields)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(url: &str) -> DataSource {
        DataSource {
            label: "Drinks app".into(),
            url: url.into(),
            auth_header: "X-Api-Key".into(),
            list: "drinks".into(),
            fields: BTreeMap::from([
                ("name".into(), "name".into()),
                ("quantity".into(), "qty".into()),
            ]),
        }
    }

    fn wanted() -> Vec<String> {
        vec!["name".into(), "quantity".into()]
    }

    #[test]
    fn reads_only_the_fields_asked_for_and_lists_what_it_cannot() {
        let body = br#"{"drinks": [
            {"name": " Mate ", "qty": 20, "email": "a@x.example"},
            {"name": "Beer"},
            {"qty": [1]}
        ]}"#;
        let pulled = read_rows(body, &source("https://x.example"), &wanted()).unwrap();
        assert_eq!(
            pulled.rows,
            [BTreeMap::from([
                ("name".to_owned(), "Mate".to_owned()),
                ("quantity".to_owned(), "20".to_owned())
            ])]
        );
        let problems: Vec<_> = pulled
            .problems
            .iter()
            .map(|p| (p.row, p.field.as_str()))
            .collect();
        assert_eq!(problems, [(2, "quantity"), (3, "name"), (3, "quantity")]);
        assert!(!serde_json::to_string(&pulled).unwrap().contains("a@x"));
    }

    #[test]
    fn says_what_is_wrong_with_a_response() {
        let s = source("https://x.example");
        let read = |body: &[u8]| read_rows(body, &s, &wanted()).unwrap_err().message;
        assert!(read(b"<html>").contains("not JSON"));
        assert!(read(br#"{"beer": []}"#).contains("`drinks`"));
        let other = read_rows(br#"{"drinks": []}"#, &s, &["size".to_owned()]).unwrap_err();
        assert!(other.message.contains("`size`"));
    }

    #[test]
    fn the_hash_follows_the_figures_and_not_the_rest() {
        let s = source("https://x.example");
        let hash = |body: &[u8]| read_rows(body, &s, &wanted()).unwrap().sha256;
        let one = hash(br#"{"drinks": [{"name": "Mate", "qty": 1, "at": "10:00"}]}"#);
        assert_eq!(
            one,
            hash(br#"{"drinks": [{"name": "Mate", "qty": 1, "at": "11:00"}]}"#)
        );
        assert_ne!(one, hash(br#"{"drinks": [{"name": "Mate", "qty": 2}]}"#));
    }

    #[test]
    fn a_token_goes_only_to_https_or_this_computer() {
        let url = |u: &str| clean(source(u)).map(|_| ()).map_err(|e| e.field.unwrap());
        assert_eq!(url("https://x.example/a?b=1"), Ok(()));
        assert_eq!(url("http://localhost:5199/a"), Ok(()));
        assert_eq!(url("http://127.0.0.1/a"), Ok(()));
        assert_eq!(url("http://x.example/a"), Err("url".to_owned()));
        assert_eq!(url("x.example"), Err("url".to_owned()));
        assert_eq!(url("ftp://x.example"), Err("url".to_owned()));
    }

    #[test]
    fn a_source_needs_every_name() {
        let error = |change: fn(&mut DataSource)| {
            let mut s = source("https://x.example");
            change(&mut s);
            clean(s).unwrap_err().field.unwrap()
        };
        assert_eq!(error(|s| s.label = " ".into()), "label");
        assert_eq!(error(|s| s.list = String::new()), "list");
        assert_eq!(error(|s| s.auth_header = "a b".into()), "authHeader");
        let field = error(|s| {
            s.fields.insert("name".into(), " ".into());
        });
        assert_eq!(field, "fields.name");
    }

    /// One answer from a local server, and the auth header it was asked with.
    fn serve(status: u16, body: &'static str) -> (String, std::thread::JoinHandle<Option<String>>) {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", server.server_addr().to_ip().unwrap());
        let handle = std::thread::spawn(move || {
            let request = server.recv().unwrap();
            let token = request
                .headers()
                .iter()
                .find(|h| h.field.equiv("X-Api-Key"))
                .map(|h| h.value.to_string());
            request
                .respond(tiny_http::Response::from_string(body).with_status_code(status))
                .unwrap();
            token
        });
        (url, handle)
    }

    #[test]
    fn pulls_with_the_token_in_its_header() {
        let (url, server) = serve(200, r#"{"drinks": [{"name": "Mate", "qty": 3}]}"#);
        let pulled = PullRequest {
            source: source(&url),
            token: Some("secret".into()),
        }
        .run(&wanted())
        .unwrap();
        assert_eq!(pulled.rows[0]["quantity"], "3");
        assert_eq!(pulled.label, "Drinks app");
        assert_eq!(server.join().unwrap().as_deref(), Some("secret"));
    }

    #[test]
    fn a_refused_pull_is_an_error_that_names_the_status_and_not_the_token() {
        let (url, server) = serve(401, "no");
        let error = PullRequest {
            source: source(&url),
            token: Some("secret".into()),
        }
        .run(&wanted())
        .unwrap_err();
        assert_eq!(error.code, "data_source_status");
        assert!(error.message.contains("401"), "{}", error.message);
        assert!(!error.message.contains("secret"));
        server.join().unwrap();
    }
}
