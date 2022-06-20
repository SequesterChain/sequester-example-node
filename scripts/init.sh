#!/usr/bin/env bash
# This script is meant to be run on Unix/Linux based systems
set -e

echo "*** Initializing WASM build environment"

if [ -z $CI_PROJECT_NAME ] ; then
   rustup update nightly
   rustup update stable
fi

rustup default nightly
rustup target add wasm32-unknown-unknown --toolchain nightly

// ensure docker_run.sh is executable
chmod +x ./scripts/docker_run.sh
