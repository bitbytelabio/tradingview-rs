docker run --user $(id -u) -v "$(pwd)/.dastardly":/.dastardly:rw \
    -e DASTARDLY_TARGET_URL=https://www.tekcent.com/umbraco \
    -e DASTARDLY_OUTPUT_FILE=/.dastardly/dastardly-report.xml \
    public.ecr.aws/portswigger/dastardly:latest
