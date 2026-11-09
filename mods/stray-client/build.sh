#!/usr/bin/env bash
# Builds stray-client.jar with plain javac against the (unobfuscated) Minecraft 26.x client.
# Usage: ./build.sh <minecraft-client.jar> <library-jars-dir> [javac]
set -euo pipefail
cd "$(dirname "$0")"
MC_JAR="$1"; LIBS="$2"; JAVAC="${3:-javac}"
CP="$MC_JAR:$(find "$LIBS" -name '*.jar' | tr '\n' ':')"
rm -rf build && mkdir -p build/classes
"$JAVAC" --release 21 -nowarn -proc:none -cp "$CP" -d build/classes $(find src -name '*.java')
cp -r resources/. build/classes/ && cp LICENSE build/classes/
mkdir -p dist
(cd build/classes && "$(dirname "$JAVAC")/jar" --create --file ../../dist/stray-client.jar --date=2026-01-01T00:00:00Z .)
echo "built dist/stray-client.jar"
