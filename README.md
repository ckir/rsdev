# rsdev
My Rust learning expirements

Use

git clone --recurse-submodules git@github.com:ckir/rsdev.git

to clone also submodules

Use

git submodule update --init --recursive

This command initializes, fetches, and checks out the submodule’s content.

## Binaries

### `dir-to-yaml`

A command-line tool to export a directory structure to YAML.

**Usage:**

```bash
dir-to-yaml <path> [flags]
```

**Arguments:**

*   `<path>`: The directory to scan.

**Flags:**

*   `--no-files`: Exclude files from the output.
*   `--use-gitignore`: Exclude items from output based on .gitignore files.
```

