#!/bin/sh
rm ./sol/bin/*.json
solc --optimize-runs=10000 --output-dir=./sol/bin/ --overwrite --combined-json bin,bin-runtime ./sol/*.sol
