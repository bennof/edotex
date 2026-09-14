#!/bin/sh
# Release artifacts may depend on the OS runtime, but not third-party dylibs/so files.
set -eu
binary=${1:?Usage: check-native-linkage.sh BINARY}
case $(uname -s) in
    Darwin)
        dependencies=$(otool -L "$binary")
        printf '%s\n' "$dependencies"
        printf '%s\n' "$dependencies" | awk '
            NR > 1 && $1 !~ /^\/usr\/lib\// && $1 !~ /^\/System\/Library\// { bad = 1 }
            END { exit bad }
        '
        ;;
    Linux)
        dependencies=$(readelf -d "$binary")
        printf '%s\n' "$dependencies"
        printf '%s\n' "$dependencies" | awk '
            /\(NEEDED\)/ {
                name = $NF
                gsub(/[][]/, "", name)
                if (name !~ /^lib(c|m|dl|pthread|rt|resolv|gcc_s|stdc\+\+)\.so\.[0-9]+$/ &&
                    name !~ /^ld-linux[^\/]*\.so\.[0-9]+$/) bad = 1
            }
            END { exit bad }
        '
        ;;
    *) echo 'Unsupported operating system' >&2; exit 1 ;;
esac
