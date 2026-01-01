# C Example

Example usage of Lolite in C. It uses the binaries built via cargo. Note that
you may want to consume the library binaries in a different way in your own
project.

## Requirements

- Have CMake installed and in path.
- Build Lolite in release using `cargo build --release`.

## Building

- Run `cmake -S . -B build` to configure the projeect.
- Run `cmake --build build` to build the project.
- Then you can run the generated binary, e.g. `build/Debug/c_usage.exe`.