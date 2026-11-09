"""Download what's needed to compile Stray Capes: the Minecraft client jar and libraries,
plus Fabric Loader and Mixin. Usage: fetch_deps.py <minecraft-version> <out-dir>"""
import json
import os
import sys
import urllib.request

MANIFEST = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"
FABRIC = "https://meta.fabricmc.net/v2"


def get(url):
    req = urllib.request.Request(url, headers={"User-Agent": "StrayClient-build"})
    with urllib.request.urlopen(req, timeout=120) as r:
        return r.read()


def save(url, path):
    if os.path.exists(path):
        return
    os.makedirs(os.path.dirname(path), exist_ok=True)
    data = get(url)
    with open(path + ".part", "wb") as f:
        f.write(data)
    os.replace(path + ".part", path)


def maven_path(name):
    group, artifact, version = name.split(":")[:3]
    return f"{group.replace('.', '/')}/{artifact}/{version}/{artifact}-{version}.jar"


def main():
    game, out = sys.argv[1], sys.argv[2]
    libs = os.path.join(out, "libraries")
    manifest = json.loads(get(MANIFEST))
    entry = next(v for v in manifest["versions"] if v["id"] == game)
    version = json.loads(get(entry["url"]))
    save(version["downloads"]["client"]["url"], os.path.join(out, "client.jar"))
    for lib in version["libraries"]:
        art = lib.get("downloads", {}).get("artifact")
        if art and "natives" not in art["path"]:
            save(art["url"], os.path.join(libs, art["path"]))
    loader = next(l for l in json.loads(get(f"{FABRIC}/versions/loader/{game}")) if l["loader"]["stable"])
    profile = json.loads(get(f"{FABRIC}/versions/loader/{game}/{loader['loader']['version']}/profile/json"))
    for lib in profile["libraries"]:
        rel = maven_path(lib["name"])
        save(lib["url"].rstrip("/") + "/" + rel, os.path.join(libs, rel))
    print(f"Minecraft {game} + Fabric Loader {loader['loader']['version']} → {out}")


if __name__ == "__main__":
    main()
