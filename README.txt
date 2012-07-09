smartos-image-server
====================

bare minimum image server for SmartOS

Requires node-restify (npm install restify)

structure:
smartos-image-server/
	server.js
	<uuid>/
		manifest.json
		<zfs-stream-file>.<extension>

There is almost no error checking,
Incorrectly formatted manifest files might bring down the server.
Patches Welcome!!
