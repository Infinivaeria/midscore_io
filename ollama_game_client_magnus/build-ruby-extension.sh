#!/usr/bin/env bash
set -euo pipefail

project_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
profile="${1:-release}"

if [[ "$profile" != "debug" && "$profile" != "release" ]]; then
	printf 'Usage: %s [debug|release]\n' "$0" >&2
	exit 2
fi

if [[ "$profile" == "release" ]]; then
	cargo build --manifest-path "$project_dir/Cargo.toml" --release
else
	cargo build --manifest-path "$project_dir/Cargo.toml"
fi

artifact_dir="$project_dir/target/$profile"
case "$(uname -s)" in
	Linux) extension="so" ;;
	Darwin) extension="bundle"; library_extension="dylib" ;;
	*)
		printf 'Unsupported operating system: %s\n' "$(uname -s)" >&2
		exit 1
		;;
esac

library_extension="${library_extension:-$extension}"

cp "$artifact_dir/libollama_game_client_magnus.$library_extension" \
	"$artifact_dir/ollama_game_client_magnus.$extension"
printf 'Ruby extension: %s\n' "$artifact_dir/ollama_game_client_magnus.$extension"