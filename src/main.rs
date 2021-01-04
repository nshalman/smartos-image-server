// Copyright 2020 Nahum Shalman
/*!
 * Rust implementation of old smartos dsapi
 */

//use anyhow::{Result, bail, anyhow};
use getopts::Options;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use std::env;
use std::sync::Arc;
use std::vec::Vec;
use uuid::Uuid;

use dropshot::{
    ApiDescription,
    ConfigDropshot,
    ConfigLogging,
    ConfigLoggingLevel,
    endpoint,
    HttpError,
    HttpResponseOk,
    HttpServer,
    Path,
    RequestContext,
};


#[tokio::main]
async fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    let program = args[0].clone();
    let mut opts = Options::new();
    opts.optflag("h", "help", "print this help menu");
    opts.optopt("l", "listen", "listen on address:port", "LISTEN");
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => { m }
        Err(f) => { panic!(f.to_string()) }
    };
    if matches.opt_present("h") {
        let brief = format!("Usage: {} [options]", program);
        print!("{}", opts.usage(&brief));
        std::process::exit(0);
    }

    /*
     * We must specify a configuration with a bind address.  We'll use 127.0.0.1
     * since it's available and won't expose this server outside the host.  We
     * request port 0, which allows the operating system to pick any available
     * port.
     */
    let config_dropshot: ConfigDropshot = Default::default();

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
    api.register(ping).unwrap();
    api.register(datasets).unwrap();
    api.register(dataset_id).unwrap();
    api.register(dataset_id_path).unwrap();

    /*
     * The functions that implement our API endpoints will share this context.
     */
    let api_context = DsapiContext::new();

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

    /*
     * Set up the server.
     */
    let mut server = HttpServer::new(&config_dropshot, api, api_context, &log)
        .map_err(|error| format!("failed to create server: {}", error))?;
    let server_task = server.run();

    /*
     * Wait for the server to stop.  Note that there's not any code to shut down
     * this server, so we should never get past this point.
     */
    server.wait_for_shutdown(server_task).await
}

/**
 * Application-specific example context (state shared by handler functions)
 */
struct DsapiContext {
}

impl DsapiContext {
    /**
     * Return a new DsapiContext.
     */
    pub fn new() -> Arc<DsapiContext> {
        Arc::new(DsapiContext {
        })
    }
}

/*
 * HTTP API interface
 */

/** pong is the response to a successful ping*/
#[derive(Deserialize, Serialize, JsonSchema)]
struct Pong {
    ping: String,
}

/** Respond to a ping with pong*/
#[endpoint {
    method = GET,
    path = "/ping",
}]
async fn ping(
    _rqctx: Arc<RequestContext>,
) -> Result<HttpResponseOk<Pong>, HttpError> {
    let pong = "pong".to_string();
    Ok(HttpResponseOk(Pong {
        ping: pong,
    }))
}

/** Represents the files for a dataset in dsapi */
#[derive(Serialize, JsonSchema)]
struct Files {
    path: String,
    sha1: String,
    size: u64,
    url: Option<String>,
}

/** Represents a dataset in dsapi */
#[derive(Serialize, JsonSchema)]
struct Manifest {
    uuid: Uuid,
    name: String,
    version: String,
    description: String,

    os: String,
    r#type: String,
    platform_type: String,
    cloud_name: String,
    urn: String,
    
    creator_name: String,
    creator_uuid: Uuid,
    vendor_uuid: Uuid,

    created_at: String,
    updated_at: String,
    published_at: String,

    files: Files,
}

#[derive(Deserialize, JsonSchema)]
struct DsapiId {
    id: String,
}

#[derive(Deserialize, JsonSchema)]
struct DsapiIdPath {
    id: String,
    path: String,
}

/** Get all datasets on this server*/
#[endpoint {
    method = GET,
    path = "/datasets",
}]
async fn datasets(
    _rqctx: Arc<RequestContext>,
) -> Result<HttpResponseOk<Option<Vec<Manifest>>>, HttpError> {
    Ok(HttpResponseOk(None))
}

/** Get all datasets on this server*/
#[endpoint {
    method = GET,
    path = "/dataset/{id}",
}]
async fn dataset_id(
    _rqctx: Arc<RequestContext>,
    path_params: Path<DsapiId>,
) -> Result<HttpResponseOk<String>, HttpError> {
//) -> Result<HttpResponseOk<Option<Manifest>>, HttpError> {
    let path_params = path_params.into_inner();
    Ok(HttpResponseOk(path_params.id))
}

/** Get all datasets on this server*/
#[endpoint {
    method = GET,
    path = "/dataset/{id}/{path}",
}]
async fn dataset_id_path(
    _rqctx: Arc<RequestContext>,
    path_params: Path<DsapiIdPath>,
//) -> Result<HttpResponseOk<Option<Vec<Manifest>>>, HttpError> {
) -> Result<HttpResponseOk<String>, HttpError> {
    let path_params = path_params.into_inner();
    let mut reply: String = path_params.id;
    reply.push_str(&path_params.path);
    Ok(HttpResponseOk(reply))
}
