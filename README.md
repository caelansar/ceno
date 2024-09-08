# CENO
CENO is an incomplete imitation of [Deno](https://deno.com/), providing a runtime for JavaScript and TypeScript with a focus on simplicity.

## Installation
Releases are available on [GitHub](https://github.com/ceno-lang/ceno/releases).
Or you can manually install it by running:
```bash
cargo install ceno
```

## Usage
CENO provides three main commands:

### Initialize a new project
```bash
ceno init
```
This command initializes a new CENO project in the current directory or creates a new directory if specified. It sets up the necessary files and configuration for your project.

### Build your project
```bash
ceno build
```
This command builds your project, compiling TypeScript to JavaScript and generating a static binary file.

### Run your project
```bash
ceno run
```
This command runs your CENO project, starting the server and listening for requests.

## Configuration
CENO uses a config.yml file for project configuration. You can specify routes and other settings in this file.
```yaml
name: my-project
routes:
  /api/hello:
    - method: GET
      handler: hello
```

## Development
CENO is built with Rust and uses various crates for its functionality. The project structure includes:
- [ceno](./ceno): The main CLI application
- [ceno-server](./ceno-server): The runtime server implementation
- [ceno-macros](./ceno-macros): Custom derive macros for the project
- [bundler](./bundler): A module for bundling JavaScript and TypeScript code
