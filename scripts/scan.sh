#!/bin/bash
docker run --user $(id -u) -v "$(pwd)/.dastardly":/.dastardly:rw \
    -e DASTARDLY_TARGET_URL=https://bitbytelab.io \
    -e DASTARDLY_OUTPUT_FILE=/.dastardly/dastardly-report.xml \
    public.ecr.aws/portswigger/dastardly:latest
