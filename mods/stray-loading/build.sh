#!/usr/bin/env bash
# Builds stray-loading.jar (Forge + NeoForge): the Stray loading screen as a code-free
# ("lowcodefml") mod with two mixins. Usage: ./build.sh <minecraft-client.jar> <library-jars-dir> [javac]
set -euo pipefail
cd "$(dirname "$0")"
MC_JAR="$1"; LIBS="$2"; JAVAC="${3:-javac}"
CP="$MC_JAR:$(find "$LIBS" -name '*.jar' | tr '\n' ':')"
rm -rf build && mkdir -p build/classes
"$JAVAC" --release 21 -nowarn -proc:none -cp "$CP" -d build/classes $(find src -name '*.java')
cp -r resources/. build/classes/ && cp LICENSE build/classes/
mkdir -p dist
(cd build/classes && "$(dirname "$JAVAC")/jar" --create --file ../../dist/stray-loading.jar --manifest ../../MANIFEST.MF --date=2026-01-01T00:00:00Z .)
echo "built dist/stray-loading.jar"
