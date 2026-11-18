#!/data/user/0/com.agy/files/usr/bin/python3
"""
=============================================================================
MEGADAEMON AUTONOMOUS PRIVILEGE & SUBSYSTEM CONTROLLER
=============================================================================
Provides MegaDaemon with root, Shizuku, CPU governor tuning, RAM management,
and native Antigravity OS integration.
=============================================================================
"""

import os
import sys
import subprocess
import shutil

class PrivilegeManager:
    def __init__(self):
        self.has_magisk_root = False
        self.has_shizuku = False
        self.has_su = False
        self._detect_privileges()

    def _detect_privileges(self):
        # 1. Check Shizuku / rish
        if shutil.which("rish"):
            try:
                res = subprocess.run(["rish", "-c", "id"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=2)
                if "uid=0" in res.stdout or "uid=2000" in res.stdout:
                    self.has_shizuku = True
                    if "uid=0" in res.stdout:
                        self.has_magisk_root = True
            except Exception:
                pass

        # 2. Check native su
        if shutil.which("su"):
            try:
                res = subprocess.run(["su", "-c", "id"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=2)
                if "uid=0" in res.stdout:
                    self.has_su = True
                    self.has_magisk_root = True
            except Exception:
                pass

    def exec_root(self, cmd, timeout=5):
        """Runs command as root via rish or su."""
        if self.has_shizuku:
            return subprocess.run(["rish", "-c", cmd], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=timeout)
        elif self.has_su:
            return subprocess.run(["su", "-c", cmd], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=timeout)
        else:
            return subprocess.run(cmd, shell=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, timeout=timeout)

    def optimize_system(self):
        """Boosts performance, renices compiler, drops caches."""
        results = {}
        # 1. Drop page cache, dentries and inodes
        r1 = self.exec_root("sync; echo 3 > /proc/sys/vm/drop_caches")
        results["drop_caches"] = r1.returncode == 0

        # 2. Set CPU governor to performance on big cores if possible
        gov_cmd = "for g in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do [ -f \"$g\" ] && echo performance > \"$g\"; done"
        r2 = self.exec_root(gov_cmd)
        results["governor_boost"] = r2.returncode == 0

        # 3. Renice self
        try:
            os.nice(-10)
            results["renice"] = -10
        except Exception:
            r3 = self.exec_root(f"renice -n -10 -p {os.getpid()}")
            results["renice"] = r3.returncode == 0

        return results

    def post_notification(self, title, message):
        """Sends status bar notification via agy-notify if present."""
        if shutil.which("agy-notify"):
            try:
                subprocess.run(["agy-notify", "post", "-t", title, "-m", message], timeout=2)
                return True
            except Exception:
                pass
        return False

_GLOBAL_PRIV_MGR = None

def get_privilege_manager():
    global _GLOBAL_PRIV_MGR
    if _GLOBAL_PRIV_MGR is None:
        _GLOBAL_PRIV_MGR = PrivilegeManager()
    return _GLOBAL_PRIV_MGR

if __name__ == "__main__":
    pm = get_privilege_manager()
    print(f"Magisk Root: {pm.has_magisk_root}")
    print(f"Shizuku: {pm.has_shizuku}")
    print(f"Native SU: {pm.has_su}")
    print("Optimization results:", pm.optimize_system())
