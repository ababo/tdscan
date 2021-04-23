#!/bin/bash

# This script is a workaround for the lack of workspaces support in wasm-pack.
# See https://github.com/rustwasm/wasm-pack/issues/642 for details.

PROJECT_PATH=$(dirname "$0")

cd $PROJECT_PATH/viewer
wasm-pack build --target web "$@"
STATUS=$?
cd ..
exit $STATUS
