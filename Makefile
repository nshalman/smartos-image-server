# Makefile SmartOS Image Server
#
#
IMAGE_CREATOR_UUID=`uuid`
IMAGE_VENDOR_UUID=`uuid`
USER=`whoami`
GROUP=`groups |head -n1`
SCRIPT_DIR=`pwd`
NODE_PATH=`which node`

all: npm config smf info

npm:
	npm install
 
config:
	@cat config.json | sed -e "s/\"image-creator-uuid\": \".*\"/\"image-creator-uuid\": \"${IMAGE_CREATOR_UUID}\"/" | sed -e "s/\"image-vendor-uuid\": \".*\"/\"image-vendor-uuid\": \"${IMAGE_VENDOR_UUID}\"/" | cat > config.new.json
	@rm config.json > /dev/null 2>&1
	@mv config.new.json config.json

smf:
	@echo '<?xml version="1.0"?>' > image-server.smf.xml
	@echo '<!DOCTYPE service_bundle SYSTEM "/usr/share/lib/xml/dtd/service_bundle.dtd.1">' >> image-server.smf.xml
	@echo '	<service_bundle type="manifest" name="image-server">' >> image-server.smf.xml
	@echo '	<service name="site/image-server" type="service" version="1">' >> image-server.smf.xml
	@echo '		<create_default_instance enabled="false"/>' >> image-server.smf.xml
	@echo '			<single_instance/>' >> image-server.smf.xml
	@echo '			<dependency name="network" grouping="require_all" restart_on="error" type="service">' >> image-server.smf.xml
	@echo '				<service_fmri value="svc:/milestone/network:default"/>' >> image-server.smf.xml
	@echo '			</dependency>' >> image-server.smf.xml
	@echo '			<dependency name="filesystem" grouping="require_all" restart_on="error" type="service">' >> image-server.smf.xml
	@echo '				<service_fmri value="svc:/system/filesystem/local"/>' >> image-server.smf.xml
	@echo '			</dependency>' >> image-server.smf.xml
	@echo "			<method_context working_directory=\"${SCRIPT_DIR}\">" >> image-server.smf.xml
	@echo "				<method_credential user=\"${USER}\" group=\"${GROUP}\" privileges=\"basic,net_privaddr\" />" >> image-server.smf.xml
	@echo '			</method_context>' >> image-server.smf.xml
	@echo "			<exec_method type=\"method\" name=\"start\" exec=\"${NODE_PATH} ${SCRIPT_DIR}/server.js\" timeout_seconds=\"60\"/>" >> image-server.smf.xml
	@echo '			<exec_method type="method" name="stop" exec=":kill" timeout_seconds="60"/>' >> image-server.smf.xml
	@echo '			<property_group name="startd" type="framework">' >> image-server.smf.xml
	@echo '				<propval name="duration" type="astring" value="child"/>' >> image-server.smf.xml
	@echo '				<propval name="ignore_error" type="astring" value="core,signal"/>' >> image-server.smf.xml
	@echo '			</property_group>' >> image-server.smf.xml
	@echo '			<property_group name="application" type="application">' >> image-server.smf.xml
	@echo '			</property_group>' >> image-server.smf.xml
	@echo '			<stability value="Evolving"/>' >> image-server.smf.xml
	@echo '			<template>' >> image-server.smf.xml
	@echo '				<common_name>' >> image-server.smf.xml
	@echo '					<loctext xml:lang="C">' >> image-server.smf.xml
	@echo '						SmartOS Image Server' >> image-server.smf.xml
	@echo '					</loctext>' >> image-server.smf.xml
	@echo '				</common_name>' >> image-server.smf.xml
	@echo '			</template>' >> image-server.smf.xml
	@echo '		</service>' >> image-server.smf.xml
	@echo '</service_bundle>' >> image-server.smf.xml

info:
	@echo ""
	@echo "--------------------------------------------------------------------------"
	@echo "Creator and Vendor UUIDs generated for config.json. You should change"
	@echo "image-creator in config.json to something more meaningful than 'internal'."
	@echo ""
	@echo "SMF Manifest generated. You can import it now by doing (as root):"
	@echo "  svccfg import image-server.smf.xml"
	@echo ""
	@echo "After importing the SMF manifest and updating config.json, you can"
	@echo "enable the image server by issuing the command (as root):"
	@echo "  svcadm enable image-server"
	@echo ""
	@echo "If you intend to use the [mkimg] script in your global zone, you will need"
	@echo "edit it and change a few settings near the top for your environment."
	@echo ""
	@echo "Please report any issues at the following location:"
	@echo "  https://github.com/nshalman/smartos-image-server/issues"
	@echo "--------------------------------------------------------------------------"
	@echo ""

clean:
	@cat config.json | sed -e "s/\"image-creator-uuid\": \".*\"/\"image-creator-uuid\": \"\"/" | sed -e "s/\"image-vendor-uuid\": \".*\"/\"image-vendor-uuid\": \"\"/" | cat > config.new.json
	@rm config.json > /dev/null 2>&1
	@mv config.new.json config.json
	@rm -rf node_modules > /dev/null 2>&1
	@rm image-server.smf.xml > /dev/null 2>&1
