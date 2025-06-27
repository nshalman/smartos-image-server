// Copyright 2020 Nahum Shalman
/*!
 * Rust implementation of old smartos dsapi
 */

//use anyhow::{Result, bail, anyhow};
use getopts::Options;
use schemars::JsonSchema;
use semver;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::vec::Vec;
use tokio::fs as async_fs;
use uuid::Uuid;

use dropshot::{
    endpoint, ApiDescription, Body, ConfigDropshot, ConfigLogging, ConfigLoggingLevel, HttpError, HttpResponseOk,
    Path as DropPath, RequestContext, ServerBuilder,
};
use http::{Response, StatusCode};
use std::net::SocketAddr;

/*#[macro_use]
extern crate slog;
*/

#[tokio::main]
async fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();
    let mut opts = Options::new();
    opts.optflag("h", "help", "print this help menu");
    opts.optopt("l", "listen", "listen on address:port", "LISTEN");
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            panic!("{}", f.to_string())
        }
    };
    if matches.opt_present("h") {
        let brief = format!("Usage: {} [options]", program);
        print!("{}", opts.usage(&brief));
        std::process::exit(0);
    }
    // Load configuration
    let config: Config = {
        let config_content = fs::read_to_string("config.json")
            .map_err(|e| format!("Failed to read config.json: {}", e))?;
        serde_json::from_str(&config_content)
            .map_err(|e| format!("Failed to parse config.json: {}", e))?
    };

    let serve_dir = match &config.serve_dir {
        Some(dir) => PathBuf::from(dir),
        None => std::env::current_dir().unwrap(),
    };

    /*
     * For simplicity, we'll configure an "info"-level logger that writes to
     * stderr assuming that it's a terminal.
     */
    let config_logging = ConfigLogging::StderrTerminal {
        level: ConfigLoggingLevel::Info,
    };
    let log = config_logging
        .to_logger("dsapi")
        .map_err(|error| format!("failed to create logger: {}", error))?;

    /*
     * Build a description of the API.
     */
    /*
    -setup_routes(server, '/datasets', alldatasets);
    -setup_routes(server, '/datasets/:id', manifest);
    -setup_routes(server, '/datasets/:id/:path', imagefile);
    -setup_routes(server, '/ping', ping);
    -setup_routes(server, '/', slash);
    */
    let mut api = ApiDescription::new();
    api.register(testme).unwrap();
    api.register(testme_head).unwrap();
    api.register(slash).unwrap();
    api.register(slash_head).unwrap();
    api.register(ping).unwrap();
    api.register(ping_head).unwrap();
    api.register(datasets).unwrap();
    api.register(datasets_head).unwrap();
    api.register(dataset_id).unwrap();
    api.register(dataset_id_head).unwrap();
    api.register(dataset_id_path).unwrap();
    api.register(dataset_id_path_head).unwrap();

    // Get bind address from command line or config before moving config
    let bind_config = matches
        .opt_str("l")
        .unwrap_or_else(|| match &config.listen_port {
            serde_json::Value::Number(n) => format!("127.0.0.1:{}", n),
            serde_json::Value::String(s) => {
                // If it's a string, assume it's host:port format
                s.clone()
            },
            _ => String::from("127.0.0.1:8876"),
        });

    /*
     * The functions that implement our API endpoints will share this context.
     */
    let api_description = api
        .openapi("dsapi", semver::Version::new(0, 2, 0))
        .json()
        .map_err(|e| e.to_string())?;
    //.to_string();
    let api_context = DsapiContext::new(api_description, config, serve_dir);

    /* How to emit my API at startup:
    api.print_openapi(
        &mut std::io::stdout(),
        &"dsapi",
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        &"0.2",
    )
    .map_err(|e| e.to_string())?;
    println!(""); // flush stdout with an extra newline
     */

    // Parse TCP bind address
    let bind_address: SocketAddr = bind_config
        .parse()
        .map_err(|e| format!("Invalid bind address '{}': {}", bind_config, e))?;

    let config_dropshot = ConfigDropshot {
        bind_address,
        ..Default::default()
    };

    let server = ServerBuilder::new(api, api_context, log)
        .config(config_dropshot)
        .start()
        .map_err(|error| format!("failed to create server: {}", error))?;

    server.await
}

/**
 * Application-specific context (state shared by handler functions)
 */
struct DsapiContext {
    api: Value,
    config: Config,
    serve_dir: PathBuf,
}

impl DsapiContext {
    /**
     * Return a new DsapiContext.
     */
    pub fn new(a: Value, config: Config, serve_dir: PathBuf) -> DsapiContext {
        DsapiContext {
            api: a,
            config,
            serve_dir,
        }
    }

    /**
     * Process a manifest file, adding URL properties and validating
     */
    async fn process_manifest(
        &self,
        uuid: &str,
        host: &str,
    ) -> Result<Manifest, Box<dyn std::error::Error + Send + Sync>> {
        let manifest_path = self.serve_dir.join(uuid).join("manifest.json");

        let manifest_content = async_fs::read_to_string(&manifest_path)
            .await
            .map_err(|e| format!("Failed to read manifest for {}: {}", uuid, e))?;

        let mut manifest: Manifest = serde_json::from_str(&manifest_content)
            .map_err(|e| format!("Failed to parse manifest for {}: {}", uuid, e))?;

        // Update file URLs
        let url_prefix = format!(
            "{}{}{}/datasets/{}/",
            self.config.prefix, host, self.config.suffix, uuid
        );

        for file in &mut manifest.files {
            file.url = Some(format!("{}{}", url_prefix, file.path));
        }

        Ok(manifest)
    }

    /**
     * Get all dataset UUIDs by scanning the serve directory
     */
    async fn get_all_dataset_uuids(
        &self,
    ) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
        let mut entries = async_fs::read_dir(&self.serve_dir)
            .await
            .map_err(|e| format!("Failed to read serve directory: {}", e))?;

        let mut uuids = Vec::new();

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| format!("Failed to read directory entry: {}", e))?
        {
            if let Some(name) = entry.file_name().to_str() {
                let manifest_path = entry.path().join("manifest.json");
                if async_fs::metadata(&manifest_path).await.is_ok() {
                    uuids.push(name.to_string());
                }
            }
        }

        Ok(uuids)
    }
}

/*
 * HTTP API interface
 */

/** Return the API description*/
#[endpoint {
    method = GET,
    path = "/",
}]
async fn slash(rqctx: RequestContext<DsapiContext>) -> Result<HttpResponseOk<String>, HttpError> {
    let context = rqctx.context();
    Ok(HttpResponseOk(context.api.to_string()))
}

/** HEAD support for API description*/
#[endpoint {
    method = HEAD,
    path = "/",
}]
async fn slash_head(
    rqctx: RequestContext<DsapiContext>,
) -> Result<HttpResponseOk<String>, HttpError> {
    let context = rqctx.context();
    Ok(HttpResponseOk(context.api.to_string()))
}

/** Test Function*/
#[endpoint {
    method = GET,
    path = "/test",
}]
async fn testme(_rqctx: RequestContext<DsapiContext>) -> Result<HttpResponseOk<String>, HttpError> {
    Ok(HttpResponseOk("Okay".to_string()))
}

/** HEAD support for test function*/
#[endpoint {
    method = HEAD,
    path = "/test",
}]
async fn testme_head(
    _rqctx: RequestContext<DsapiContext>,
) -> Result<HttpResponseOk<String>, HttpError> {
    Ok(HttpResponseOk("Okay".to_string()))
}

/** Ping response*/
#[derive(Deserialize, Serialize, JsonSchema)]
struct Ping {
    ping: String,
}

/** Respond to a ping with pong*/
#[endpoint {
    method = GET,
    path = "/ping",
}]
async fn ping(_rqctx: RequestContext<DsapiContext>) -> Result<HttpResponseOk<Ping>, HttpError> {
    let pong = "pong".to_string();
    Ok(HttpResponseOk(Ping { ping: pong }))
}

/** HEAD support for ping*/
#[endpoint {
    method = HEAD,
    path = "/ping",
}]
async fn ping_head(
    _rqctx: RequestContext<DsapiContext>,
) -> Result<HttpResponseOk<Ping>, HttpError> {
    let pong = "pong".to_string();
    Ok(HttpResponseOk(Ping { ping: pong }))
}

/** Represents a file in a dataset manifest */
#[derive(Serialize, Deserialize, JsonSchema, Clone)]
struct ManifestFile {
    path: String,
    sha1: String,
    size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

/** Represents a dataset manifest */
#[derive(Serialize, Deserialize, JsonSchema, Clone)]
struct Manifest {
    uuid: Uuid,
    name: String,
    version: String,
    description: String,
    os: String,
    #[serde(rename = "type")]
    manifest_type: String,
    urn: String,
    creator_name: String,
    creator_uuid: Uuid,
    published_at: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    platform_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cloud_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vendor_uuid: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    requirements: Option<serde_json::Value>,

    files: Vec<ManifestFile>,
}

#[derive(Deserialize, JsonSchema)]
struct DsapiId {
    id: Uuid, // TODO: Convert UUID path param parsing properly
}

#[derive(Deserialize, JsonSchema)]
struct DsapiIdPath {
    id: Uuid, // TODO: Convert UUID path param parsing properly
    path: String,
}

#[derive(Deserialize, Serialize, Debug)]
struct Config {
    listen_port: serde_json::Value,
    prefix: String,
    suffix: String,
    loglevel: String,
    serve_dir: Option<String>,
}

/** Get all datasets on this server*/
#[endpoint {
    method = GET,
    path = "/datasets",
}]
async fn datasets(rqctx: RequestContext<DsapiContext>) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let host = rqctx
        .request
        .headers()
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");

    let uuids = context
        .get_all_dataset_uuids()
        .await
        .map_err(|e| HttpError::for_internal_error(format!("Failed to get dataset list: {}", e)))?;

    let mut manifests = Vec::new();

    for uuid in uuids {
        match context.process_manifest(&uuid, host).await {
            Ok(manifest) => manifests.push(manifest),
            Err(e) => {
                // Log error but continue with other manifests
                eprintln!("Failed to process manifest for {}: {}", uuid, e);
            }
        }
    }

    let json_body = serde_json::to_string(&manifests)
        .map_err(|e| HttpError::for_internal_error(format!("JSON serialization failed: {}", e)))?;
    let body = Body::with_content(json_body.clone());

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/json")
        .header("Content-Length", json_body.len().to_string())
        .header(
            "Access-Control-Allow-Headers",
            "Origin, Accept, Content-Type, X-Requested-With, X-CSRF-Token",
        )
        .header(
            "Access-Control-Allow-Methods",
            "PUT, GET, POST, DELETE, OPTIONS",
        )
        .header("Access-Control-Allow-Origin", "*")
        .body(body)?)
}

/** HEAD support for datasets list*/
#[endpoint {
    method = HEAD,
    path = "/datasets",
}]
async fn datasets_head(
    rqctx: RequestContext<DsapiContext>,
) -> Result<HttpResponseOk<Vec<Manifest>>, HttpError> {
    let context = rqctx.context();
    let host = rqctx
        .request
        .headers()
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");

    let uuids = context
        .get_all_dataset_uuids()
        .await
        .map_err(|e| HttpError::for_internal_error(format!("Failed to get dataset list: {}", e)))?;

    let mut manifests = Vec::new();

    for uuid in uuids {
        match context.process_manifest(&uuid, host).await {
            Ok(manifest) => manifests.push(manifest),
            Err(e) => {
                // Log error but continue with other manifests
                eprintln!("Failed to process manifest for {}: {}", uuid, e);
            }
        }
    }

    Ok(HttpResponseOk(manifests))
}

/** Get specific dataset manifest*/
#[endpoint {
    method = GET,
    path = "/datasets/{id}",
}]
async fn dataset_id(
    rqctx: RequestContext<DsapiContext>,
    path_params: DropPath<DsapiId>,
) -> Result<HttpResponseOk<Manifest>, HttpError> {
    let context = rqctx.context();
    let path_params = path_params.into_inner();
    let host = rqctx
        .request
        .headers()
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");

    let uuid_str = path_params.id.to_string();

    context
        .process_manifest(&uuid_str, host)
        .await
        .map(HttpResponseOk)
        .map_err(|e| {
            if e.to_string().contains("No such file") {
                HttpError::for_not_found(None, format!("Dataset {} not found", uuid_str))
            } else {
                HttpError::for_internal_error(format!("Failed to process manifest: {}", e))
            }
        })
}

/** HEAD support for specific dataset manifest*/
#[endpoint {
    method = HEAD,
    path = "/datasets/{id}",
}]
async fn dataset_id_head(
    rqctx: RequestContext<DsapiContext>,
    path_params: DropPath<DsapiId>,
) -> Result<HttpResponseOk<Manifest>, HttpError> {
    let context = rqctx.context();
    let path_params = path_params.into_inner();
    let host = rqctx
        .request
        .headers()
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost");

    let uuid_str = path_params.id.to_string();

    context
        .process_manifest(&uuid_str, host)
        .await
        .map(HttpResponseOk)
        .map_err(|e| {
            if e.to_string().contains("No such file") {
                HttpError::for_not_found(None, format!("Dataset {} not found", uuid_str))
            } else {
                HttpError::for_internal_error(format!("Failed to process manifest: {}", e))
            }
        })
}

/** Serve dataset file*/
#[endpoint {
    method = GET,
    path = "/datasets/{id}/{path}",
    unpublished = true,
}]
async fn dataset_id_path(
    rqctx: RequestContext<DsapiContext>,
    path_params: DropPath<DsapiIdPath>,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let path_params = path_params.into_inner();
    let uuid_str = path_params.id.to_string();
    let file_path = context.serve_dir.join(&uuid_str).join(&path_params.path);

    // Security check: ensure the file is within the dataset directory
    let canonical_base = context
        .serve_dir
        .join(&uuid_str)
        .canonicalize()
        .map_err(|_| HttpError::for_not_found(None, "Dataset not found".to_string()))?;
    let canonical_file = file_path
        .canonicalize()
        .map_err(|_| HttpError::for_not_found(None, "File not found".to_string()))?;

    if !canonical_file.starts_with(&canonical_base) {
        return Err(HttpError::for_bad_request(
            None,
            "Invalid file path".to_string(),
        ));
    }

    let file = async_fs::File::open(&file_path).await.map_err(|e| {
        HttpError::for_bad_request(None, format!("failed to read file {:?}: {:#}", file_path, e))
    })?;

    let metadata = file.metadata().await.map_err(|e| {
        HttpError::for_internal_error(format!("Failed to get file metadata: {}", e))
    })?;

    let file_access = hyper_staticfile::vfs::TokioFileAccess::new(file);
    let file_stream = hyper_staticfile::util::FileBytesStream::new(file_access);
    let body = Body::wrap(hyper_staticfile::Body::Full(file_stream));

    // Derive the MIME type from the file name
    let content_type = mime_guess::from_path(&file_path)
        .first()
        .map_or_else(|| "application/octet-stream".to_string(), |m| m.to_string());

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", content_type)
        .header("Content-Length", metadata.len().to_string())
        .body(body)?)
}

/** HEAD support for dataset files*/
#[endpoint {
    method = HEAD,
    path = "/datasets/{id}/{path}",
    unpublished = true,
}]
async fn dataset_id_path_head(
    rqctx: RequestContext<DsapiContext>,
    path_params: DropPath<DsapiIdPath>,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let path_params = path_params.into_inner();
    let uuid_str = path_params.id.to_string();
    let file_path = context.serve_dir.join(&uuid_str).join(&path_params.path);

    // Security check: ensure the file is within the dataset directory
    let canonical_base = context
        .serve_dir
        .join(&uuid_str)
        .canonicalize()
        .map_err(|_| HttpError::for_not_found(None, "Dataset not found".to_string()))?;
    let canonical_file = file_path
        .canonicalize()
        .map_err(|_| HttpError::for_not_found(None, "File not found".to_string()))?;

    if !canonical_file.starts_with(&canonical_base) {
        return Err(HttpError::for_bad_request(
            None,
            "Invalid file path".to_string(),
        ));
    }

    let metadata = async_fs::metadata(&file_path)
        .await
        .map_err(|_| HttpError::for_not_found(None, "File not found".to_string()))?;

    // Derive the MIME type from the file name
    let content_type = mime_guess::from_path(&file_path)
        .first()
        .map_or_else(|| "application/octet-stream".to_string(), |m| m.to_string());

    let body = Body::empty();

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", content_type)
        .header("Content-Length", metadata.len().to_string())
        .body(body)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_process_manifest_url_generation() {
        let temp_dir = TempDir::new().unwrap();
        let uuid = "08d4292e-4fa2-11e2-852e-c3b213e7719c";
        let dataset_dir = temp_dir.path().join(uuid);
        fs::create_dir_all(&dataset_dir).await.unwrap();

        let manifest_content = json!({
            "uuid": uuid,
            "name": "test-dataset",
            "version": "1.0.0",
            "description": "Test dataset",
            "os": "smartos",
            "type": "zone-dataset",
            "urn": "test:test:test:1.0.0",
            "creator_name": "test",
            "creator_uuid": "550e8400-e29b-41d4-a716-446655440000",
            "published_at": "2023-01-01T00:00:00.000Z",
            "files": [
                {
                    "path": "test-file.zfs.bz2",
                    "sha1": "da39a3ee5e6b4b0d3255bfef95601890afd80709",
                    "size": 12345
                }
            ]
        });

        let manifest_path = dataset_dir.join("manifest.json");
        fs::write(&manifest_path, manifest_content.to_string()).await.unwrap();

        let config = Config {
            listen_port: json!(8876),
            prefix: "http://".to_string(),
            suffix: ":8876".to_string(),
            loglevel: "info".to_string(),
            serve_dir: Some(temp_dir.path().to_string_lossy().to_string()),
        };

        let context = DsapiContext::new(
            json!({}),
            config,
            temp_dir.path().to_path_buf(),
        );

        let result = context.process_manifest(uuid, "localhost").await;
        assert!(result.is_ok());

        let manifest = result.unwrap();
        assert_eq!(manifest.uuid.to_string(), uuid);
        assert_eq!(manifest.files.len(), 1);
        assert!(manifest.files[0].url.is_some());
        
        let expected_url = format!("http://localhost:8876/datasets/{}/test-file.zfs.bz2", uuid);
        assert_eq!(manifest.files[0].url.as_ref().unwrap(), &expected_url);
    }

    #[tokio::test]
    async fn test_process_manifest_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let uuid = "nonexistent-uuid";

        let config = Config {
            listen_port: json!(8876),
            prefix: "http://".to_string(),
            suffix: "".to_string(),
            loglevel: "info".to_string(),
            serve_dir: Some(temp_dir.path().to_string_lossy().to_string()),
        };

        let context = DsapiContext::new(
            json!({}),
            config,
            temp_dir.path().to_path_buf(),
        );

        let result = context.process_manifest(uuid, "localhost").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_all_dataset_uuids() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create test datasets
        let uuids = ["08d4292e-4fa2-11e2-852e-c3b213e7719c", "550e8400-e29b-41d4-a716-446655440000"];
        
        for uuid in &uuids {
            let dataset_dir = temp_dir.path().join(uuid);
            fs::create_dir_all(&dataset_dir).await.unwrap();
            
            let manifest_content = json!({
                "uuid": uuid,
                "name": "test",
                "version": "1.0.0",
                "description": "Test",
                "os": "smartos",
                "type": "zone-dataset",
                "urn": "test:test:test:1.0.0",
                "creator_name": "test",
                "creator_uuid": "550e8400-e29b-41d4-a716-446655440000",
                "published_at": "2023-01-01T00:00:00.000Z",
                "files": []
            });
            
            let manifest_path = dataset_dir.join("manifest.json");
            fs::write(&manifest_path, manifest_content.to_string()).await.unwrap();
        }

        // Create directory without manifest (should be ignored)
        let invalid_dir = temp_dir.path().join("invalid-dataset");
        fs::create_dir_all(&invalid_dir).await.unwrap();

        let config = Config {
            listen_port: json!(8876),
            prefix: "http://".to_string(),
            suffix: "".to_string(),
            loglevel: "info".to_string(),
            serve_dir: Some(temp_dir.path().to_string_lossy().to_string()),
        };

        let context = DsapiContext::new(
            json!({}),
            config,
            temp_dir.path().to_path_buf(),
        );

        let result = context.get_all_dataset_uuids().await;
        assert!(result.is_ok());

        let mut found_uuids = result.unwrap();
        found_uuids.sort();
        
        let mut expected_uuids: Vec<String> = uuids.iter().map(|s| s.to_string()).collect();
        expected_uuids.sort();
        
        assert_eq!(found_uuids, expected_uuids);
    }

    #[test]
    fn test_config_parsing() {
        // Test number port
        let config_json = r#"{"listen_port": 8080, "prefix": "http://", "suffix": "", "loglevel": "info"}"#;
        let config: Config = serde_json::from_str(config_json).unwrap();
        assert_eq!(config.listen_port, json!(8080));

        // Test string port  
        let config_json = r#"{"listen_port": "127.0.0.1:9000", "prefix": "http://", "suffix": "", "loglevel": "info"}"#;
        let config: Config = serde_json::from_str(config_json).unwrap();
        assert_eq!(config.listen_port, json!("127.0.0.1:9000"));
    }

    #[tokio::test]
    async fn test_path_traversal_protection() {
        let temp_dir = TempDir::new().unwrap();
        let uuid = "550e8400-e29b-41d4-a716-446655440000";
        let dataset_dir = temp_dir.path().join(uuid);
        fs::create_dir_all(&dataset_dir).await.unwrap();

        // Create a test file inside the dataset directory
        let test_file = dataset_dir.join("test.txt");
        fs::write(&test_file, "test content").await.unwrap();

        // Create a file outside the dataset directory that we shouldn't be able to access
        let outside_file = temp_dir.path().join("secret.txt");
        fs::write(&outside_file, "secret content").await.unwrap();

        let config = Config {
            listen_port: json!(8876),
            prefix: "http://".to_string(),
            suffix: "".to_string(),
            loglevel: "info".to_string(),
            serve_dir: Some(temp_dir.path().to_string_lossy().to_string()),
        };

        let context = DsapiContext::new(
            json!({}),
            config,
            temp_dir.path().to_path_buf(),
        );

        // Test that we can access a legitimate file
        let legitimate_path = context.serve_dir.join(uuid).join("test.txt");
        let canonical_base = context.serve_dir.join(uuid).canonicalize().unwrap();
        let canonical_file = legitimate_path.canonicalize().unwrap();
        assert!(canonical_file.starts_with(&canonical_base));

        // Test that path traversal is blocked (this would fail canonicalize due to file not existing in the expected location)
        let traversal_path = context.serve_dir.join(uuid).join("../secret.txt");
        // This should either fail canonicalize or fail the starts_with check
        let result = traversal_path.canonicalize();
        if let Ok(canonical_traversal) = result {
            assert!(!canonical_traversal.starts_with(&canonical_base));
        }
        // If canonicalize fails, that's also good - it means the path doesn't exist relative to the dataset
    }

    #[tokio::test]
    async fn test_ping_endpoint() {
        let temp_dir = TempDir::new().unwrap();
        
        let config = Config {
            listen_port: json!(0), // Let OS pick a port
            prefix: "http://".to_string(),
            suffix: "".to_string(),
            loglevel: "info".to_string(),
            serve_dir: Some(temp_dir.path().to_string_lossy().to_string()),
        };

        let config_logging = ConfigLogging::StderrTerminal {
            level: ConfigLoggingLevel::Info,
        };
        let log = config_logging.to_logger("test").unwrap();

        let mut api = ApiDescription::new();
        api.register(ping).unwrap();
        api.register(ping_head).unwrap();

        let api_description = api.openapi("test", semver::Version::new(0, 2, 0)).json().unwrap();
        let api_context = DsapiContext::new(api_description, config, temp_dir.path().to_path_buf());

        let config_dropshot = ConfigDropshot {
            bind_address: "127.0.0.1:0".parse().unwrap(),
            ..Default::default()
        };

        let server = ServerBuilder::new(api, api_context, log)
            .config(config_dropshot)
            .start()
            .unwrap();
        
        let local_addr = server.local_addr();
        
        // Test GET /ping
        let url = format!("http://{}/ping", local_addr);
        let client = reqwest::Client::new();
        let response = client.get(&url).send().await.unwrap();
        
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        let body: serde_json::Value = response.json().await.unwrap();
        assert_eq!(body["ping"], "pong");
        
        // Test HEAD /ping
        let response = client.head(&url).send().await.unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        
        server.close().await.unwrap();
    }
}
