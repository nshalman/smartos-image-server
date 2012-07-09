#!/usr/bin/env node

var restify = require('restify');
var    path = require('path');
var      fs = require('fs');

function alldatasets(req, res, next) {
	var everything = [];
	fs.readdir(process.cwd(), function (err, dirlist) {
		if (err) {
			res.send(500, 'Internal Server Error');
			return;
		}
		else {
			for (entry in dirlist) {
				console.log("dirlist[entry]: " + dirlist[entry]);
				if (fs.existsSync(dirlist[entry] + '/manifest.json')) {
					everything.push(require('./' + dirlist[entry] + '/manifest'));
				};
			};
			res.send(everything);
		};
	});
}

function manifest(req, res, next) {
	var filename = './' + req.params.id + '/manifest';
	res.send(require(filename));
}

function imagefile(req, res, next) {
	var filename = path.join(process.cwd(), req.params.id + '/' + req.params.path);
	fs.exists(filename, function (exists) {
		if (!exists) {
			res.send(404, '404 Not Found');
			return;
		} else {
			var stream = fs.createReadStream(filename, { bufferSize: 64 * 1024 });
			stream.pipe(res);
		}
	});
}

function ping(req, res, next) {
	res.send({"ping":"pong"});
}

function setup_routes(server, route, handler) {
	server.get(route, handler);
	server.head(route, handler);
}

var server = restify.createServer();

setup_routes(server, '/datasets', alldatasets);
setup_routes(server, '/datasets/:id', manifest);
setup_routes(server, '/datasets/:id/:path', imagefile);
setup_routes(server, '/ping', ping);

server.listen(8080, function() {
  console.log('%s listening at %s', server.name, server.url);
});
