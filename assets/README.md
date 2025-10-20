Just some notes on how I created the animated SVG for future reference.

```bash
SHELL=/bin/fish asciinema rec -f asciicast-v2 -t "Bieglers TimeTracker Demo" --overwrite usage-demo.cast
```

Specifically using the older v2 format via `-f` so that `svg-term-cli` can pick it up for conversion.

```bash
npx svg-term-cli < usage-demo.cast > usage-demo.svg
```

