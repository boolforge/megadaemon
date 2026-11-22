//MdGhidra.java
//MegaDaemon Ghidra helper (headless). @category megadaemon
import java.io.BufferedReader;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.cmd.function.CreateFunctionCmd;
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileOptions;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressFactory;
import ghidra.program.model.data.StringDataType;
import ghidra.program.model.listing.CodeUnit;
import ghidra.program.model.listing.Data;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;
import ghidra.program.model.listing.Listing;
import ghidra.program.model.listing.Program;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolTable;
import ghidra.util.task.ConsoleTaskMonitor;

public class MdGhidra extends GhidraScript {

    private static final Pattern DAT_RE =
        Pattern.compile("(?:&|\\()?(DAT|LST)_([0-9a-fA-F]{4,16})");

    private AddressFactory af;
    private SymbolTable st;
    private FunctionManager fm;
    private Listing listing;
    private Program prog;
    private long imageBase = 0;
    private Map<Long, String> stringByAddr = new HashMap<Long, String>();

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 4) {
            println("MdGhidra: se esperan >=4 argumentos, llegaron " + args.length);
            return;
        }
        String mode = args[0];
        String symbolsTsv = args[1];
        String anchorsTsv = args[2];
        String outDir = args[3];
        String filter = args.length > 4 ? args[4] : "";

        af = currentProgram.getAddressFactory();
        st = currentProgram.getSymbolTable();
        fm = currentProgram.getFunctionManager();
        listing = currentProgram.getListing();
        prog = currentProgram;
        try {
            imageBase = currentProgram.getImageBase().getOffset();
        }
        catch (Exception e) {
            imageBase = 0;
        }
        new File(outDir).mkdirs();

        println("MDGHIDRA mode=" + mode + " program=" + currentProgram.getName()
            + " lang=" + currentProgram.getLanguageID()
            + " imageBase=0x" + Long.toHexString(imageBase));

        if (mode.equals("symbols")) {
            applySymbols(symbolsTsv);
        }
        else if (mode.equals("decompile")) {
            decompile(anchorsTsv, outDir, filter);
        }
        else if (mode.equals("info")) {
            programInfo(outDir);
        }
        else {
            println("MDGHIDRA: modo desconocido '" + mode + "'");
        }
        println("MDGHIDRA: DONE functions=" + fm.getFunctionCount()
            + " symbols=" + st.getNumSymbols());
    }

    /** Resuelve un offset del ELF a una direccion de Ghidra (respeta el image base). */
    private Address addrFor(long off) {
        Address a = safeAddr(off + imageBase);
        if (initialized(a)) {
            return a;
        }
        a = safeAddr(off);
        if (initialized(a)) {
            return a;
        }
        return safeAddr(off + imageBase);
    }

    private Address safeAddr(long v) {
        try {
            return toAddr(v);
        }
        catch (Throwable t) {
            return null;
        }
    }

    private boolean initialized(Address a) {
        if (a == null) {
            return false;
        }
        try {
            MemoryBlock b = currentProgram.getMemory().getBlock(a);
            return b != null && b.isInitialized();
        }
        catch (Throwable t) {
            return false;
        }
    }

    private void applySymbols(String path) throws Exception {
        long t0 = System.currentTimeMillis();
        int ok = 0, dup = 0, err = 0, skipped = 0, n = 0;
        BufferedReader br = open(path);
        String line;
        while ((line = br.readLine()) != null) {
            if (line.isEmpty()) {
                continue;
            }
            n++;
            String[] p = line.split("\t", 3);
            if (p.length < 2) {
                err++;
                continue;
            }
            Long off = parseHex(p[0]);
            if (off == null) {
                err++;
                continue;
            }
            Address addr = addrFor(off.longValue());
            if (addr == null) {
                err++;
                continue;
            }
            if (!initialized(addr)) {
                skipped++;
                continue;
            }
            String name = p[1];
            String comment = p.length > 2 ? p[2] : "";
            try {
                Function f = fm.getFunctionAt(addr);
                if (f != null) {
                    try {
                        f.setName(name, SourceType.USER_DEFINED);
                        if (comment.length() > 0) {
                            f.setComment(comment.replace(" | ", "\n"));
                        }
                        ok++;
                    }
                    catch (Exception e) {
                        dup++;
                    }
                    continue;
                }
                Symbol ex = st.getPrimarySymbol(addr);
                if (ex != null && ex.getName().equals(name)) {
                    if (comment.length() > 0) {
                        listing.setComment(addr, CodeUnit.PLATE_COMMENT,
                            comment.replace(" | ", "\n"));
                    }
                    ok++;
                    continue;
                }
                st.createLabel(addr, name, SourceType.USER_DEFINED);
                if (comment.length() > 0) {
                    listing.setComment(addr, CodeUnit.PLATE_COMMENT,
                        comment.replace(" | ", "\n"));
                }
                ok++;
            }
            catch (Exception e) {
                dup++;
            }
            if ((n % 20000) == 0) {
                println("MDGHIDRA: symbols " + n + " ok=" + ok + " dup=" + dup
                    + " elapsed=" + (System.currentTimeMillis() - t0) + "ms");
            }
        }
        br.close();
        println("MDGHIDRA: symbols done rows=" + n + " ok=" + ok + " dup=" + dup
            + " err=" + err + " skipped=" + skipped
            + " elapsed=" + (System.currentTimeMillis() - t0) + "ms");
    }

    private void decompile(String anchorsTsv, String outDir, String filter) throws Exception {
        DecompInterface ifc = new DecompInterface();
        ifc.setOptions(new DecompileOptions());
        ifc.toggleCCode(true);
        ifc.toggleSyntaxTree(true);
        ifc.setSimplificationStyle("decompile");
        if (!ifc.openProgram(currentProgram)) {
            println("MDGHIDRA: ERROR no se pudo abrir el programa en el decompilador");
            return;
        }
        PrintWriter rep = new PrintWriter(new OutputStreamWriter(
            new FileOutputStream(new File(outDir, "_decompile_report.tsv")), StandardCharsets.UTF_8));
        rep.println("addr\tname\torig\tloc\ttag\tghidra_name\tsig\tbody\tdec_ok\tdec_err\tout");

        BufferedReader br = open(anchorsTsv);
        String line;
        int idx = 0;
        while ((line = br.readLine()) != null) {
            if (line.isEmpty()) {
                continue;
            }
            String[] p = line.split("\t", 5);
            if (p.length < 3) {
                continue;
            }
            idx++;
            String name = p[1];
            if (filter != null && filter.length() > 0 && !name.contains(filter)) {
                continue;
            }
            String orig = p[2];
            String loc = p.length > 3 ? p[3] : "";
            String tag = p.length > 4 ? p[4] : "anchor";
            Long off = parseHex(p[0]);
            if (off == null) {
                continue;
            }
            Address addr = addrFor(off.longValue());
            if (addr == null) {
                continue;
            }
            String gname = "";
            String sig = "";
            String err = "";
            long body = 0;
            boolean ok = false;
            String outFile = "";
            try {
                DisassembleCommand dc = new DisassembleCommand(addr,
                    (ghidra.program.model.address.AddressSetView) null, true);
                dc.applyTo(currentProgram, monitor);
                Function f = fm.getFunctionAt(addr);
                if (f == null) {
                    CreateFunctionCmd cfc = new CreateFunctionCmd(addr);
                    cfc.applyTo(currentProgram, monitor);
                    f = fm.getFunctionAt(addr);
                }
                if (f != null) {
                    gname = f.getName();
                    body = f.getBody().getNumAddresses();
                    try {
                        if (!f.getName().equals(name)) {
                            f.setName(name, SourceType.USER_DEFINED);
                            gname = f.getName();
                        }
                    }
                    catch (Exception e) {
                    }
                    DecompileResults r = ifc.decompileFunction(f, 120, monitor);
                    if (r != null && r.decompileCompleted() && r.getDecompiledFunction() != null) {
                        sig = r.getDecompiledFunction().getSignature();
                        String c = r.getDecompiledFunction().getC();
                        outFile = write(outDir, tag, name, f, orig, loc, sig, c);
                        ok = true;
                    }
                    else if (r != null) {
                        err = String.valueOf(r.getErrorMessage());
                    }
                }
                else {
                    err = "no-function-created";
                }
            }
            catch (Throwable t) {
                err = String.valueOf(t.toString());
            }
            rep.println(p[0] + "\t" + name + "\t" + orig + "\t" + loc + "\t" + tag + "\t" + gname
                + "\t" + sig.replace("\t", " ") + "\t" + body + "\t" + ok + "\t"
                + err.replace("\t", " ").replace("\n", " ") + "\t" + outFile);
            rep.flush();
            println("MDGHIDRA: anchor " + idx + " " + name + " dec=" + ok
                + (err.isEmpty() ? "" : (" err=" + err)));
        }
        br.close();
        rep.close();
        ifc.dispose();
    }

    private void programInfo(String outDir) throws Exception {
        PrintWriter pw = new PrintWriter(new OutputStreamWriter(
            new FileOutputStream(new File(outDir, "_program_info.txt")), StandardCharsets.UTF_8));
        pw.println("program=" + currentProgram.getName());
        pw.println("language=" + currentProgram.getLanguageID());
        pw.println("compiler=" + currentProgram.getCompilerSpec().getCompilerSpecID());
        pw.println("image_base=" + currentProgram.getImageBase());
        pw.println("functions=" + fm.getFunctionCount());
        pw.println("symbols=" + st.getNumSymbols());
        pw.println("--- memory blocks ---");
        for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
            pw.println(b.getName() + "\t" + b.getStart() + "\t" + b.getEnd() + "\t"
                + b.getSize() + "\t" + (b.isRead() ? "r" : "-") + (b.isWrite() ? "w" : "-")
                + (b.isExecute() ? "x" : "-") + "\t" + b.getSourceName());
        }
        pw.println("--- external symbols (imports) ---");
        for (Symbol s : st.getExternalSymbols()) {
            pw.println(s.getName());
        }
        pw.close();
        println("MDGHIDRA: info written to " + outDir + "/_program_info.txt");
    }

    private String write(String outDir, String tag, String name, Function f, String orig,
            String loc, String sig, String c) throws Exception {
        String safe = name.replaceAll("[^A-Za-z0-9._-]", "_");
        if (safe.length() > 150) {
            safe = safe.substring(0, 150);
        }
        String base = tag + "__" + safe;
        PrintWriter pw = new PrintWriter(new OutputStreamWriter(
            new FileOutputStream(new File(outDir, base + ".c")), StandardCharsets.UTF_8));
        pw.println("// " + tag + "  " + name);
        pw.println("// original Go symbol : " + orig);
        pw.println("// source location    : " + loc);
        pw.println("// entry address      : " + f.getEntryPoint());
        pw.println("// ghidra function    : " + f.getName());
        pw.println("// body instructions  : " + f.getBody().getNumAddresses());
        pw.println("// signature          : " + (sig.isEmpty() ? "(none)" : sig));
        pw.println("");
        pw.println(c);
        pw.close();
        return base + ".c";
    }

    private static Long parseHex(String s) {
        String t = s.trim();
        if (t.startsWith("0x") || t.startsWith("0X")) {
            t = t.substring(2);
        }
        if (t.isEmpty()) {
            return null;
        }
        try {
            return Long.valueOf(Long.parseUnsignedLong(t, 16));
        }
        catch (Exception e) {
            return null;
        }
    }

    private BufferedReader open(String path) throws Exception {
        return new BufferedReader(new java.io.InputStreamReader(
            new FileInputStream(path), StandardCharsets.UTF_8), 1 << 20);
    }
}
