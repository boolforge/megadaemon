#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""Matriz de portabilidad de los drivers propietarios: que API de kernel usa cada uno
y que hay que cambiar para llevarlo de 5.4 (Samsung) a 5.10/5.15/6.1/6.6/7.3 (mainline/GKI)."""
import json, re
from pathlib import Path

MD = Path("/workspace/output/mass_decomp")
OUT = Path("/workspace/output")

# API de kernel que cambia entre 5.4 y 7.3 (resumen operativo para reimplementar)
API = {
    "ioctl_magic": ("_IO/_IOR/_IOW/_IORW", "5.4->7.3 sin cambios; el ioctl de V4L2/VS4L se mantiene"),
    "vfs_read_write": ("read/write -> read_iter/write_iter", "5.9+: file_operations con read_iter/write_iter; cambia la firma"),
    "poll_select": ("poll/select -> poll_request", "5.9+: ->poll() con poll_request; 6.11: ->poll() cambia otra vez"),
    "mmap": ("mmap con vm_ops", "6.x: vm_flags, y read_folio/write_folio en 6.5+"),
    "drm": ("struct drm_driver / legacy ioctl", "6.x: drm_device, drm_mode_config_funcs; los ioctl legacy migran a DRM_IOCTL_*"),
    "v4l2": ("struct video_device", "6.x: media controller 2.0 (v4l2_subdev, media_pad)"),
    "input": ("struct input_dev", "6.2+: input_device_abs_set_up deprecado; 6.12+: nuevo input API"),
    "netlink": ("netlink_kernel_create", "6.3+: netlink_policies obligatorias; 6.7+: per-netns"),
    "sysfs_proc": ("proc_create / sysfs attribute_group", "6.x: sin cambios de API, pero class->device_model migracion"),
    "regmap": ("regmap_init / regmap_config", "6.x: sin cambios"),
    "clk_gpio": ("clk_prepare_enable / gpio_request", "6.x: sin cambios"),
    "i2c_spi": ("i2c_driver / spi_driver", "6.9+: i2c_driver const; 6.12: cambios en adapters"),
    "power": ("regulator / dev_pm", "6.11+: dev_pm_ops refactor"),
    "container_of": ("container_of", "sin cambios"),
    "kABI": ("struct layout de kernel structs", "el kABI de GKI cambia entre 5.10/5.15/6.1/6.6: un modulo .ko de 5.4 NO carga en 5.15"),
}

PATTERNS = [
    ("vfs", r"\b(struct file_operations|file_operations)\b"),
    ("vfs_read_write", r"\b(->read|->write)\s*=|\bread_iter\b|\bwrite_iter\b"),
    ("poll_select", r"\b(poll_request|->poll)\b"),
    ("mmap", r"\b(vm_ops|->mmap|read_folio|write_folio)\b"),
    ("drm", r"\b(drm_driver|drm_device|drm_ioctl)\b"),
    ("v4l2", r"\b(video_device|v4l2_subdev|media_pad)\b"),
    ("input", r"\b(input_dev|input_device)\b"),
    ("netlink", r"\b(netlink_kernel_create|netlink_policy)\b"),
    ("sysfs_proc", r"\b(proc_create|sysfs_create|attribute_group)\b"),
    ("regmap", r"\bregmap_(init|read|write)\b"),
    ("clk_gpio", r"\b(clk_prepare_enable|gpio_request|gpio_request_array)\b"),
    ("i2c_spi", r"\b(i2c_driver|spi_driver|i2c_adapter)\b"),
    ("power", r"\b(dev_pm_ops|regulator_enable|regulator_get)\b"),
]

def main():
    rows = []
    for c in sorted(MD.glob("*.c")):
        txt = c.read_text(errors="replace")
        hits = [k for k, pat in PATTERNS if re.search(pat, txt)]
        ioctls = len(re.findall(r"\bioctl\s*\(", txt))
        rows.append({"binario": c.name[:-2], "bytes": c.stat().st_size,
                     "llamadas_ioctl": ioctls, "apis": hits,
                     "riesgo_5.4->7.3": ("alto" if hits else "bajo")})
    rows.sort(key=lambda r: (-len(r["apis"]), -r["llamadas_ioctl"]))
    L = ["# Matriz de portabilidad: drivers proprietary (5.4 Samsung) -> 5.10/5.15/6.1/6.6/7.3", "",
         "Generado con `tools/driver_port_matrix.py` sobre el C decompilado de `/workspace/output/mass_decomp`",
         "(el C decompilado NO se versiona).", "",
         "## Por que no vale con solo recompilar", "",
         "| API | Cambio relevante |", "|---|---|"]
    for k, (a, b) in API.items():
        L.append("| `%s` | %s: %s |" % (k, a, b))
    L += ["", "## Clasificacion de los drivers por superficie de API", "",
          "| driver | bytes | ioctls | APIs de kernel usadas | riesgo |", "|---|---|---|---|---|"]
    for r in rows[:120]:
        L.append("| `%s` | %d | %d | %s | %s |" % (r["binario"], r["bytes"], r["llamadas_ioctl"],
                  ", ".join(r["apis"]) or "—", r["riesgo_5.4->7.3"]))
    L += ["", "## Lectura", "",
          "- Los drivers con `vfs` + `poll` + `mmap` (display, V4L2, NPU) **no** se pueden portar por "
          "recopilacion: hay que reescribirlos con la API moderna (read_iter/write_iter, poll_request, "
          "read_folio/write_folio).",
          "- Un `.ko` de 5.4 **no carga** en 5.15/6.x por el kABI de GKI: el camino es "
          "**reimplementacion en codigo abierto** (que es lo que hace este repo), no parcheo binario.",
          "- Los drivers con pocas APIs (regmap/clk/gpio/i2c) son los candidatos rapidos para el port.",
          ""]
    (OUT / "driver_port_matrix.md").write_text("\n".join(L) + "\n")
    json.dump(rows, open(OUT / "driver_port_matrix.json", "w"), ensure_ascii=False, indent=1)
    print("driver_port_matrix.md: %d drivers clasificados" % len(rows))

if __name__ == "__main__":
    main()
