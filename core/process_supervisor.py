#!/usr/bin/env python3
"""
MegaDaemon Process & Task Supervisor
Tracks lifecycle, PIDs, rolling stdout/stderr logs, and health of all background workers.
"""

import os
import sys
import time
import signal
import subprocess
import threading
from collections import deque

class ManagedProcess:
    def __init__(self, name, cmd, cwd=None):
        self.name = name
        self.cmd = cmd
        self.cwd = cwd
        self.pid = None
        self.start_time = None
        self.exit_code = None
        self.status = "PENDING"
        self.proc = None
        self.logs = deque(maxlen=100)
        self.lock = threading.Lock()

    def launch(self):
        self.start_time = time.time()
        self.status = "RUNNING"
        try:
            self.proc = subprocess.Popen(
                self.cmd,
                cwd=self.cwd,
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                text=True,
                bufsize=1
            )
            self.pid = self.proc.pid

            # Thread to consume logs into deque
            t = threading.Thread(target=self._log_consumer, daemon=True)
            t.start()
            return True
        except Exception as e:
            self.status = f"FAILED: {e}"
            self.logs.append(f"Launch error: {e}")
            return False

    def _log_consumer(self):
        try:
            for line in iter(self.proc.stdout.readline, ''):
                if not line:
                    break
                with self.lock:
                    self.logs.append(f"[{time.strftime('%H:%M:%S')}] {line.strip()}")
            self.proc.wait()
            self.exit_code = self.proc.returncode
            self.status = "FINISHED" if self.exit_code == 0 else f"EXITED({self.exit_code})"
        except Exception as e:
            self.status = f"ERROR: {e}"

    def stop(self, timeout=3.0):
        if self.proc and self.proc.poll() is None:
            try:
                self.proc.terminate()
                self.proc.wait(timeout=timeout)
            except subprocess.TimeoutExpired:
                self.proc.kill()
            self.status = "TERMINATED"
            return True
        return False

    def to_dict(self):
        with self.lock:
            recent_logs = list(self.logs)[-20:]
        return {
            "name": self.name,
            "cmd": self.cmd if isinstance(self.cmd, str) else " ".join(self.cmd),
            "pid": self.pid,
            "status": self.status,
            "exit_code": self.exit_code,
            "start_time": self.start_time,
            "uptime_sec": round(time.time() - self.start_time, 1) if self.start_time and self.status == "RUNNING" else 0,
            "recent_logs": recent_logs
        }

class ProcessSupervisor:
    def __init__(self):
        self._processes = {}
        self._lock = threading.Lock()

    def spawn(self, name, cmd, cwd=None):
        with self._lock:
            # Stop existing if still running under same name
            if name in self._processes:
                old = self._processes[name]
                if old.status == "RUNNING":
                    old.stop(timeout=1.0)
            mp = ManagedProcess(name, cmd, cwd=cwd)
            if mp.launch():
                self._processes[name] = mp
                return mp.to_dict()
            else:
                self._processes[name] = mp
                return {"error": mp.status}

    def list_all(self):
        with self._lock:
            return {name: proc.to_dict() for name, proc in self._processes.items()}

    def get(self, name):
        with self._lock:
            proc = self._processes.get(name)
            return proc.to_dict() if proc else None

    def terminate(self, name):
        with self._lock:
            proc = self._processes.get(name)
            if proc:
                return proc.stop()
            return False

supervisor = ProcessSupervisor()
