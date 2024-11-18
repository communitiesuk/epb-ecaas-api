set-env-vars:
	cd hem && export HEM_SOFTWARE_VERSION=`git rev-parse HEAD` && cd .. && echo "hem software version set to $$HEM_SOFTWARE_VERSION"

build-lambda-binary: set-env-vars
	cargo lambda build --arm64 -r

build-lambda-zip: set-env-vars
	cargo lambda build --arm64 -r --output-format zip

watch-lambda:
	cargo lambda watch --release


invoke-lambda:
	cargo lambda invoke --data-file examples/api_gateway_demo_fhs_bottomup.json
