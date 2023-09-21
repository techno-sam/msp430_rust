#!/usr/bin/sh
# don't need to specify `--test-threads 8` because tests run in parallel by default
cargo test -- --include-ignored
