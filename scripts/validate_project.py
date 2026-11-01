import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read_json(relative: str):
    return json.loads((ROOT / relative).read_text(encoding="utf-8"))


for relative in [
    "package.json",
    "src-tauri/tauri.conf.json",
    "src-tauri/capabilities/default.json",
    "shared/bells.json",
    "fixtures/opec-available.json",
    "fixtures/opec-catalog.json",
    "fixtures/opec-day.json",
    "fixtures/omaep-available.json",
    "fixtures/omaep-day.json",
]:
    read_json(relative)

bells = read_json("shared/bells.json")
assert len(bells["opec"]) == 6
assert len(bells["omaep"]) == 6
assert bells["special"]["opec"]["01.09.2026"][0] == ["13:00", "13:50"]

opec_day = read_json("fixtures/opec-day.json")["days"]["02.11.2026"]
group = next(item for item in opec_day["groups"] if item["name"] == "БП2-113")
assert [(lesson["pair"], lesson["subgroups"][0]["room"]) for lesson in group["lessons"]] == [(3, "С.з 3к"), (4, "547")]
assert group["lessons"][1]["subgroups"][0]["teacher"] == "Рудакова О.И."
assert opec_day["ov"]["statuses"]["БП2-113"] == "учеб.пр.+теорет.обуч."

omaep_day = read_json("fixtures/omaep-day.json")
assert len(omaep_day["groups"]) == 91

conditional = (ROOT / "fixtures/opec-conditional.headers").read_text(encoding="utf-8")
assert conditional.startswith("HTTP/1.1 304")

for path in (ROOT / "src-tauri/src").rglob("*.rs"):
    text = path.read_text(encoding="utf-8")
    for forbidden in ["api/tg.php", "api/token.php", "q/data.php", "api/parser_bridge.php", "api/site_overlay.php"]:
        assert forbidden not in text, f"Запрещённый адрес в {path}: {forbidden}"

print("JSON, звонки, контрольный день БП2-113, 91 группа ОмАЭиП и ответ 304 проверены.")
