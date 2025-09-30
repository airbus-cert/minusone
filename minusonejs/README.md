# WASM build for minusone

Run minusone in your browser!

## Requirements

- devenv

If you auto load devenv shell with .envrc, you can just run the commands of this Readme in your shell.
Otherwise, you might start a devenv shell with:
```bash
devenv shell
```

## Build step

Just use the `justfile` recipe:
```bash
just build
```

The project is build with wasm32-wasip2, then transpiled to JS thanks to `jco`.

## Tests

To test in the browser, you can use:
```bash
just serve
```

It will run locally a webapp that execute minueone commands in the console.
