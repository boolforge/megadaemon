#!/usr/bin/env python3
"""
MegaDaemon Dynamic Plugin & Scripting Manager
Enables zero-downtime hot-reloading of custom reverse engineering hooks.
"""

import os
import sys
import importlib.util
import time

PLUGINS_DIR = "/data/user/0/com.agy/files/megadaemon/plugins"

class PluginRegistry:
    def __init__(self, plugins_dir=PLUGINS_DIR):
        self.plugins_dir = plugins_dir
        self.plugins = {}
        os.makedirs(self.plugins_dir, exist_ok=True)
        self.reload_all()

    def reload_all(self):
        loaded = {}
        if not os.path.exists(self.plugins_dir):
            return loaded

        for fname in os.listdir(self.plugins_dir):
            if fname.endswith(".py") and not fname.startswith("__"):
                plugin_name = fname[:-3]
                path = os.path.join(self.plugins_dir, fname)
                try:
                    spec = importlib.util.spec_from_file_location(plugin_name, path)
                    mod = importlib.util.module_from_spec(spec)
                    spec.loader.exec_module(mod)
                    if hasattr(mod, "register_plugin"):
                        meta = mod.register_plugin()
                        loaded[plugin_name] = {
                            "name": meta.get("name", plugin_name),
                            "version": meta.get("version", "1.0.0"),
                            "description": meta.get("description", ""),
                            "module": mod,
                            "path": path,
                            "loaded_at": time.time()
                        }
                except Exception as e:
                    print(f"[PLUGIN_LOAD_ERROR] Failed loading {fname}: {e}")

        self.plugins = loaded
        return {k: {"name": v["name"], "version": v["version"], "description": v["description"]} for k, v in self.plugins.items()}

    def execute_hook(self, plugin_name, hook_func, *args, **kwargs):
        if plugin_name not in self.plugins:
            return {"error": f"Plugin '{plugin_name}' not registered"}
        mod = self.plugins[plugin_name]["module"]
        if not hasattr(mod, hook_func):
            return {"error": f"Hook '{hook_func}' not found in plugin '{plugin_name}'"}
        try:
            fn = getattr(mod, hook_func)
            return fn(*args, **kwargs)
        except Exception as e:
            return {"error": f"Execution error in {plugin_name}.{hook_func}: {e}"}

plugin_registry = PluginRegistry()
