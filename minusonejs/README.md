# WASM build for minusone

Run minusone in your browser!

## Requirements

- devenv

## Build step

Just use the `justfile` recipe:

```bash
devenv shell  # or load automatically it with .envrc
just pack
```

The project is build with wasm32-wasip2, then transpiled to JS thanks to `jco`.

## Tests

To test in the browser, you can use:
```bash
just serve
```

It will run locally a webapp that execute minueone commands in the console.
