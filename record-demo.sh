#!/usr/bin/env bash

stty cols 120 rows 34
asciinema rec demo.cast \
	--overwrite \
	--title "CSV to Parquet (Rust)" \
	--cols 120 \
	--rows 34 \
	--command "bash demo.sh"

# sed -i 's/"cols":[0-9]\+/"cols":120/; s/"rows":[0-9]\+/"rows":34/' demo.cast
agg --font-size 18 --theme monokai demo.cast demo.gif
gifsicle --lossy=50 -k 128 -O3 demo.gif -o demo-opt.gif

mv -fv demo-opt.gif demo.gif
