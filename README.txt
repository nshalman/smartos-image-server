smartos-image-server
====================

bare minimum image server for SmartOS

Requires nodejs>=0.8 and node-restify (npm install restify)

structure:
smartos-image-server/
	server.js
	config.json
	<uuid>/
		manifest.json
		<zfs-stream-file>.<extension>

There is some basic error checking, but it could probably be improved.
Patches Welcome!!

To install in a SmartOS Zone:

pkgin in nodejs scmgit
git clone https://github.com/nshalman/smartos-image-server
cd smartos-image-server
npm install restify
./server.js

then copy in a bunch of manifests and compressed zfs send streams in the right hierarchy
and add http://<your_ip>/datasets to /var/db/imgadm/sources.list
imgadm update
imgadm avail
 etc.

Someone else has written a tool to make it easy to generate manifest files:
https://github.com/project-fifo/nomnom/tree/master/tools

Behind nginx
============
These examples taken straight from my running server

config.json:
{
	"listen_port": "/var/tmp/image-server.sock",
	"prefix": "http://",
	"suffix": "",
	"loglevel": "info"
}

nginx config snippet:
    server {
        listen       80;
        server_name  datasets.shalman.org;
        location / {
            proxy_set_header Host $host;
            proxy_pass http://unix:/var/tmp/image-server.sock:;
        }
    }
