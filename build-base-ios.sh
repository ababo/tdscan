#!/usr/bin/env sh

# This script is mainly to be called by `Fitsme Scanner` Xcode-project that
# links with `base` static library for `aarch64-apple-ios` target.

if [ -z $(which cargo) ]; then
    PATH="$HOME/.cargo/bin:$PATH"
fi

PROJECT_PATH=$(dirname "$0")

cd $PROJECT_PATH/base
cargo build -p base --target aarch64-apple-ios --release
exit $?
