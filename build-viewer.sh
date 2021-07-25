#!/usr/bin/env sh

# This script is a workaround for the lack of workspaces support in wasm-pack.
# See https://github.com/rustwasm/wasm-pack/issues/642 for details.

if [ -z $(which cargo) ]; then
    PATH="$HOME/.cargo/bin:$PATH"
fi

PROJECT_PATH=$(dirname "$0")

cd $PROJECT_PATH/viewer
wasm-pack build --target web "$@"
exit $?
