#@category GoRetool
# Import go-retool JSON function names into the currently open Ghidra program.
# Run from Script Manager after importing the target binary. Addresses are only
# created when the JSON carries an explicit, resolved VA; unresolved offsets are
# retained as comments on no address only in the external JSON, never guessed.

import json
from java.io import File
from ghidra.program.model.symbol import SourceType
from ghidra.program.model.listing import Function

chooser = askFile("Select go-retool analysis JSON", "Open")
with open(chooser.getAbsolutePath(), "r") as fh:
    doc = json.load(fh)

created = 0
renamed = 0
skipped = 0
listing = currentProgram.getListing()
symtab = currentProgram.getSymbolTable()

for module in doc.get("modules", []):
    module_id = module.get("module_identity", "module")
    for fn in module.get("functions", []):
        name = fn.get("observed_name")
        va = fn.get("start_va")
        if not name or not va:
            skipped += 1
            continue
        try:
            address = toAddr(long(va, 16) if isinstance(va, basestring) else long(va))
            if not currentProgram.getMemory().contains(address):
                skipped += 1
                continue
            safe_name = name.replace("/", "_").replace(" ", "_")
            existing = listing.getFunctionAt(address)
            if existing is None:
                fn_obj = createFunction(address, safe_name)
                if fn_obj is not None:
                    created += 1
            else:
                # Never overwrite a user-defined name silently.
                if existing.getSymbol().getSource() == SourceType.DEFAULT:
                    existing.setName(safe_name, SourceType.ANALYSIS)
                    renamed += 1
            comment = "go-retool: module=%s entryoff=0x%x funcoff=0x%x nameoff=%d" % (
                module_id, fn.get("entryoff", 0), fn.get("funcoff", 0), fn.get("nameoff", 0))
            setPlateComment(address, comment)
        except Exception as exc:
            printerr("Skipping function %s: %s" % (name, exc))

print("go-retool import complete: created=%d renamed=%d skipped=%d" % (created, renamed, skipped))
