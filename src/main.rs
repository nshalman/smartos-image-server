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
    endpoint, ApiDescription, Body, ConfigLogging, ConfigLoggingLevel, HttpError, HttpResponseOk,
    Path as DropPath, RequestContext, ServerBuilder,
};
use http::{Response, StatusCode};

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

    let _bind = matches
        .opt_str("l")
        .unwrap_or_else(|| match &config.listen_port {
            serde_json::Value::Number(n) => format!("0.0.0.0:{}", n),
            serde_json::Value::String(s) => s.clone(),
            _ => String::from("0.0.0.0:8876"),
        });

    // TODO: Use bind address from config instead of default

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

    let server = ServerBuilder::new(api, api_context, log)
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

    // TODO: Optimize to use streaming instead of loading entire file into memory
    // Current implementation loads the full file for simplicity, but could be
    // optimized using ReaderStream + http-body-util for large files
    let file_content = async_fs::read(&file_path)
        .await
        .map_err(|_| HttpError::for_not_found(None, "File not found".to_string()))?;

    let body = Body::with_content(file_content.clone());

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/octet-stream")
        .header("Content-Length", file_content.len().to_string())
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

    let body = Body::empty();

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("Content-Type", "application/octet-stream")
        .header("Content-Length", metadata.len().to_string())
        .body(body)?)
}
