# Unity Commander

A command line interface for Unity projects written in Rust.

Because typing `ucom open .` to open the Unity project in the current directory is sometimes more convenient than having
to deal with the Unity Hub.

Some examples:

- `ucom list` lists all the Unity versions on the system.
- `ucom list -u 2021.3` lists the Unity versions in the 2021.3 range on the system.

- `ucom new ~/Develop/MyProject` creates a new project using the latest Unity version on the system and initializes a
  git repository wih a Unity specific `.gitignore`.
- `ucom new ~/Develop/MyProject -u 2021.3 -- -quit` creates a new project using the latest 2021.3 version on the system
  and closes the editor after it has been created.

- `ucom open ~/Develop/MyProject` opens the project in the directory.
- `ucom open ~/Develop/MyProject -u 2021.3` opens the project with the latest 2021.3 version. Use it to e.g. upgrade the
  project to the latest Unity version.

- `ucom build . ios` builds the project in the current directory for iOS in batch mode.

## How to install

- Make sure [Rust](https://www.rust-lang.org) is installed on your system (v1.65.0 or later).
- Run `cargo install --git https://github.com/jakkovanhunen/ucom`.
- The `ucom` command is now available from the commandline.

Or build manually:

- Clone the repository.
- Run `cargo build --release` in the root of this project.
- After completion the executable can be found in the `target/release` directory.

## Limitations

- Requires that Unity Hub and the editors are installed in the default locations.
- Requires that git is available for initializing a repository when using the `new` command.

## `ucom help`

```
Usage: ucom [COMMAND]

Commands:
  list   Shows a list of the installed Unity versions
  new    Creates a new Unity project and Git repository (uses latest available Unity version by default)
  open   Opens the given Unity project in the Unity Editor [aliases: o]
  build  Builds the given Unity project [aliases: b]
  run    Runs Unity with the givens arguments (uses latest available Unity version by default) [aliases: r]
  help   Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help information
  -V, --version  Print version information
```

## `ucom help list`

```
Shows a list of the installed Unity versions

Usage: ucom.exe list [OPTIONS]

Options:
  -u, --unity <VERSION>  The Unity versions to list. You can specify a partial version; e.g. 2021 will list all
                         the 2021.x.y versions you have installed on your system.
  -h, --help             Print help information
```

## `ucom help new`

```
Creates a new Unity project and Git repository (uses latest available Unity version by default)

Usage: ucom.exe new [OPTIONS] <DIRECTORY> [-- <UNITY_ARGS>...]

Arguments:
  <DIRECTORY>      The directory where the project is created. This directory should not exist yet
  [UNITY_ARGS]...  A list of arguments passed directly to Unity

Options:
  -u, --unity <VERSION>  The Unity version to use for the new project. You can specify a partial version;
                         e.g. 2021 will match the latest 2021.x.y version you have installed on your system.
      --no-git           Suppress initializing a new git repository
  -w, --wait             Waits for the command to finish before continuing
  -q, --quiet            Do not print ucom log messages
  -n, --dry-run          Show what would be run, but do not actually run it
  -h, --help             Print help information
```

## `ucom help open`

```
Opens the given Unity project in the Unity Editor

Usage: ucom.exe open [OPTIONS] <DIRECTORY> [-- <UNITY_ARGS>...]

Arguments:
  <DIRECTORY>      The directory of the project
  [UNITY_ARGS]...  A list of arguments passed directly to Unity

Options:
  -u, --unity <VERSION>  The Unity version to open the project with. Use it to open a project with a newer
                         Unity version. You can specify a partial version; e.g. 2021 will match the latest
                         2021.x.y version you have installed on your system.
  -w, --wait             Waits for the command to finish before continuing
  -q, --quiet            Do not print ucom log messages
  -n, --dry-run          Show what would be run, but do not actually run it
  -h, --help             Print help information
```

## `ucom help build`

```
Builds the given Unity project

Usage: ucom.exe build [OPTIONS] <DIRECTORY> <TARGET> [-- <UNITY_ARGS>...]

Arguments:
  <DIRECTORY>
          The directory of the project

  <TARGET>
          The target platform to build for

          [possible values: win32, win64, macos, linux64, ios, android, webgl]

  [UNITY_ARGS]...
          A list of arguments passed directly to Unity

Options:
  -o, --output <DIRECTORY>
          The output directory of the build. When omitted the build will be placed in
          <DIRECTORY>/Builds/<TARGET>

  -i, --inject <ACTION>
          Build script injection method

          [default: auto]

          Possible values:
          - auto:       If there is no build script, inject one and remove it after the build
          - persistent: Inject the build script into the project and don't remove it afterwards
          - off:        Don't inject the build script and use the one that is already in the project

  -m, --mode <MODE>
          Build mode

          [default: batch]

          Possible values:
          - batch:
            Build in batch mode and wait for the build to finish
          - batch-nogfx:
            Build in batch mode without the graphics device and wait for the build to finish
          - editor-quit:
            Build in the editor and quit after the build
          - editor:
            Build in the editor and keep it open (handy for debugging the build process)

  -l, --log-file <FILE>
          [default: build.log]

  -n, --dry-run
          Show what would be run, but do not actually run it

  -h, --help
          Print help information (use `-h` for a summary)
```

## `ucom help run`

```
Runs Unity with the givens arguments (uses latest available Unity version by default)

Usage: ucom.exe run [OPTIONS] -- <UNITY_ARGS>...

Arguments:
  <UNITY_ARGS>...  A list of arguments passed directly to Unity

Options:
  -u, --unity <VERSION>  The Unity version to run. You can specify a partial version; e.g. 2021 will match the
                         latest 2021.x.y version you have installed on your system.
  -w, --wait             Waits for the command to finish before continuing
  -q, --quiet            Do not print ucom log messages
  -n, --dry-run          Show what would be run, but do not actually run it
  -h, --help             Print help information
```