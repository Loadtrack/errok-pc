from pathlib import Path
from zipfile import ZIP_DEFLATED, ZipFile

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "errok-pc-source.zip"
ROOT_FILES = [
    ".gitignore",
    "README.md",
    "api-reference.txt",
    "task.txt",
    "index.html",
    "package.json",
    "package-lock.json",
    "playwright.config.ts",
    "tsconfig.json",
    "vite.config.ts",
]
DIRECTORIES = [".github", "fixtures", "public", "scripts", "shared", "src", "src-tauri", "tests"]

with ZipFile(OUTPUT, "w", ZIP_DEFLATED, compresslevel=9) as archive:
    for name in ROOT_FILES:
        path = ROOT / name
        archive.write(path, Path("errok-pc") / name)
    for directory in DIRECTORIES:
        for path in (ROOT / directory).rglob("*"):
            excluded = {"__pycache__", "target", "node_modules", "dist"}
            if path.is_file() and not excluded.intersection(path.parts):
                archive.write(path, Path("errok-pc") / path.relative_to(ROOT))

print(f"Архив исходников: {OUTPUT} ({OUTPUT.stat().st_size} байт)")
