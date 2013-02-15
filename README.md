smartos-image-server
====================

bare minimum image server for SmartOS

# Installing in a SmartOS zone

```
pkgin in nodejs scmgit
git clone https://github.com/nshalman/smartos-image-server
cd smartos-image-server
make
```

# Configuration

After installation is complete, you should update config.json for 
your environment. At the very least you should set an image-creator 
name for your organization, if you intend to use the supplied 
import-image script to publish images.

## Running behind ngnix

config.json:

``` json
{
	"listen_port": "/var/tmp/image-server.sock",
	"prefix": "http://",
	"suffix": "",
	"loglevel": "info",
  "image-creator": "internal",
  "image-creator-uuid": "...",
  "image-vendor-uuid": "..."
}
```

nginx config snippet:

```
    server {
        listen       80;
        server_name  datasets.shalman.org;
        location / {
            proxy_set_header Host $host;
            proxy_pass http://unix:/var/tmp/image-server.sock:;
        }
    }
```

# Enabling the image server

After installation is complete, you can import the SMF manifest 
and enable the service.

```
svccfg import image-server.smf.xml
svcadm enable image-server
```

# Serving images

To serve images, the "serve_dir" (configured in config.json, defaults 
to smartos-image-server directory) needs to be populated with manifests and 
zfs send streams in the proper hierarchy (see structure below). 

Once populated, add http://<image_server_ip_or_hostname>/datasets to /var/db/imgadm/sources.list

Then:

```
imgadm update
imgadm avail
```

## Publishing images

There's a few options for publishing images to the image server:

* Build manifests manually (http://wiki.smartos.org/display/DOC/Managing+Images#ManagingImages-ImageManifests) 
and populate "serve_dir" according to structure below.
* Someone else has written a tool to make it easy to generate manifest files:
https://github.com/project-fifo/nomnom/tree/master/tools
* Or use the included import-image script via SSH from the global zone 

## Using import-image

The long way, from the global zone:

```
zlogin a48b615e-76fb-11e2-9502-6b130bf7f4ff 
sm-prepare-image
# exit / wait for zone to stop
zfs snapshot zones/a48b615e-76fb-11e2-9502-6b130bf7f4ff@image
zfs send zones/a48b615e-76fb-11e2-9502-6b130bf7f4ff@image | gzip | ssh datasets.yourdomain.local '/path/to/import-image orgbase 1.0.0 "Base image for our organization"'
imgadm update
imgadm avail
```

Alternatively, there's a `mkimg` script included that can be copied to the 
global zone (/opt/local/bin), and used to make the above process simpler. To 
do so, first edit the `mkimg` script and set IMGSERVER, IMPORTPATH, and LOGIN 
to proper values for your environment. Once configured you can now publish 
images from the global zone by doing:

`mkimg <uuid> <name> <version> <description>`

Example:
```
mkimg a48b615e-76fb-11e2-9502-6b130bf7f4ff orgbase 1.0.0 "Base image for our organization"
```

# Structure

By default, images are stored under smartos-image-server. This can
be changed in config.json.

```
smartos-image-server/
	server.js
	config.json
  import-image
	<uuid>/
		manifest.json
		<zfs-stream-file>.<extension>
```

# Requirements

* nodejs>=0.8
* node-restify 

# Final thoughts

There is some basic error checking, but it could probably be improved.

Patches Welcome!!

