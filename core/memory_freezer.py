#!/usr/bin/env python3
"""
=============================================================================
MEGADAEMON MEMORY FREEZER & TAINT INJECTOR (v6.0 Apex)
=============================================================================
Thread-safe real-time memory freeze engine. Maintains active locks on specific
RAM offsets, SHM buffers, or process memory (/proc/<pid>/mem) at 60Hz.
=============================================================================
"""

import os
import sys
import time
import threading
from typing import Dict, List, Any, Optional

class MemoryLock:
    def __init__(self, lock_id: str, target: str, offset: int, value: bytes, description: str = ""):
        self.lock_id = lock_id
        self.target = target  # Path to SHM file or file path
        self.offset = offset
        self.value = value
        self.description = description
        self.active = True
        self.write_count = 0
        self.last_write = 0.0

    def to_dict(self) -> Dict[str, Any]:
        return {
            "lock_id": self.lock_id,
            "target": self.target,
            "offset": f"0x{self.offset:08X}",
            "value_hex": self.value.hex(),
            "description": self.description,
            "active": self.active,
            "write_count": self.write_count,
            "last_write": self.last_write
        }

class MemoryFreezer:
    _instance = None
    _lock = threading.Lock()

    def __new__(cls):
        with cls._lock:
            if cls._instance is None:
                cls._instance = super(MemoryFreezer, cls).__new__(cls)
                cls._instance._init()
            return cls._instance

    def _init(self):
        self.locks: Dict[str, MemoryLock] = {}
        self.running = True
        self.worker_thread = threading.Thread(target=self._freeze_loop, daemon=True, name="MemoryFreezerLoop")
        self.worker_thread.start()

    def add_lock(self, lock_id: str, target: str, offset: int, value_bytes: bytes, description: str = "") -> Dict[str, Any]:
        with self._lock:
            lock = MemoryLock(lock_id, target, offset, value_bytes, description)
            self.locks[lock_id] = lock
            return {"status": "ok", "message": f"Lock '{lock_id}' added", "lock": lock.to_dict()}

    def remove_lock(self, lock_id: str) -> Dict[str, Any]:
        with self._lock:
            if lock_id in self.locks:
                del self.locks[lock_id]
                return {"status": "ok", "message": f"Lock '{lock_id}' removed"}
            return {"status": "error", "message": f"Lock '{lock_id}' not found"}

    def toggle_lock(self, lock_id: str, active: Optional[bool] = None) -> Dict[str, Any]:
        with self._lock:
            if lock_id not in self.locks:
                return {"status": "error", "message": f"Lock '{lock_id}' not found"}
            if active is None:
                self.locks[lock_id].active = not self.locks[lock_id].active
            else:
                self.locks[lock_id].active = active
            return {"status": "ok", "lock": self.locks[lock_id].to_dict()}

    def list_locks(self) -> List[Dict[str, Any]]:
        with self._lock:
            return [l.to_dict() for l in self.locks.values()]

    def clear(self):
        with self._lock:
            self.locks.clear()

    def _freeze_loop(self):
        while self.running:
            start = time.time()
            active_locks: List[MemoryLock] = []
            with self._lock:
                active_locks = [l for l in self.locks.values() if l.active]

            for lock in active_locks:
                try:
                    if os.path.exists(lock.target):
                        with open(lock.target, "r+b") as f:
                            f.seek(lock.offset)
                            f.write(lock.value)
                            f.flush()
                            lock.write_count += 1
                            lock.last_write = time.time()
                except Exception:
                    pass

            elapsed = time.time() - start
            # Target ~60 Hz (16.6ms)
            sleep_time = max(0.005, 0.0166 - elapsed)
            time.sleep(sleep_time)

    def shutdown(self):
        self.running = False
