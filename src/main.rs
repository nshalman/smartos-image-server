// Copyright 2020 Nahum Shalman
/*!
 * Rust implementation of old smartos dsapi
 */

//use anyhow::{Result, bail, anyhow};
use getopts::Options;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::any::Any;
use std::env;
use std::sync::Arc;
use std::vec::Vec;
use uuid::Uuid;

use dropshot::{
    endpoint, ApiDescription, ConfigDropshot, ConfigLogging, ConfigLoggingLevel, HttpError,
    HttpResponseOk, HttpServer, Path, RequestContext,
};

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
            panic!(f.to_string())
        }
    };
    if matches.opt_present("h") {
        let brief = format!("Usage: {} [options]", program);
        print!("{}", opts.usage(&brief));
        std::process::exit(0);
    }
    let bind = matches
        .opt_str("l")
        .unwrap_or_else(|| String::from("0.0.0.0:8876"));

    let config_dropshot = ConfigDropshot {
        bind_address: bind.parse().unwrap(),
        ..Default::default()
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
    api.register(slash).unwrap();
    api.register(ping).unwrap();
    api.register(datasets).unwrap();
    api.register(dataset_id).unwrap();
    api.register(dataset_id_path).unwrap();

    /*
     * The functions that implement our API endpoints will share this context.
     */
    let api_description = api
        .openapi("dsapi", "")
        .json()
        .map_err(|e| e.to_string())?;
        //.to_string();
    let api_context = DsapiContext::new(api_description);

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

    let mut server = HttpServer::new(&config_dropshot, api, api_context, &log)
        .map_err(|error| format!("failed to create server: {}", error))?;
    let server_task = server.run();

    server.wait_for_shutdown(server_task).await
}

/**
 * Application-specific example context (state shared by handler functions)
 */
struct DsapiContext {
    api: Value,
}

impl DsapiContext {
    /**
     * Return a new DsapiContext.
     */
    pub fn new(a: Value) -> Arc<DsapiContext> {
        Arc::new(DsapiContext { api: a })
    }

    /**
     * Given `rqctx` (which is provided by Dropshot to all HTTP handler
     * functions), return our application-specific context.
     */
    pub fn from_rqctx(rqctx: &Arc<RequestContext>) -> Arc<DsapiContext> {
        let ctx: Arc<dyn Any + Send + Sync + 'static> = Arc::clone(&rqctx.server.private);
        ctx.downcast::<DsapiContext>()
            .expect("wrong type for private data")
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
async fn slash(rqctx: Arc<RequestContext>) -> Result<HttpResponseOk<String>, HttpError> {
    let context = DsapiContext::from_rqctx(&rqctx);
    Ok(HttpResponseOk(context.api.to_string()))
}

/** Test Function*/
#[endpoint {
    method = GET,
    path = "/test",
}]
async fn testme(rqctx: Arc<RequestContext>) -> Result<HttpResponseOk<String>, HttpError> {
    //info!(rqctx.log, "Hello There {:?}", &rqctx.request.get_mut());
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
async fn ping(_rqctx: Arc<RequestContext>) -> Result<HttpResponseOk<Ping>, HttpError> {
    let pong = "pong".to_string();
    Ok(HttpResponseOk(Ping { ping: pong }))
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
    id: Uuid,
}

#[derive(Deserialize, JsonSchema)]
struct DsapiIdPath {
    id: Uuid,
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
    Ok(HttpResponseOk(path_params.id.to_string()))
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
    let mut reply: String = path_params.id.to_string();
    reply.push_str(&path_params.path);
    Ok(HttpResponseOk(reply))
}
