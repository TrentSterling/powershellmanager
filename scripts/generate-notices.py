"""Collect shipped Windows dependency notices from the locked Cargo sources.

Run after `cargo fetch --locked --target x86_64-pc-windows-msvc`.
No network access, credentials, or machine-specific paths enter the output.
"""
import json
from pathlib import Path
import subprocess
import re

ROOT = Path(__file__).resolve().parents[1]
metadata = json.loads(subprocess.check_output([
    "cargo", "metadata", "--locked", "--offline", "--format-version", "1",
    "--filter-platform", "x86_64-pc-windows-msvc",
], cwd=ROOT))
sections = ["PowerShellManager third-party notices\n\n"
            "These components retain their upstream licenses. The PowerShellManager Commons\n"
            "Clause does not restrict these components when used independently.\n"
            "For OR alternatives, PowerShellManager uses MIT where available, otherwise\n"
            "Apache-2.0 where available. AND requirements remain cumulative.\n"
            "This inventory includes Windows build and test dependencies.\n"]
missing = []
fallbacks = {
    "accesskit_consumer":"accesskit", "accesskit_windows":"accesskit", "accesskit_winit":"accesskit",
    "codespan-reporting":"codespan", "egui-wgpu":"egui", "egui_glow":"egui", "fax":"fax", "fax_derive":"fax",
    "gpu-alloc":"gpu-alloc", "gpu-alloc-types":"gpu-alloc", "naga":"naga", "zune-core":"zune", "zune-jpeg":"zune",
    "accesskit": "accesskit", "clipboard-win": "clipboard-win",
    "ecolor": "egui", "eframe": "egui", "egui": "egui", "egui-winit": "egui",
    "egui_extras": "egui", "emath": "egui", "epaint": "egui",
    "gl_generator": "gl-rs", "khronos_api": "gl-rs",
    "gpu-descriptor": "gpu-descriptor", "gpu-descriptor-types": "gpu-descriptor",
    "hexf-parse": "hexf", "profiling": "profiling", "spirv": "rspirv",
}
for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
    if package["name"] == "powershellmanager":
        continue
    root = Path(package["manifest_path"]).parent
    files = sorted(p for p in root.rglob("*") if p.is_file()
                   and (p.name.lower().startswith(("license", "licence", "copying", "notice", "ofl", "ufl"))
                        or any(parent.name.lower() == "licenses" for parent in p.parents)
                        or (package["name"] == "epaint_default_fonts" and p.suffix == ".txt"))
                   and p.suffix.lower() not in (".rs", ".html", ".json"))
    if not files and package["name"] in fallbacks:
        files = [ROOT / "docs/licenses" / (fallbacks[package["name"]] + ".txt")]
    if not files:
        missing.append(package["name"])
    header = f"\n{'=' * 72}\n{package['name']} {package['version']}\nUpstream license: {package.get('license', 'See notices')}\n"
    sections.append(header)
    if package.get("authors"):
        sections.append("Package authors: " + "; ".join(package["authors"]))
    # REUSE projects may put only a template in LICENSES/MIT.txt and keep the
    # actual copyright owners in per-file SPDX headers. Preserve those too.
    copyright_lines = set()
    for source in root.rglob("*"):
        if not source.is_file() or source.suffix.lower() not in (".rs", ".c", ".h", ".cpp", ".md", ".license", ".toml"):
            continue
        with source.open(encoding="utf-8", errors="replace") as stream:
            for _, line in zip(range(100), stream):
                if re.match(r"\s*(?://|#|/\*|\*|<!--)?\s*(SPDX-FileCopyrightText:|Copyright\s+(?:\(c\)|©|[12][0-9]{3}))", line, re.I):
                    copyright_lines.add(line.strip().lstrip("/*# ").rstrip("*/ "))
    if copyright_lines:
        sections.append("Source copyright notices:\n" + "\n".join(sorted(copyright_lines)))
    for path in files:
        if "gpl" in path.name.lower() and "Apache-2.0 OR GPL" in (package.get("license") or ""):
            continue
        label = path.relative_to(root).as_posix() if path.is_relative_to(root) else path.name
        sections.append(f"\n--- {label} ---\n" + path.read_text(encoding="utf-8", errors="replace"))
for name, path in [
    ("Rajdhani font", ROOT / "assets/fonts/OFL.txt"),
    ("Trontop theme engine", ROOT / "assets/theme-engine-NOTICE"),
    ("Trontop theme engine license", ROOT / "assets/theme-engine-LICENSE"),
]:
    sections.append(f"\n{'=' * 72}\n{name}\n\n" + path.read_text(encoding="utf-8"))
if missing:
    raise SystemExit("Missing license files: " + ", ".join(missing))
text = "\n".join(line.rstrip() for line in "\n".join(sections).splitlines()) + "\n"
(ROOT / "THIRD_PARTY_NOTICES.txt").write_text(text, encoding="utf-8", newline="\n")
print(f"Wrote notices for {len(metadata['packages']) - 1} resolved dependencies and bundled references/fonts.")
