import argparse
import hashlib
import urllib.request
import xml.etree.ElementTree as Et
from pathlib import Path

# Downloads the official PoB manifest file and performs the following modifications:
# - set platform and branch
# - add a new 'default' source that points to this repo
# - rename old 'default' source to 'origin'
# - remove win32 libraries and executables
# - remove font files
# - use modified UpdateCheck.lua from this repo, which is compatible with unix paths and disables the 'basic' update mode

repo = "meehl/RustyPathOfBuilding"


def is_font_file(name: str) -> bool:
    return name.endswith(".tga") or name.endswith(".tgf")


def is_tar_file(name: str) -> bool:
    return name.endswith(".tar") or name.endswith(".tgf")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--game", choices=["poe1", "poe2"], required=True)
    args = parser.parse_args()

    if args.game == "poe1":
        origin_url = "https://raw.githubusercontent.com/PathOfBuildingCommunity/PathOfBuilding/master/manifest.xml"
    else:
        origin_url = "https://raw.githubusercontent.com/PathOfBuildingCommunity/PathOfBuilding-PoE2/master/manifest.xml"

    with urllib.request.urlopen(origin_url) as f:
        manifest_root = Et.fromstring(f.read().decode("utf-8"))

    for child in list(manifest_root):
        if child.tag == "Version":
            child.set("branch", "master")
            child.set("platform", "any")
        elif child.tag == "Source":
            if child.get("part") == "default":
                child.set("part", "origin")

            if child.get("platform") is not None:
                child.attrib.pop("platform")
        elif child.tag == "File":
            if child.get("part") == "default":
                child.set("part", "origin")

            if (
                child.get("runtime") is not None
                or is_font_file(child.get("name"))
                or is_tar_file(child.get("name"))
            ):
                manifest_root.remove(child)

            if child.get("name") == "UpdateCheck.lua":
                child.set("part", "default")

                path = Path(f"../manifest/{args.game}/UpdateCheck.lua")
                data = path.read_bytes()
                sha1 = hashlib.sha1(data).hexdigest()
                child.set("sha1", sha1)

    default_source = Et.Element(
        "Source",
        {
            "part": "default",
            "url": f"https://raw.githubusercontent.com/{repo}/main/lua/{args.game}/",
        },
    )
    manifest_root.insert(1, default_source)

    tree = Et.ElementTree(manifest_root)
    Et.indent(tree, "\t")
    tree.write(f"./{args.game}/manifest.xml")


if __name__ == "__main__":
    main()
