# Bun completions
#compdef bun

_bun_add_completion() {
    _arguments -s -C \
        '1: :->cmd1' \
        '*: :->package' \
        '--config[Load config(bunfig.toml)]: :->config' \
        '-c[Load config(bunfig.toml)]: :->config' \
        '--yarn[Write a yarn.lock file (yarn v1)]' \
        '-y[Write a yarn.lock file (yarn v1)]' \
        '--production[Don'"'"'t install devDependencies]' \
        '-p[Don'"'"'t install devDependencies]' \
        '--no-save[Don'"'"'t save a lockfile]' \
        '--save[Save to package.json]' \
        '--dry-run[Don'"'"'t install anything]' \
        '--frozen-lockfile[Disallow changes to lockfile]' \
        '--force[Always request the latest versions from the registry & reinstall all dependencies]' \
        '-f[Always request the latest versions from the registry & reinstall all dependencies]' \
        '--cache-dir[Store & load cached data from a specific directory path]:cache-dir' \
        '--no-cache[Ignore manifest cache entirely]' \
        '--silent[Don'"'"'t log anything]' \
        '--verbose[Excessively verbose logging]' \
        '--no-progress[Disable the progress bar]' \
        '--no-summary[Don'"'"'t print a summary]' \
        '--no-verify[Skip verifying integrity of newly downloaded packages]' \
        '--ignore-scripts[Skip lifecycle scripts in the package.json (dependency scripts are never run)]' \
        '--global[Add a package globally]' \
        '-g[Add a package globally]' \
        '--cwd[Set a specific cwd]:cwd' \
        '--backend[Platform-specific optimizations for installing dependencies]:backend:("copyfile" "hardlink" "symlink")' \
        '--link-native-bins[Link "bin" from a matching platform-specific dependency instead. Default: esbuild, turbo]:link-native-bins' \
        '--help[Print this help menu]' \
        '--dev[Add dependence to "devDependencies]' \
        '-d[Add dependence to "devDependencies]' \
        '-D[]' \
        '--development[]' \
        '--optional[Add dependency to "optionalDependencies]' \
        '--peer[Add dependency to "peerDependencies]' \
        '--exact[Add the exact version instead of the ^range]' &&
        ret=0

    case $state in
    config)
        _bun_list_bunfig_toml

        ;;
    package)
        _bun_add_param_package_completion

        ;;
    esac
}

_bun_unlink_completion() {
    _arguments -s -C \
        '1: :->cmd1' \
        '*: :->package' \
        '--config[Load config(bunfig.toml)]: :->config' \
        '-c[Load config(bunfig.toml)]: :->config' \
        '--yarn[Write a yarn.lock file (yarn v1)]' \
        '-y[Write a yarn.lock file (yarn v1)]' \
        '--production[Don'"'"'t install devDependencies]' \
        '-p[Don'"'"'t install devDependencies]' \
        '--no-save[Don'"'"'t save a lockfile]' \
        '--save[Save to package.json]' \
        '--dry-run[Don'"'"'t install anything]' \
        '--frozen-lockfile[Disallow changes to lockfile]' \
        '--force[Always request the latest versions from the registry & reinstall all dependencies]' \
        '-f[Always request the latest versions from the registry & reinstall all dependencies]' \
        '--cache-dir[Store & load cached data from a specific directory path]:cache-dir' \
        '--no-cache[Ignore manifest cache entirely]' \
        '--silent[Don'"'"'t log anything]' \
        '--verbose[Excessively verbose logging]' \
        '--no-progress[Disable the progress bar]' \
        '--no-summary[Don'"'"'t print a summary]' \
        '--no-verify[Skip verifying integrity of newly downloaded packages]' \
        '--ignore-scripts[Skip lifecycle scripts in the package.json (dependency scripts are never run)]' \
        '--global[Add a package globally]' \
        '-g[Add a package globally]' \
        '--cwd[Set a specific cwd]:cwd' \
        '--backend[Platform-specific optimizations for installing dependencies]:backend:("copyfile" "hardlink" "symlink")' \
        '--link-native-bins[Link "bin" from a matching platform-specific dependency instead. Default: esbuild, turbo]:link-native-bins' \
        '--help[Print this help menu]' &&
        ret=0

    case $state in
    config)
        _bun_list_bunfig_toml

        ;;
    package)
        # TODO: error: bun unlink {packageName} not implemented yet

        ;;
    esac
}

_bun_link_completion() {
    _arguments -s -C \
        '1: :->cmd1' \
        '*: :->package' \
        '--config[Load config(bunfig.toml)]: :->config' \
        '-c[Load config(bunfig.toml)]: :->config' \
        '--yarn[Write a yarn.lock file (yarn v1)]' \
        '-y[Write a yarn.lock file (yarn v1)]' \
        '--production[Don'"'"'t install devDependencies]' \
        '-p[Don'"'"'t install devDependencies]' \
        '--no-save[Don'"'"'t save a lockfile]' \
        '--save[Save to package.json]' \
        '--dry-run[Don'"'"'t install anything]' \
        '--frozen-lockfile[Disallow changes to lockfile]' \
        '--force[Always request the latest versions from the registry & reinstall all dependencies]' \
        '-f[Always request the latest versions from the registry & reinstall all dependencies]' \
        '--cache-dir[Store & load cached data from a specific directory path]:cache-dir' \
        '--no-cache[Ignore manifest cache entirely]' \
        '--silent[Don'"'"'t log anything]' \
        '--verbose[Excessively verbose logging]' \
        '--no-progress[Disable the progress bar]' \
        '--no-summary[Don'"'"'t print a summary]' \
        '--no-verify[Skip verifying integrity of newly downloaded packages]' \
        '--ignore-scripts[Skip lifecycle scripts in the package.json (dependency scripts are never run)]' \
        '--global[Add a package globally]' \
        '-g[Add a package globally]' \
        '--cwd[Set a specific cwd]:cwd' \
        '--backend[Platform-specific optimizations for installing dependencies]:backend:("copyfile" "hardlink" "symlink")' \
        '--link-native-bins[Link "bin" from a matching platform-specific dependency instead. Default: esbuild, turbo]:link-native-bins' \
        '--help[Print this help menu]' &&
        ret=0

    case $state in
    config)
        _bun_list_bunfig_toml

        ;;
    package)
        _bun_link_param_package_completion

        ;;
    esac
}

_bun_bun_completion() {
    _arguments -s -C \
        '1: :->cmd' \
        '*: :->file' \
        '--version[Show version and exit]' \
        '-V[Show version and exit]' \
        '--cwd[Change directory]:cwd' \
        '--help[Show command help]' \
        '-h[Show command help]' \
        '--use[Use a framework, e.g. "next"]:use' &&
        ret=0

    case $state in
    file)
        _files

        ;;
    esac
}

_bun_init_completion() {
    _arguments -s -C \
        '1: :->cmd' \
        '-y[Answer yes to all prompts]:' \
        '--yes[Answer yes to all prompts]:' &&
        ret=0

}

_bun_create_completion() {
    _arguments -s -C \
        '1: :->cmd' \
        '2: :->cmd2' \
        '*: :->args' &&
        ret=0

    case $state in
    cmd2)
        _alternative 'args:create:((next-app\:"Next.js app" react-app\:"React app"))'

        ;;
    args)
        case $line[2] in
        next)
            pmargs=(
                '1: :->cmd'
                '2: :->cmd2'
                '3: :->file'
                '--force[Overwrite existing files]'
                '--no-install[Don'"'"'t install node_modules]'
                '--no-git[Don'"'"'t create a git repository]'
                '--verbose[verbose]'
                '--no-package-json[Disable package.json transforms]'
                '--open[On finish, start bun & open in-browser]'
            )

            # ---- Command: create next
            _arguments -s -C \
                $pmargs &&
                ret=0

            case $state in
            file)
                _files

                ;;
            esac

            ;;
        react)

            # ---- Command: create react
            _arguments -s -C \
                $pmargs &&
                ret=0

            case $state in
            file)
                _files

                ;;
            esac

            ;;
        *)

            # ---- Command: create from other template
            _arguments -s -C \
                $pmargs &&
                ret=0

            case $state in
            file)
                _files

                ;;
            esac

            ;;
        esac

        ;;

    esac
}

_bun_pm_completion() {
    _arguments -s -C \
        '1: :->cmd' \
        '2: :->cmd2' \
        '*: :->args' &&
        ret=0

    case $state in
    cmd2)
        sub_commands=(
            'bin\:"print the path to bin folder" '
            'ls\:"list the dependency tree according to the current lockfile" '
            'hash\:"generate & print the hash of the current lockfile" '
            'hash-string\:"print the string used to hash the lockfile" '
            'hash-print\:"print the hash stored in the current lockfile" '
            'cache\:"print the path to the cache folder" '
            'version\:"bump the version in package.json and create a git tag" '
        )

        _alternative "args:cmd3:(($sub_commands))"

        ;;
    args)
        case $line[2] in
        cache)
            _arguments -s -C \
                '1: :->cmd' \
                '2: :->cmd2' \
                ':::(rm)' &&
                ret=0

            ;;
        bin)
            pmargs=(
                "-g[print the global path to bin folder]"
            )

            _arguments -s -C \
                '1: :->cmd' \
                '2: :->cmd2' \
                $pmargs &&
                ret=0

            ;;
        ls)
            pmargs=(
                "--all[list the entire dependency tree according to the current lockfile]"
            )

            _arguments -s -C \
                '1: :->cmd' \
                '2: :->cmd2' \
                $pmargs &&
                ret=0

            ;;
        version)
            version_args=(
                "patch[increment patch version]"
                "minor[increment minor version]"
                "major[increment major version]"
                "prepatch[increment patch version and add pre-release]"
                "preminor[increment minor version and add pre-release]"
                "premajor[increment major version and add pre-release]"
                "prerelease[increment pre-release version]"
                "from-git[use version from latest git tag]"
            )

            pmargs=(
                "--no-git-tag-version[don't create a git commit and tag]"
                "--allow-same-version[allow bumping to the same version]"
                "-m[use the given message for the commit]:message"
                "--message[use the given message for the commit]:message"
                "--preid[identifier to prefix pre-release versions]:preid"
            )

            _arguments -s -C \
                '1: :->cmd' \
                '2: :->cmd2' \
                '3: :->increment' \
                $pmargs &&
                ret=0

            case $state in
            increment)
                _alternative "args:increment:(($version_args))"
                ;;
            esac

            ;;
        esac

        ;;
    esac
}

_bun_install_completion() {
    _arguments -s -C \
        '1: :->cmd1' \
        '--config[Load config(bunfig.toml)]: :->config' \
        '-c[Load config(bunfig.toml)]: :->config' \
        '--yarn[Write a yarn.lock file (yarn v1)]' \
        '-y[Write a yarn.lock file (yarn v1)]' \
        '--production[Don'"'"'t install devDependencies]' \
        '-p[Don'"'"'t install devDependencies]' \
        '--no-save[Don'"'"'t save a lockfile]' \
        '--save[Save to package.json]' \
        '--dry-run[Don'"'"'t install anything]' \
        '--frozen-lockfile[Disallow changes to lockfile]' \
        '--force[Always request the latest versions from the registry & reinstall all dependencies]' \
        '-f[Always request the latest versions from the registry & reinstall all dependencies]' \
        '--cache-dir[Store & load cached data from a specific directory path]:cache-dir' \
        '--no-cache[Ignore manifest cache entirely]' \
        '--silent[Don'"'"'t log anything]' \
        '--verbose[Excessively verbose logging]' \
        '--no-progress[Disable the progress bar]' \
        '--no-summary[Don'"'"'t print a summary]' \
        '--no-verify[Skip verifying integrity of newly downloaded packages]' \
        '--ignore-scripts[Skip lifecycle scripts in the package.json (dependency scripts are never run)]' \
        '--global[Add a package globally]' \
        '-g[Add a package globally]' \
        '--cwd[Set a specific cwd]:cwd' \
        '--backend[Platform-specific optimizations for installing dependencies]:backend:("copyfile" "hardlink" "symlink")' \
        '--link-native-bins[Link "bin" from a matching platform-specific dependency instead. Default: esbuild, turbo]:link-native-bins' \
        '--help[Print this help menu]' \
        '--dev[Add dependence to "devDependencies]' \
        '-d[Add dependence to "devDependencies]' \
        '--development[]' \
        '-D[]' \
        '--optional[Add dependency to "optionalDependencies]' \
        '--peer[Add dependency to "peerDependencies]' \
        '--exact[Add the exact version instead of the ^range]' &&
        ret=0

    case $state in
    config)
        _bun_list_bunfig_toml

        ;;
    esac
}

_bun_remove_completion() {
    _arguments -s -C \
        '1: :->cmd1' \
        '*: :->package' \
        '--config[Load config(bunfig.toml)]: :->config' \
        '-c[Load config(bunfig.toml)]: :->config' \
        '--yarn[Write a yarn.lock file (yarn v1)]' \
        '-y[Write a yarn.lock file (yarn v1)]' \
        '--production[Don'"'"'t install devDependencies]' \
        '-p[Don'"'"'t install devDependencies]' \
        '--no-save[Don'"'"'t save a lockfile]' \
        '--save[Save to package.json]' \
        '--dry-run[Don'"'"'t install anything]' \
        '--frozen-lockfile[Disallow changes to lockfile]' \
        '--force[Always request the latest versions from the registry & reinstall all dependencies]' \
        '-f[Always request the latest versions from the registry & reinstall all dependencies]' \
        '--cache-dir[Store & load cached data from a specific directory path]:cache-dir' \
        '--no-cache[Ignore manifest cache entirely]' \
        '--silent[Don'"'"'t log anything]' \
        '--verbose[Excessively verbose logging]' \
        '--no-progress[Disable the progress bar]' \
        '--no-summary[Don'"'"'t print a summary]' \
        '--no-verify[Skip verifying integrity of newly downloaded packages]' \
        '--ignore-scripts[Skip lifecycle scripts in the package.json (dependency scripts are never run)]' \
        '--global[Add a package globally]' \
        '-g[Add a package globally]' \
        '--cwd[Set a specific cwd]:cwd' \
        '--backend[Platform-specific optimizations for installing dependencies]:backend:("copyfile" "hardlink" "symlink")' \
        '--link-native-bins[Link "bin" from a matching platform-specific dependency instead. Default: esbuild, turbo]:link-native-bins' \
        '--help[Print this help menu]' &&
        ret=0

    case $state in
    config)
        _bun_list_bunfig_toml

        ;;
    package)
        _bun_remove_param_package_completion

        ;;
    esac
}

_bun_run_completion() {
    _arguments -s -C \
        '1: :->cmd' \
        '2: :->script' \
        '*: :->other' \
        '--help[Display this help and exit]' \
        '-h[Display this help and exit]' \
        '--bun[Force a script or package to use Bun'"'"'s runtime instead of Node.js (via symlinking node)]' \
        '-b[Force a script or package to use Bun'"'"'s runtime instead of Node.js (via symlinking node)]' \
        '--cwd[Absolute path to resolve files & entry points from. This just changes the process cwd]:cwd' \
        '--config[Config file to load bun from (e.g. -c bunfig.toml]: :->config' \
        '-c[Config file to load bun from (e.g. -c bunfig.toml]: :->config' \
        '--env-file[Load environment variables from the specified file(s)]:env-file' \
        '--extension-order[Defaults to: .tsx,.ts,.jsx,.js,.json]:extension-order' \
        '--jsx-factory[Changes the function called when compiling JSX elements using the classic JSX runtime]:jsx-factory' \
        '--jsx-fragment[Changes the function called when compiling JSX fragments]:jsx-fragment' \
        '--jsx-import-source[Declares the module specifier to be used for importing the jsx and jsxs factory functions. Default: "react"]:jsx-import-source' \
        '--jsx-runtime["automatic" (default) or "classic"]: :->jsx-runtime' \
        '--preload[Import a module before other modules are loaded]:preload' \
        '-r[Import a module before other modules are loaded]:preload' \
        '--main-fields[Main fields to lookup in package.json. Defaults to --target dependent]:main-fields' \
        '--no-summary[Don'"'"'t print a summary]' \
        '--version[Print version and exit]' \
        '-v[Print version and exit]' \
        '--revision[Print version with revision and exit]' \
        '--tsconfig-override[Load tsconfig from path instead of cwd/tsconfig.json]:tsconfig-override' \
        '--define[Substitute K:V while parsing, e.g. --define process.env.NODE_ENV:"development". Values are parsed as JSON.]:define' \
        '-d[Substitute K:V while parsing, e.g. --define process.env.NODE_ENV:"development". Values are parsed as JSON.]:define' \
        '--external[Exclude module from transpilation (can use * wildcards). ex: -e react]:external' \
        '-e[Exclude module from transpilation (can use * wildcards). ex: -e react]:external' \
        '--loader[Parse files with .ext:loader, e.g. --loader .js:jsx. Valid loaders: js, jsx, ts, tsx, json, toml, text, file, wasm, napi]:loader' \
        '--packages[Exclude dependencies from bundle, e.g. --packages external. Valid options: bundle, external]:packages' \
        '-l[Parse files with .ext:loader, e.g. --loader .js:jsx. Valid loaders: js, jsx, ts, tsx, json, toml, text, file, wasm, napi]:loader' \
        '--origin[Rewrite import URLs to start with --origin. Default: ""]:origin' \
        '-u[Rewrite import URLs to start with --origin. Default: ""]:origin' \
        '--port[Port to serve bun'"'"'s dev server on. Default: '"'"'3000'"'"']:port' \
        '-p[Port to serve bun'"'"'s dev server on. Default: '"'"'3000'"'"']:port' \
        '--smol[Use less memory, but run garbage collection more often]' \
        '--minify[Minify (experimental)]' \
        '--minify-syntax[Minify syntax and inline data (experimental)]' \
        '--minify-whitespace[Minify Whitespace (experimental)]' \
        '--minify-identifiers[Minify identifiers]' \
        '--no-macros[Disable macros from being executed in the bundler, transpiler and runtime]' \
        '--target[The intended execution environment for the bundle. "browser", "bun" or "node"]: :->target' \
        '--inspect[Activate Bun'"'"'s Debugger]:inspect' \
        '--inspect-wait[Activate Bun'"'"'s Debugger, wait for a connection before executing]:inspect-wait' \
        '--inspect-brk[Activate Bun'"'"'s Debugger, set breakpoint on first line of code and wait]:inspect-brk' \
        '--hot[Enable auto reload in bun'"'"'s JavaScript runtime]' \
        '--watch[Automatically restart bun'"'"'s JavaScript runtime on file change]' \
        '--no-install[Disable auto install in bun'"'"'s JavaScript runtime]' \
        '--install[Install dependencies automatically when no node_modules are present, default: "auto". "force" to ignore node_modules, fallback to install any missing]: :->install_' \
        '-i[Automatically install dependencies and use global cache in bun'"'"'s runtime, equivalent to --install=fallback'] \
        '--prefer-offline[Skip staleness checks for packages in bun'"'"'s JavaScript runtime and resolve from disk]' \
        '--prefer-latest[Use the latest matching versions of packages in bun'"'"'s JavaScript runtime, always checking npm]' \
        '--silent[Don'"'"'t repeat the command for bun run]' \
        '--dump-environment-variables[Dump environment variables from .env and process as JSON and quit. Useful for debugging]' \
        '--dump-limits[Dump system limits. Userful for debugging]' &&
        ret=0

    case $state in
    script)
        curcontext="${curcontext%:*:*}:bun-grouped"
        _bun_run_param_script_completion

        ;;
    jsx-runtime)
        _alternative 'args:cmd3:((classic automatic))'

        ;;
    target)
        _alternative 'args:cmd3:((browser bun node))'

        ;;
    install_)
        _alternative 'args:cmd3:((auto force fallback))'

        ;;
    other)
        _files

        ;;
    esac

}

_bun_upgrade_completion() {
    _arguments -s -C \
        '1: :->cmd' \
        '--canary[Upgrade to canary build]' &&
        ret=0

}

_bun_build_completion() {
    _arguments -s -C \
        '1: :->cmd' \
        '*: :->file' \
        '--outfile[Write the output to a specific file (default: stdout)]:outfile' \
        '--outdir[Write the output to a directory (required for splitting)]:outdir' \
        '--minify[Enable all minification flags]' \
        '--minify-whitespace[Remove unneeded whitespace]' \
        '--minify-syntax[Transform code to use less syntax]' \
        '--minify-identifiers[Shorten variable names]' \
        '--sourcemap[Generate sourcemaps]: :->sourcemap' \
        '--target[The intended execution environment for the bundle. "browser", "bun" or "node"]: :->target' \
        '--splitting[Whether to enable code splitting (requires --outdir)]' \
        '--compile[generating a standalone binary from a TypeScript or JavaScript file]' \
        '--format[Specifies the module format to be used in the generated bundles]: :->format' &&
        ret=0

    case $state in
    file)
        _files

        ;;
    target)
        _alternative 'args:cmd3:((browser bun node))'

        ;;
    sourcemap)
        _alternative 'args:cmd3:((none external inline))'

        ;;
    format)

        _alternative 'args:cmd3:((esm cjs iife))'
        ;;
    esac
}

_bun_update_completion() {
    _arguments -s -C \
        '1: :->cmd1' \
        '-c[Load config(bunfig.toml)]: :->config' \
        '--config[Load config(bunfig.toml)]: :->config' \
        '-y[Write a yarn.lock file (yarn v1)]' \
        '--yarn[Write a yarn.lock file (yarn v1)]' \
        '-p[Don'"'"'t install devDependencies]' \
        '--production[Don'"'"'t install devDependencies]' \
        '--no-save[Don'"'"'t save a lockfile]' \
        '--save[Save to package.json]' \
        '--dry-run[Don'"'"'t install anything]' \
        '--frozen-lockfile[Disallow changes to lockfile]' \
        '--latest[Updates dependencies to latest version, regardless of compatibility]' \
        '-f[Always request the latest versions from the registry & reinstall all dependencies]' \
        '--force[Always request the latest versions from the registry & reinstall all dependencies]' \
        '--cache-dir[Store & load cached data from a specific directory path]:cache-dir' \
        '--no-cache[Ignore manifest cache entirely]' \
        '--silent[Don'"'"'t log anything]' \
        '--verbose[Excessively verbose logging]' \
        '--no-progress[Disable the progress bar]' \
        '--no-summary[Don'"'"'t print a summary]' \
        '--no-verify[Skip verifying integrity of newly downloaded packages]' \
        '--ignore-scripts[Skip lifecycle scripts in the package.json (dependency scripts are never run)]' \
        '-g[Add a package globally]' \
        '--global[Add a package globally]' \
        '--cwd[Set a specific cwd]:cwd' \
        '--backend[Platform-specific optimizations for installing dependencies]:backend:("copyfile" "hardlink" "symlink")' \
        '--link-native-bins[Link "bin" from a matching platform-specific dependency instead. Default: esbuild, turbo]:link-native-bins' \
        '--help[Print this help menu]' &&
        ret=0

    case $state in
    config)
        _bun_list_bunfig_toml

        ;;
    esac
}

_bun_outdated_completion() {
    _arguments -s -C \
        '--cwd[Set a specific cwd]:cwd' \
        '--verbose[Excessively verbose logging]' \
        '--no-progress[Disable the progress bar]' \
        '--help[Print this help menu]' &&
        ret=0

    case $state in
    config)
        _bun_list_bunfig_toml

        ;;
    esac
}

_bun_test_completion() {
    _arguments -s -C \
        '1: :->cmd1' \
        '*: :->file' \
        '-h[Display this help and exit]' \
        '--help[Display this help and exit]' \
        '-b[Force a script or package to use Bun.js instead of Node.js (via symlinking node)]' \
        '--bun[Force a script or package to use Bun.js instead of Node.js (via symlinking node)]' \
        '--cwd[Set a specific cwd]:cwd' \
        '-c[Load config(bunfig.toml)]: :->config' \
        '--config[Load config(bunfig.toml)]: :->config' \
        '--env-file[Load environment variables from the specified file(s)]:env-file' \
        '--extension-order[Defaults to: .tsx,.ts,.jsx,.js,.json]:extension-order' \
        '--jsx-factory[Changes the function called when compiling JSX elements using the classic JSX runtime]:jsx-factory' \
        '--jsx-fragment[Changes the function called when compiling JSX fragments]:jsx-fragment' \
        '--jsx-import-source[Declares the module specifier to be used for importing the jsx and jsxs factory functions. Default: "react"]:jsx-import-source' \
        '--jsx-runtime["automatic" (default) or "classic"]: :->jsx-runtime' \
        '--preload[Import a module before other modules are loaded]:preload' \
        '-r[Import a module before other modules are loaded]:preload' \
        '--main-fields[Main fields to lookup in package.json. Defaults to --target dependent]:main-fields' \
        '--no-summary[Don'"'"'t print a summary]' \
        '--version[Print version and exit]' \
        '-v[Print version and exit]' \
        '--revision[Print version with revision and exit]' \
        '--tsconfig-override[Load tsconfig from path instead of cwd/tsconfig.json]:tsconfig-override' \
        '--define[Substitute K:V while parsing, e.g. --define process.env.NODE_ENV:"development". Values are parsed as JSON.]:define' \
        '-d[Substitute K:V while parsing, e.g. --define process.env.NODE_ENV:"development". Values are parsed as JSON.]:define' \
        '--external[Exclude module from transpilation (can use * wildcards). ex: -e react]:external' \
        '-e[Exclude module from transpilation (can use * wildcards). ex: -e react]:external' \
        '--loader[Parse files with .ext:loader, e.g. --loader .js:jsx. Valid loaders: js, jsx, ts, tsx, json, toml, text, file, wasm, napi]:loader' \
        '-l[Parse files with .ext:loader, e.g. --loader .js:jsx. Valid loaders: js, jsx, ts, tsx, json, toml, text, file, wasm, napi]:loader' \
        '--origin[Rewrite import URLs to start with --origin. Default: ""]:origin' \
        '-u[Rewrite import URLs to start with --origin. Default: ""]:origin' \
        '--port[Port to serve bun'"'"'s dev server on. Default: '"'"'3000'"'"']:port' \
        '-p[Port to serve bun'"'"'s dev server on. Default: '"'"'3000'"'"']:port' \
        '--smol[Use less memory, but run garbage collection more often]' \
        '--minify[Minify (experimental)]' \
        '--minify-syntax[Minify syntax and inline data (experimental)]' \
        '--minify-identifiers[Minify identifiers]' \
        '--no-macros[Disable macros from being executed in the bundler, transpiler and runtime]' \
        '--target[The intended execution environment for the bundle. "browser", "bun" or "node"]: :->target' \
        '--inspect[Activate Bun'"'"'s Debugger]:inspect' \
        '--inspect-wait[Activate Bun'"'"'s Debugger, wait for a connection before executing]:inspect-wait' \
        '--inspect-brk[Activate Bun'"'"'s Debugger, set breakpoint on first line of code and wait]:inspect-brk' \
        '--watch[Automatically restart bun'"'"'s JavaScript runtime on file change]' \
        '--timeout[Set the per-test timeout in milliseconds, default is 5000.]:timeout' \
        '--update-snapshots[Update snapshot files]' \
        '--rerun-each[Re-run each test file <NUMBER> times, helps catch certain bugs]:rerun' \
        '--only[Only run tests that are marked with "test.only()"]' \
        '--todo[Include tests that are marked with "test.todo()"]' \
        '--coverage[Generate a coverage profile]' \
        '--bail[Exit the test suite after <NUMBER> failures. If you do not specify a number, it defaults to 1.]:bail' \
        '--test-name-pattern[Run only tests with a name that matches the given regex]:pattern' \
        '-t[Run only tests with a name that matches the given regex]:pattern' &&
        ret=0

    case $state in
    file)
        _bun_test_param_script_completion

        ;;
    config)
        _files

        ;;

    esac

}

_bun() {
    zstyle ':completion:*:*:bun:*' group-name ''
    zstyle ':completion:*:*:bun-grouped:*' group-name ''

    zstyle ':completion:*:*:bun::descriptions' format '%F{green}-- %d --%f'
    zstyle ':completion:*:*:bun-grouped:*' format '%F{green}-- %d --%f'
    # zstyle ':completion:*:options' list-colors '=^(-- *)=34'

    local program=bun
    typeset -A opt_args
    local curcontext="$curcontext" state line context

    # ---- Command:
    _arguments -s \
        '1: :->cmd' \
        '*: :->args' &&
        ret=0

    case $state in
    cmd)
        local -a scripts_list
        IFS=$'\n' scripts_list=($(SHELL=zsh bun getcompletes i))
        scripts="scripts:scripts:((${scripts_list//:/\\\\:}))"
        IFS=$'\n' files_list=($(SHELL=zsh bun getcompletes j))

        main_commands=(
            'run\:"Run JavaScript with Bun, a package.json script, or a bin" '
            'test\:"Run unit tests with Bun" '
            'x\:"Install and execute a package bin (bunx)" '
            'repl\:"Start a REPL session with Bun" '
            'init\:"Start an empty Bun project from a blank template" '
            'create\:"Create a new project from a template (bun c)" '
            'install\:"Install dependencies for a package.json (bun i)" '
            'add\:"Add a dependency to package.json (bun a)" '
            'remove\:"Remove a dependency from package.json (bun rm)" '
            'update\:"Update outdated dependencies & save to package.json" '
            'outdated\:"Display the latest versions of outdated dependencies" '
            'link\:"Link an npm package globally" '
            'unlink\:"Globally unlink an npm package" '
            'pm\:"More commands for managing packages" '
            'build\:"Bundle TypeScript & JavaScript into a single file" '
            'upgrade\:"Get the latest version of bun" '
            'help\:"Show all supported flags and commands" '
        )
        main_commands=($main_commands)
        _alternative "$scripts" "args:command:(($main_commands))" "files:files:(($files_list))"

        ;;
    args)
        case $line[1] in
        add|a)
            _bun_add_completion

            ;;
        unlink)
            _bun_unlink_completion

            ;;
        link)
            _bun_link_completion

            ;;
        bun)
            _bun_bun_completion

            ;;
        init)
            _bun_init_completion

            ;;
        create|c)
            _bun_create_completion

            ;;
        x)
            _arguments -s -C \
                '1: :->cmd' \
                '2: :->cmd2' \
                '*: :->args' &&
                ret=0
            ;;
        pm)
            _bun_pm_completion

            ;;
        install|i)
            _bun_install_completion

            ;;
        remove|rm)
            _bun_remove_completion

            ;;
        run)
            _bun_run_completion

            ;;
        upgrade)
            _bun_upgrade_completion

            ;;
        build)
            _bun_build_completion

            ;;
        update)
            _bun_update_completion

            ;;
        outdated)
            _bun_outdated_completion

            ;;
        'test')
            _bun_test_completion

            ;;
        help)
            # ---- Command: help
            _arguments -s -C \
                '1: :->cmd' \
                '2: :->cmd2' \
                '*: :->args' &&
                ret=0

            case $state in
            cmd2)
                curcontext="${curcontext%:*:*}:bun-grouped"
                _alternative "args:command:(($main_commands))"

                ;;
            args)
                case $line[2] in
                add)
                    _bun_add_completion

                    ;;
                unlink)
                    _bun_unlink_completion

                    ;;
                link)
                    _bun_link_completion

                    ;;
                bun)
                    _bun_bun_completion

                    ;;
                init)
                    _bun_init_completion

                    ;;
                create)
                    _bun_create_completion

                    ;;
                x)
                    _arguments -s -C \
                        '1: :->cmd' \
                        '2: :->cmd2' \
                        '*: :->args' &&
                        ret=0
                    ;;
                pm)
                    _bun_pm_completion

                    ;;
                install)
                    _bun_install_completion

                    ;;
                remove)
                    _bun_remove_completion

                    ;;
                run)
                    _bun_run_completion

                    ;;
                upgrade)
                    _bun_upgrade_completion

                    ;;
                build)
                    _bun_build_completion

                    ;;
                update)
                    _bun_update_completion

                    ;;
                outdated)
                    _bun_outdated_completion

                    ;;
                'test')
                    _bun_test_completion

                    ;;
                esac

                ;;
            esac

            ;;
        esac

        ;;
    esac
}

_bun_list_bunfig_toml() {
    # _alternative "files:file:_files -g '*.toml'"
    _files
}

_bun_run_param_script_completion() {
    local -a scripts_list
    IFS=$'\n' scripts_list=($(SHELL=zsh bun getcompletes s))
    IFS=$'\n' bins=($(SHELL=zsh bun getcompletes b))

    _alternative "scripts:scripts:((${scripts_list//:/\\\\:}))"
    _alternative "bin:bin:((${bins//:/\\\\:}))"
    _alternative "files:file:_files -g '*.(js|ts|jsx|tsx|wasm)'"
}

_bun_link_param_package_completion() {
    # Read packages from ~/.bun/install/global/node_modules
    install_env=$BUN_INSTALL
    install_dir=${(P)install_env:-$HOME/.bun}
    global_node_modules=$install_dir/install/global/node_modules

    local -a packages_full_path=(${global_node_modules}/*(N))
    packages=$(echo $packages_full_path | tr ' ' '\n' | xargs  basename)
    _alternative "dirs:directory:(($packages))"
}

_bun_remove_param_package_completion() {
    if ! command -v jq &>/dev/null; then
        return
    fi

    # TODO: move to "bun getcompletes"
    if [ -f "package.json" ]; then
        local dependencies=$(jq -r '.dependencies | keys[]' package.json)
        local dev_dependencies=$(jq -r '.devDependencies | keys[]' package.json)
        _alternative "deps:dependency:(($dependencies))"
        _alternative "deps:dependency:(($dev_dependencies))"
    fi
}

_bun_test_param_script_completion() {
    local -a scripts_list

    _alternative "files:file:_files -g '*(_|.)(test|spec).(js|ts|jsx|tsx)'"
}

_set_remove() {
    comm -23 <(echo $1 | sort | tr " " "\n") <(echo $2 | sort | tr " " "\n") 2>/dev/null
}

_bun_add_param_package_completion() {

    IFS=$'\n' inexact=($(history -n bun | grep -E "^bun add " | cut -c 9- | uniq))
    IFS=$'\n' exact=($($inexact | grep -E "^$words[$CURRENT]"))
    IFS=$'\n' packages=($(SHELL=zsh bun getcompletes a $words[$CURRENT]))

    to_print=$inexact
    if [ ! -z "$exact" -a "$exact" != " " ]; then
        to_print=$exact
    fi

    if [ ! -z "$to_print" -a "$to_print" != " " ]; then
        if [ ! -z "$packages" -a "$packages" != " " ]; then
            _describe -1 -t to_print 'History' to_print
            _describe -1 -t packages "Popular" packages
            return
        fi

        _describe -1 -t to_print 'History' to_print
        return
    fi

    if [ ! -z "$packages" -a "$packages" != " " ]; then
        _describe -1 -t packages "Popular" packages
        return
    fi

}

__bun_dynamic_comp() {
    local comp=""

    for arg in scripts; do
        local line
        while read -r line; do
            local name="$line"
            local desc="$line"
            name="${name%$'\t'*}"
            desc="${desc/*$'\t'/}"
            echo
        done <<<"$arg"
    done

    return $comp
}

if ! command -v compinit >/dev/null; then
    autoload -U compinit && compinit
fi

compdef _bun bun

# cf completions
#compdef cf
compdef _cf cf

# zsh completion for cf -*- shell-script -*-

__cf_debug() {
    local file="$BASH_COMP_DEBUG_FILE"
    if [[ -n ${file} ]]; then
        echo "$*" >> "${file}"
    fi
}

_cf() {
    local shellCompDirectiveError=1
    local shellCompDirectiveNoSpace=2
    local shellCompDirectiveNoFileComp=4
    local shellCompDirectiveFilterFileExt=8
    local shellCompDirectiveFilterDirs=16
    local shellCompDirectiveKeepOrder=32

    local lastParam lastChar flagPrefix requestComp out directive comp lastComp noSpace keepOrder
    local -a completions

    __cf_debug "\n========= starting completion logic =========="
    __cf_debug "CURRENT: ${CURRENT}, words[*]: ${words[*]}"

    # The user could have moved the cursor backwards on the command-line.
    # We need to trigger completion from the $CURRENT location, so we need
    # to truncate the command-line ($words) up to the $CURRENT location.
    # (We cannot use $CURSOR as its value does not work when a command is an alias.)
    words=( "${=words[1,CURRENT]}" )
    __cf_debug "Truncated words[*]: ${words[*]},"

    lastParam=${words[-1]}
    lastChar=${lastParam[-1]}
    __cf_debug "lastParam: ${lastParam}, lastChar: ${lastChar}"

    # For zsh, when completing a flag with an = (e.g., cf -n=<TAB>)
    # completions must be prefixed with the flag
    setopt local_options BASH_REMATCH
    if [[ "${lastParam}" =~ '-.*=' ]]; then
        # We are dealing with a flag with an =
        flagPrefix="-P ${BASH_REMATCH}"
    fi

    # Prepare the command to obtain completions, ensuring arguments are quoted for eval
    local -a args_to_quote=("${(@)words[2,-1]}")
    if [ "${lastChar}" = "" ] && [ "${args_to_quote[-1]}" != "" ]; then
        # If the last parameter is complete (there is a space following it)
        # We add an extra empty parameter so we can indicate this to the go completion code.
        __cf_debug "Adding extra empty parameter"
        args_to_quote+=("")
    fi

    # Use Zsh's (q) flag to quote each argument safely for eval
    local quoted_args=("${(@q)args_to_quote}")

    # Join the main command and the quoted arguments into a single string for eval
    requestComp="cf complete -- ${quoted_args[*]}"

    __cf_debug "About to call: eval ${requestComp}"

    # Use eval to handle any environment variables and such
    out=$(eval ${requestComp} 2>/dev/null)
    __cf_debug "completion output: ${out}"

    # Extract the directive integer following a : from the last line
    local lastLine
    while IFS='
' read -r line; do
        lastLine=${line}
    done < <(printf "%s
" "${out[@]}")
    __cf_debug "last line: ${lastLine}"

    if [ "${lastLine[1]}" = : ]; then
        directive=${lastLine[2,-1]}
        # Remove the directive including the : and the newline
        local suffix
        (( suffix=${#lastLine}+2))
        out=${out[1,-$suffix]}
    else
        # There is no directive specified.  Leave $out as is.
        __cf_debug "No directive found.  Setting to default"
        directive=0
    fi

    __cf_debug "directive: ${directive}"
    __cf_debug "completions: ${out}"
    __cf_debug "flagPrefix: ${flagPrefix}"

    if [ $((directive & shellCompDirectiveError)) -ne 0 ]; then
        __cf_debug "Completion received error. Ignoring completions."
        return
    fi

    local activeHelpMarker="%"
    local endIndex=${#activeHelpMarker}
    local startIndex=$((${#activeHelpMarker}+1))
    local hasActiveHelp=0
    while IFS='
' read -r comp; do
        # Check if this is an activeHelp statement (i.e., prefixed with $activeHelpMarker)
        if [ "${comp[1,$endIndex]}" = "$activeHelpMarker" ];then
            __cf_debug "ActiveHelp found: $comp"
            comp="${comp[$startIndex,-1]}"
            if [ -n "$comp" ]; then
                compadd -x "${comp}"
                __cf_debug "ActiveHelp will need delimiter"
                hasActiveHelp=1
            fi
            continue
        fi

        if [ -n "$comp" ]; then
            # If requested, completions are returned with a description.
            # The description is preceded by a TAB character.
            # For zsh's _describe, we need to use a : instead of a TAB.
            # We first need to escape any : as part of the completion itself.
            comp=${comp//:/\:}

            local tab="$(printf '\t')"
            comp=${comp//$tab/:}

            __cf_debug "Adding completion: ${comp}"
            completions+=${comp}
            lastComp=$comp
        fi
    done < <(printf "%s
" "${out[@]}")

    # Add a delimiter after the activeHelp statements, but only if:
    # - there are completions following the activeHelp statements, or
    # - file completion will be performed (so there will be choices after the activeHelp)
    if [ $hasActiveHelp -eq 1 ]; then
        if [ ${#completions} -ne 0 ] || [ $((directive & shellCompDirectiveNoFileComp)) -eq 0 ]; then
            __cf_debug "Adding activeHelp delimiter"
            compadd -x "--"
            hasActiveHelp=0
        fi
    fi

    if [ $((directive & shellCompDirectiveNoSpace)) -ne 0 ]; then
        __cf_debug "Activating nospace."
        noSpace="-S ''"
    fi

    if [ $((directive & shellCompDirectiveKeepOrder)) -ne 0 ]; then
        __cf_debug "Activating keep order."
        keepOrder="-V"
    fi

    if [ $((directive & shellCompDirectiveFilterFileExt)) -ne 0 ]; then
        # File extension filtering
        local filteringCmd
        filteringCmd='_files'
        for filter in ${completions[@]}; do
            if [ ${filter[1]} != '*' ]; then
                # zsh requires a glob pattern to do file filtering
                filter="\*.$filter"
            fi
            filteringCmd+=" -g $filter"
        done
        filteringCmd+=" ${flagPrefix}"

        __cf_debug "File filtering command: $filteringCmd"
        _arguments '*:filename:'"$filteringCmd"
    elif [ $((directive & shellCompDirectiveFilterDirs)) -ne 0 ]; then
        # File completion for directories only
        local subdir
        subdir="${completions[1]}"
        if [ -n "$subdir" ]; then
            __cf_debug "Listing directories in $subdir"
            pushd "${subdir}" >/dev/null 2>&1
        else
            __cf_debug "Listing directories in ."
        fi

        local result
        _arguments '*:dirname:_files -/'" ${flagPrefix}"
        result=$?
        if [ -n "$subdir" ]; then
            popd >/dev/null 2>&1
        fi
        return $result
    else
        __cf_debug "Calling _describe"
        if eval _describe $keepOrder "completions" completions -Q ${flagPrefix} ${noSpace}; then
            __cf_debug "_describe found some completions"

            # Return the success of having called _describe
            return 0
        else
            __cf_debug "_describe did not find completions."
            __cf_debug "Checking if we should do file completion."
            if [ $((directive & shellCompDirectiveNoFileComp)) -ne 0 ]; then
                __cf_debug "deactivating file completion"

                # Return 0 to indicate completion is finished and prevent zsh from
                # trying other completion algorithms (which could cause hanging).
                # We use NoFileComp directive to explicitly disable file completion.
                return 0
            else
                # Perform file completion
                __cf_debug "Activating file completion"

                # We must return the result of this command, so it must be the
                # last command, or else we must store its result to return it.
                _arguments '*:filename:_files'" ${flagPrefix}"
            fi
        fi
    fi
}

# don't run the completion function when being sourced or eval-ed
if [ "${funcstack[1]}" = "_cf" ]; then
    _cf
fi

