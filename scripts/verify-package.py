import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import tarfile
import tomllib


def require(condition, message):
    if not condition:
        raise SystemExit(message)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def verify(tag, directory, binary):
    require(re.fullmatch(r"v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", tag), "Not a stable version tag")
    source = json.loads(Path("plugin.json").read_text())
    cargo = tomllib.loads(Path("backend/Cargo.toml").read_text())
    frontend = json.loads(Path("frontend/package.json").read_text())
    version = tag[1:]
    require(source["version"] == cargo["package"]["version"] == frontend["version"] == version, "Version mismatch")
    identity = "dean-cherry.state-observer"
    filename = f"{identity}-{version}-x86_64-unknown-linux-gnu.tar.gz"
    require({entry.name for entry in directory.iterdir()} == {filename, filename + ".sha256"}, "Unexpected release assets")
    archive_path = directory / filename
    digest = sha256(archive_path.read_bytes())
    require((directory / (filename + ".sha256")).read_text() == f"{digest}  {filename}\n", "Archive checksum or sidecar filename mismatch")

    environment = {**os.environ, "LC_ALL": "C"}
    header = subprocess.check_output(["readelf", "--file-header", "--wide", str(binary)], text=True, env=environment)
    require(re.search(r"Class:\s+ELF64\b", header), "Release binary is not ELF64")
    require(re.search(r"Machine:\s+Advanced Micro Devices X86-64\b", header), "Release binary is not x86_64")
    versions = subprocess.check_output(["readelf", "--version-info", "--wide", str(binary)], text=True, env=environment)
    required_versions = re.findall(r"Name:\s+GLIBC_([^\s]+)", versions)
    require(required_versions, "No GLIBC version references found")
    require(all(re.fullmatch(r"[0-9]+(?:\.[0-9]+)+", value) for value in required_versions), "Unexpected GLIBC version reference")
    minimum_glibc = max(tuple(map(int, value.split("."))) for value in required_versions)
    require(minimum_glibc <= (2, 28), f"GLIBC requirement exceeds 2.28: {minimum_glibc}")

    resources = {"LICENSE", "NOTICE", "web/index.html", "web/app.js", "web/app.css"}
    expected_files = resources | {"bin/plugin"}
    require(set(source["resources"]) == resources, "Unexpected source resources")
    with tarfile.open(archive_path, "r:gz") as archive:
        members = archive.getmembers()
        names = [member.name for member in members]
        require(len(names) == len(set(names)), "Duplicate archive paths")
        require(set(names) == expected_files | {"plugin.json"}, "Unexpected archive contents")
        require(all(member.isfile() for member in members), "Archive contains non-regular entries")
        require(archive.getmember("bin/plugin").mode & 0o111, "Packaged binary is not executable")
        manifest = json.loads(archive.extractfile("plugin.json").read())
        require(manifest["manifestVersion"] == 2, "Manifest version must be 2")
        require(f"{manifest['publisher']}.{manifest['name']}" == identity, "Plugin identity changed")
        require(manifest["version"] == version, "Packaged version mismatch")
        require(manifest["runtime"] == "trustedProcess", "Unexpected runtime")
        require(manifest["main"] == "bin/plugin", "Unexpected entry point")
        require(manifest["engines"] == {"codex-proxy-rs": ">=3.18.0"}, "Unexpected host compatibility")
        require("permissions" not in manifest, "Legacy permissions are forbidden")
        require(manifest.get("secretFields", []) == [], "Unexpected secret fields")
        require(len(manifest["state"]) == 2 and {
            state["namespace"]: state["schemaVersion"] for state in manifest["state"]
        } == {"settings": 1, "observations": 1}, "State schema changed")
        for field, value in source.items():
            if field != "contributes":
                require(manifest[field] == value, f"Packaged source field changed: {field}")
        contributes = manifest["contributes"]
        require(set(contributes) == {"middleware", "observer", "management"}, "Unexpected capabilities")
        for capability, capability_version, stages in [
            ("middleware", 3, ["request", "attempt"]),
            ("observer", 1, ["observation"]),
            ("management", 1, ["management"]),
        ]:
            declaration = contributes[capability]
            require(declaration["id"] == f"{identity}.{capability}", "Capability identity changed")
            require(declaration["version"] == capability_version and declaration["stages"] == stages, "Capability contract mismatch")
            for field, value in source["contributes"][capability].items():
                require(declaration[field] == value, f"Capability field changed: {capability}.{field}")
        package = manifest["package"]
        require(package["protocolVersion"] == 2, "Protocol version must be 2")
        require(package["target"] == {"os": "linux", "architecture": "x86_64"}, "Package target mismatch")
        require(set(package["files"]) == expected_files, "Unexpected package hash entries")
        for name in sorted(expected_files):
            contents = archive.extractfile(name).read()
            require(sha256(contents) == package["files"][name], f"Package hash mismatch: {name}")
            current = binary if name == "bin/plugin" else (
                Path("frontend/dist") / name.removeprefix("web/") if name.startswith("web/") else Path(name)
            )
            require(contents == current.read_bytes(), f"Package differs from current build: {name}")
    print(f"PACKAGE_VERIFIED {filename} sha256={digest} max_glibc={'.'.join(map(str, minimum_glibc))}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Verify the Linux release package against this checkout and fresh binary")
    parser.add_argument("--tag", required=True)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    arguments = parser.parse_args()
    verify(arguments.tag, arguments.directory, arguments.binary)
