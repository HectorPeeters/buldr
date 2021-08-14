# Buldr

Buldr is a very simple build system written in Rust. It was built as a more userfriendly replacement to CMAKE and took a lot of inspiration from Premake5.

**WARNING**: This project is a proof of concept and the code is extremely ugly and slow at the moment. Don't take this as an example for any other projects.

## Usage

Start by creating a project in the current directory.

```shell
buldr create
```

This will create a `build.toml` file which contains all the build settings. A full example can be found at the end of the README.

Building the project is as simple as calling executing `buldr` in the directory that contains the `build.toml` file.

```shell
buldr
```

You can build a specific project by adding the name at the end.

```shell
buldr example-project-name
```

Cleaning all the build artifacts can be done like so:

```shell
buldr clean
```

And generating a `compile_commands.json` file for editor support is as easy as running:

```shell
buldr compile_commands
```

## Install

Building and installing this project can be done using Cargo:

```shell
cargo install --path .
```

## Features

- Build C/C++ projects
- Determine dependency order
- Clean build artifacts
- Generate compile_commands.json

## Planned Features

- Platform specific compilation or platform define flags
- Code actions (e.g. format, ...)
- Recompilation on header file change

## Full Example

This is an example configuration which builds my [OpenGL Premake boilerplate](https://github.com/HectorPeeters/opengl_premake_boilerplate).

```toml
[config]
compiler = "clang"
compiler_opts = ["-Wall"]
linker = "clang"
linker_opts = []
packer = "ar"
packer_opts = []
bin = "bin/"
obj = "obj/"

[[project]]
name = "opengl"
kind = "executable"
src = ["src/"]
include = [
    "include/",
    "libs/glad/include/",
    "libs/glfw/include/",
    "libs/glm/",
    "libs/imgui/",
    "libs/imgui/examples",
]
links = [
    "stdc++",
    "m",
    "dl",
    "pthread",
]
depends = ["glfw", "glad", "glm", "imgui"]
default = true

[[project]]
name = "glfw"
kind = "library"
src = [
    "libs/glfw/src/context.c",
    "libs/glfw/src/init.c",
    "libs/glfw/src/input.c",
    "libs/glfw/src/monitor.c",
    "libs/glfw/src/vulkan.c",
    "libs/glfw/src/window.c",
    "libs/glfw/src/x11_init.c",
    "libs/glfw/src/x11_monitor.c",
    "libs/glfw/src/x11_window.c",
    "libs/glfw/src/xkb_unicode.c",
    "libs/glfw/src/posix_time.c",
    "libs/glfw/src/posix_thread.c",
    "libs/glfw/src/glx_context.c",
    "libs/glfw/src/egl_context.c",
    "libs/glfw/src/osmesa_context.c",
    "libs/glfw/src/linux_joystick.c",
]
include = ["libs/glfw/src/glfw_config.h", "libs/glfw/include/"]
links = ["dl"]
defines = ["_GLFW_X11"]

[[project]]
name = "glad"
kind = "library"
src = ["libs/glad/src/glad.c"]
include = ["libs/glad/include/"]
links = ["dl"]

[[project]]
name = "imgui"
kind = "library"
src = [
    "libs/imgui/",
    "libs/imgui/examples/imgui_impl_glfw.cpp",
    "libs/imgui/examples/imgui_impl_opengl3.cpp",
]
extensions = ["cpp"]
include = [
    "libs/imgui/",
    "libs/imgui/examples/",
    "libs/glad/include",
    "libs/glfw/include/",
]
defines = ["IMGUI_IMPL_OPENGL_LOADER_GLAD"]
depends = ["glad", "glfw"]

[[project]]
name = "glm"
kind = "library"
src = [
    "libs/glm/glm/detail",
    "libs/glm/glm/ext",
    "libs/glm/glm/gtc",
    "libs/glm/glm/gtx",
    "libs/glm/glm/simd",
]
extensions = ["cpp"]
include = ["libs/glm/"]
```
