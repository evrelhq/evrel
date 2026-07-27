# Evrel

Evrel is a JavaScript and TypeScript compiler written in Rust.

This repository contains the compiler's JavaScript pipeline:

- `evrel-ir` defines the compiler intermediate representation.
- `evrel-frontend` parses and lowers JavaScript and TypeScript into the IR.
- `evrel-codegen-js` plans and emits JavaScript from the IR.
- `evrel-compiler` exposes the end-to-end compiler API.
- `evrel-cli` provides the `evrel` command-line interface.
- `evrel-node` provides the `@evrel/compiler` Node.js binding.

## Development

The workspace uses the Rust toolchain declared in `rust-toolchain.toml`.

```sh
cargo test --workspace
```

The Node.js binding additionally requires Node.js 24 and pnpm:

```sh
pnpm --dir crates/evrel-node install --frozen-lockfile
pnpm --dir crates/evrel-node test
```

## Project status

Evrel is under active development. This repository is currently published for
inspection and evaluation. Public contributions are not being accepted yet.

## Copyright

Copyright © Evrel. All rights reserved. See `COPYRIGHT`.
