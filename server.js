#!/usr/bin/env node

var restify = require('restify');
var    path = require('path');
var      fs = require('fs');

/* read in configuration */
var config = require('./config');
var serve_dir;

if (config["serve_dir"]){
	serve_dir = config["serve_dir"];
}
else {
	serve_dir = process.cwd();
}

/*
 * We autogenerate the url property for each file to ensure that it is correct.
 * This overrides whatever may have been place into the manifest.
 * Adjust config.json if these urls are coming out wrong
 */
function process_manifest(req, uuid) {
	var manifest;
	try {
		manifest = require('./' + uuid + '/manifest');
		var url_prefix = config.prefix + req.header('Host') + config.suffix + "/datasets/" + uuid + "/";
		for (entry in manifest.files) {
			manifest.files[entry].url = url_prefix + manifest.files[entry].path
		};
		return manifest;
	}
	catch(err) {
		req.log.error("Failed to parse manifest for " + uuid + " error: " + err);
		return false;
	}
}

/*
 * Smoosh together all manifests into an array and return it
 */
function alldatasets(req, res, next) {
	var everything = [];
	fs.readdir(serve_dir, function (err, dirlist) {
		if (err) {
			res.send(500, 'Internal Server Error');
			return;
		}
		else {
			for (entry in dirlist) {
				if (fs.existsSync(dirlist[entry] + '/manifest.json')) {
					var manifest = process_manifest(req, dirlist[entry]);
					if ( manifest ) {
						everything.push(manifest);
					};
				};
			};
			res.send(everything);
			req.log.info("served up /datasets");
		};
	});
	return next();
}

/*
 * Process and return the requested manifest
 */
function manifest(req, res, next) {
	var manifest = process_manifest(req, req.params.id);
	if (!manifest) {
		res.send(404, '404 Not Found');
		return next();
	}
	res.send(process_manifest(req, req.params.id));
	req.log.info("served up /datasets/" + req.params.id);
	return next();
}

/*
 * Serve up the requested image file
 */
function imagefile(req, res, next) {
	var filename = path.join(serve_dir, req.params.id + '/' + req.params.path);
	req.log.info("serving up /datasets/" + req.params.id + '/' + req.params.path);
	fs.exists(filename, function (exists) {
		if (!exists) {
			res.send(404, '404 Not Found');
			return;
		} else {
			res.header("Content-Type", "application/octet-stream");
			var stream = fs.createReadStream(filename, { bufferSize: 64 * 1024 });
			stream.pipe(res);
		}
	});
	return next();
}

/*
 * Implement ping
 */
function ping(req, res, next) {
	res.send({"ping":"pong"});
	req.log.info("served up /ping");
	return next();
}

/*
 * Serve /
 */
function slash(req, res, next) {
	var accept = req.header("Accept");
	if (accept && (accept.search("application/xhtml+xml") != -1 || accept.search("text/html") != -1)) {
		var stream = fs.createReadStream(serve_dir + "/index.html", { bufferSize: 64 * 1024 });
		stream.pipe(res);
	} else {
		res.header("Content-Type", "application/json");
		var stream = fs.createReadStream(serve_dir + "/index.json", { bufferSize: 64 * 1024 });
		stream.pipe(res);
	}
	req.log.info("served up /");
	return next();
}

/*
 * route creation helper
 */
function setup_routes(server, route, handler) {
	server.get(route, handler);
	server.head(route, handler);
}

/* node restify rocks! */
var server = restify.createServer();

setup_routes(server, '/datasets', alldatasets);
setup_routes(server, '/datasets/:id', manifest);
setup_routes(server, '/datasets/:id/:path', imagefile);
setup_routes(server, '/ping', ping);
setup_routes(server, '/', slash);

server.log.level(config.loglevel);
server.listen(config.listen_port, function() {
	if ( typeof config.listen_port == "string") {
		console.log('listening on unix socket: %s', config.listen_port);
	}
	else {
		console.log('%s listening at %s', server.name, server.url);
	}
	console.log('serving from %s', serve_dir);
});
