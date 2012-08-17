smartos-image-server
====================

bare minimum image server for SmartOS

Requires nodejs>=0.8 and node-restify (npm install restify)

structure:
smartos-image-server/
	server.js
	<uuid>/
		manifest.json
		<zfs-stream-file>.<extension>

There is almost no error checking,
Incorrectly formatted manifest files might bring down the server.
Patches Welcome!!

To install in a SmartOS Zone:

pkgin in nodejs scmgit
git clone https://github.com/nshalman/smartos-image-server
cd smartos-image-server
npm install restify
./server.js

then copy in a bunch of manifests and compressed zfs send streams in the right hierarchy
and add http://<your_ip>:8080/datasets to /var/db/imgadm/sources.list
imgadm update
imgadm avail
 etc.

Someone else has written a tool to make it easy to generate manifest files:
https://github.com/project-fifo/nomnom/tree/master/tools
