#!/bin/env bash

set -e

pushd async-stream
cargo readme > README.md
popd
cp async-stream/README.md ./
