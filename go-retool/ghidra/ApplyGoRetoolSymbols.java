// Aplica los simbolos exportados por go-retool a un programa de Ghidra y
// descompila un conjunto de funciones de interes.
//
// Uso (headless):
//   analyzeHeadless <proyecto> Go -process liblanguage_server.so -noanalysis \
//     -scriptPath <dir> -postScript ApplyGoRetoolSymbols.java <modo> <tsv> ...
//
// Argumentos:
//   0  modo: "symbols" | "decompile" | "both"
//   1  TSV de simbolos (export de go-retool):
//       0x<va>\t<nombre real>\t<fichero>:<linea>\t0x<va_fin>\t<paquete>
//   2  TSV de anclas (opcional si el modo no es "decompile"):
//       0x<va>\t<nombre>\t<fichero>:<linea>\t<etiqueta>
//   3  directorio de salida (un .c por funcion)
//   4  TSV de cadenas (opcional): "0x<addr>\t<texto>"
//   5  TSV de informe (opcional): una linea por ancla
//
// NOTAS MEDIDAS (2026-11-22, liblanguage_server.so de Antigravity):
// * El ELF es PIE: Ghidra lo carga con image base 0x100000, asi que la
//   direccion del simbolo es `offset + imageBase`.  El script prueba las dos
//   formas y se queda con la que cae en memoria inicializada.
// * Los nombres que exporta go-retool son los REALES de Go
//   (`google3/third_party/jetski/gateway/gateway.(*AdminSettings).Validate`).
//   Ghidra puede rechazar caracteres en un nombre de simbolo: si pasa, se
//   deja el nombre real; si no, se deja un nombre saneado y el nombre real
//   va en el comentario de placa.  El informe dice cual de los dos se uso.
//
//@category go-retool
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

public class ApplyGoRetoolSymbols extends GhidraScript {

    /** Direcciones de datos que el decompilador emite como DAT_/LST_/UNK_/SUB_. */
    private static final Pattern DAT_RE =
        Pattern.compile("(?:&|\\()?(DAT|LST|UNK|SUB)_([0-9a-fA-F]{4,16})");

    private AddressFactory af;
    private SymbolTable st;
    private FunctionManager fm;
    private Listing listing;
    private Program prog;
    private long imageBase = 0;

    private Map<Long, String> stringByAddr = new HashMap<Long, String>();
    private Map<String, String> resolvedStrings = new LinkedHashMap<String, String>();
    private int stringsDefined = 0;

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 4) {
            println("ApplyGoRetoolSymbols: need >=4 args, got " + args.length);
            return;
        }
        String mode = args[0];
        String symbolsTsv = args[1];
        String anchorsTsv = args[2];
        String outDir = args[3];
        String stringsTsv = args.length > 4 ? args[4] : "";
        String reportTsv = args.length > 5 ? args[5] : "";

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

        println("GOSYM: mode=" + mode + " program=" + currentProgram.getName() +
            " lang=" + currentProgram.getLanguageID() + " imageBase=0x" +
            Long.toHexString(imageBase));

        if (stringsTsv != null && stringsTsv.length() > 0) {
            loadStrings(stringsTsv);
            println("GOSYM: string table loaded, entries=" + stringByAddr.size());
        }

        if (mode.equals("symbols") || mode.equals("both")) {
            applySymbols(symbolsTsv);
        }

        if (mode.equals("decompile") || mode.equals("both")) {
            decompileAnchors(anchorsTsv, outDir, reportTsv);
        }

        println("GOSYM: DONE functions=" + fm.getFunctionCount() +
            " symbols=" + st.getNumSymbols() +
            " stringsDefined=" + stringsDefined);
    }

    // ------------------------------------------------------------------ direcciones

    /** Resuelve un offset del ELF a una direccion de Ghidra (con image base). */
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

    // ------------------------------------------------------------------ simbolos

    /**
     * Crea una etiqueta (o renombra la funcion) en cada VA del export.
     *
     * El nombre REAL de Go se intenta tal cual; si Ghidra lo rechaza se usa
     * un nombre saneado y el nombre real queda en el comentario de placa.
     */
    private void applySymbols(String path) throws Exception {
        long t0 = System.currentTimeMillis();
        int conNombreReal = 0;
        int conNombreSaneado = 0;
        int err = 0;
        int skipped = 0;
        long n = 0;
        BufferedReader br = openReader(path);
        String line;
        while ((line = br.readLine()) != null) {
            if (line.isEmpty()) {
                continue;
            }
            n++;
            String[] p = line.split("\t", 5);
            if (p.length < 3) {
                err++;
                continue;
            }
            Long off = parseHex(p[0]);
            if (off == null) {
                err++;
                continue;
            }
            Address a = addrFor(off.longValue());
            if (a == null || !initialized(a)) {
                skipped++;
                continue;
            }
            String nombre = p[1];
            String loc = p[2];
            String comentario = "go-retool: " + nombre +
                (loc != null && !loc.isEmpty() && !"?".equals(loc) ? (" | " + loc) : "");
            boolean ok = false;
            try {
                Function f = fm.getFunctionAt(a);
                if (f != null) {
                    try {
                        f.setName(nombre, SourceType.USER_DEFINED);
                        ok = true;
                    }
                    catch (Exception e) {
                        String saneado = sanea(nombre);
                        try {
                            f.setName(saneado, SourceType.USER_DEFINED);
                            ok = true;
                            comentario = comentario + " | nombre real: " + nombre;
                        }
                        catch (Exception e2) {
                            ok = false;
                        }
                    }
                }
                else {
                    Symbol existing = st.getPrimarySymbol(a);
                    if (existing != null && existing.getName().equals(nombre)) {
                        ok = true;
                    }
                    else {
                        try {
                            st.createLabel(a, nombre, SourceType.USER_DEFINED);
                            ok = true;
                        }
                        catch (Exception e) {
                            try {
                                st.createLabel(a, sanea(nombre), SourceType.USER_DEFINED);
                                ok = true;
                                comentario = comentario + " | nombre real: " + nombre;
                            }
                            catch (Exception e2) {
                                ok = false;
                            }
                        }
                    }
                }
                if (ok) {
                    try {
                        listing.setComment(a, CodeUnit.PLATE_COMMENT, comentario);
                    }
                    catch (Exception e) {
                        /* comentario es informativo: no es critico */
                    }
                    if (comentario.contains("nombre real:")) {
                        conNombreSaneado++;
                    }
                    else {
                        conNombreReal++;
                    }
                }
                else {
                    err++;
                }
            }
            catch (Exception e) {
                err++;
            }
            if ((n % 20000) == 0) {
                println("GOSYM: symbols progress " + n + " real=" + conNombreReal +
                    " saneado=" + conNombreSaneado + " err=" + err + " skipped=" + skipped +
                    " elapsed=" + (System.currentTimeMillis() - t0) + "ms");
            }
        }
        br.close();
        println("GOSYM: symbols done rows=" + n + " nombre_real=" + conNombreReal +
            " nombre_saneado=" + conNombreSaneado + " err=" + err + " skipped=" + skipped +
            " elapsed=" + (System.currentTimeMillis() - t0) + "ms");
    }

    /** Nombre de simbolo sin los caracteres que Ghidra suele rechazar. */
    private static String sanea(String nombre) {
        StringBuilder sb = new StringBuilder();
        for (char c : nombre.toCharArray()) {
            boolean ok = Character.isLetterOrDigit(c) || c == '_' || c == '.' || c == '-';
            sb.append(ok ? c : '_');
        }
        return sb.toString();
    }

    // ------------------------------------------------------------------ cadenas

    private void loadStrings(String path) throws Exception {
        BufferedReader br = openReader(path);
        String line;
        int skipped = 0;
        while ((line = br.readLine()) != null) {
            if (line.isEmpty()) {
                continue;
            }
            int sp = line.indexOf(' ');
            if (sp < 0) {
                skipped++;
                continue;
            }
            String addrTok = line.substring(0, sp);
            String rest = line.substring(sp + 1).trim();
            int sp2 = rest.indexOf(' ');
            String text = (sp2 < 0) ? rest : rest.substring(sp2 + 1);
            Long a = parseHex(addrTok);
            if (a == null) {
                skipped++;
                continue;
            }
            if (text.length() > 400) {
                text = text.substring(0, 400) + "...";
            }
            stringByAddr.put(a, text);
        }
        br.close();
        if (skipped > 0) {
            println("GOSYM: string parse skipped=" + skipped);
        }
    }

    // ------------------------------------------------------------------ anclas

    private void decompileAnchors(String anchorsTsv, String outDir, String reportTsv)
            throws Exception {
        DecompInterface ifc = new DecompInterface();
        DecompileOptions opts = new DecompileOptions();
        ifc.setOptions(opts);
        ifc.toggleCCode(true);
        ifc.toggleSyntaxTree(true);
        ifc.setSimplificationStyle("decompile");
        if (!ifc.openProgram(prog)) {
            println("GOSYM: ERROR could not open program in decompiler");
            return;
        }

        BufferedReader br = openReader(anchorsTsv);
        PrintWriter rep = null;
        if (reportTsv != null && reportTsv.length() > 0) {
            rep = new PrintWriter(new OutputStreamWriter(
                new FileOutputStream(reportTsv), StandardCharsets.UTF_8));
            rep.println("addr\tname\tloc\ttag\tghidra_name\tsig\tbody_size\t" +
                "dec_ok\tdec_err\tn_strings\tout_file");
        }

        String line;
        int idx = 0;
        while ((line = br.readLine()) != null) {
            if (line.isEmpty()) {
                continue;
            }
            String[] p = line.split("\t", 4);
            if (p.length < 4) {
                continue;
            }
            idx++;
            String addrHex = p[0];
            String goName = p[1];
            String loc = p[2];
            String tag = p[3];
            Long off = parseHex(addrHex);
            Address a = off == null ? null : addrFor(off.longValue());

            String ghidraName = "";
            String sig = "";
            String decErr = "";
            long bodySize = 0;
            int nStrings = 0;
            boolean decOk = false;
            String outFile = "";

            if (a == null) {
                decErr = "address-not-parsed";
            }
            else {
                try {
                    DisassembleCommand dc =
                        new DisassembleCommand(a, (ghidra.program.model.address.AddressSetView) null, true);
                    dc.applyTo(prog, monitor);

                    Function f = fm.getFunctionAt(a);
                    if (f == null) {
                        CreateFunctionCmd cfc = new CreateFunctionCmd(a);
                        cfc.applyTo(prog, monitor);
                        f = fm.getFunctionAt(a);
                    }
                    if (f == null) {
                        decErr = "no-function-created";
                    }
                    else {
                        ghidraName = f.getName();
                        bodySize = f.getBody().getNumAddresses();
                        if (!f.getName().equals(goName)) {
                            try {
                                f.setName(goName, SourceType.USER_DEFINED);
                                ghidraName = f.getName();
                            }
                            catch (Exception e) {
                                try {
                                    f.setName(sanea(goName), SourceType.USER_DEFINED);
                                    ghidraName = f.getName();
                                }
                                catch (Exception e2) {
                                    /* se queda el nombre automatico */
                                }
                            }
                        }

                        String c1 = null;
                        try {
                            DecompileResults r1 = ifc.decompileFunction(f, 180, monitor);
                            if (r1 != null && r1.decompileCompleted() && r1.getDecompiledFunction() != null) {
                                c1 = r1.getDecompiledFunction().getC();
                                sig = r1.getDecompiledFunction().getSignature();
                            }
                            else if (r1 != null) {
                                decErr = shortErr(r1.getErrorMessage());
                            }
                        }
                        catch (Exception e) {
                            decErr = shortErr(e.toString());
                        }

                        List<Long> dataRefs = new ArrayList<Long>();
                        if (c1 != null) {
                            collectDataRefs(c1, dataRefs);
                            nStrings = defineStrings(dataRefs);
                        }

                        String c = c1;
                        if (nStrings > 0) {
                            try {
                                DecompileResults r2 = ifc.decompileFunction(f, 180, monitor);
                                if (r2 != null && r2.decompileCompleted() && r2.getDecompiledFunction() != null) {
                                    c = r2.getDecompiledFunction().getC();
                                    sig = r2.getDecompiledFunction().getSignature();
                                }
                            }
                            catch (Exception e) {
                                /* se queda la primera pasada */
                            }
                        }

                        if (c != null && c.length() > 0) {
                            decOk = true;
                            outFile = writeFunctionFile(outDir, tag, goName, loc, f, sig, c, dataRefs);
                        }
                        else if (decErr.isEmpty()) {
                            decErr = "empty-decompilation";
                        }
                    }
                }
                catch (Throwable t) {
                    decErr = shortErr(t.toString());
                }
            }

            if (rep != null) {
                rep.println(join("\t", addrHex, goName, loc, tag, ghidraName, sig,
                    Long.toString(bodySize), Boolean.toString(decOk),
                    decErr.replace("\t", " "), Integer.toString(nStrings), outFile));
                rep.flush();
            }
            println("GOSYM: anchor " + idx + " " + goName + " dec=" + decOk +
                " strings=" + nStrings + (decErr.isEmpty() ? "" : (" err=" + decErr)));
        }
        br.close();
        if (rep != null) {
            rep.close();
        }
        ifc.dispose();
    }

    private void collectDataRefs(String c, List<Long> out) {
        Matcher m = DAT_RE.matcher(c);
        while (m.find()) {
            Long v = parseHex(m.group(2));
            if (v != null) {
                out.add(v);
            }
        }
    }

    /**
     * Crea las cadenas que el decompilador referencia, para que la segunda
     * pasada las muestre en linea.  La direccion que emite Ghidra esta en su
     * espacio de direcciones (offset + image base): se prueban las dos.
     */
    private int defineStrings(List<Long> refs) {
        int made = 0;
        for (Long r : refs) {
            Address a = safeAddr(r.longValue());
            if (a == null || !initialized(a)) {
                continue;
            }
            String text = null;
            Long key = r;
            String fromTable = stringByAddr.get(r);
            if (fromTable == null && r.longValue() >= imageBase) {
                Long alt = Long.valueOf(r.longValue() - imageBase);
                fromTable = stringByAddr.get(alt);
                if (fromTable != null) {
                    key = alt;
                }
            }
            if (fromTable != null) {
                text = fromTable;
            }
            else {
                text = readPrintable(a);
            }
            if (text == null || text.length() < 4) {
                continue;
            }
            resolvedStrings.put(String.format("0x%016x", r.longValue()), text);
            try {
                Data d = listing.getDefinedDataAt(a);
                if (d != null) {
                    continue;
                }
                int len = text.length();
                if (len > 200) {
                    len = 200;
                }
                listing.createData(a, StringDataType.dataType, len);
                stringsDefined++;
                made++;
            }
            catch (Exception e) {
                /* conflicto: se deja como esta */
            }
        }
        return made;
    }

    /** Lee bytes imprimibles desde la direccion (max 512). */
    private String readPrintable(Address a) {
        try {
            byte[] buf = new byte[512];
            int n = currentProgram.getMemory().getBytes(a, buf, 0, buf.length);
            if (n <= 0) {
                return null;
            }
            StringBuilder sb = new StringBuilder();
            for (int i = 0; i < n; i++) {
                int c = buf[i] & 0xff;
                if (c < 0x20 || c > 0x7e) {
                    break;
                }
                sb.append((char) c);
            }
            return sb.length() > 0 ? sb.toString() : null;
        }
        catch (Throwable t) {
            return null;
        }
    }

    // ------------------------------------------------------------------ utilidades

    private String writeFunctionFile(String outDir, String tag, String goName, String loc,
            Function f, String sig, String c, List<Long> refs) throws Exception {
        String safe = sanea(tag.isEmpty() ? goName : tag);
        File out = new File(outDir, safe + ".c");
        PrintWriter w = new PrintWriter(new OutputStreamWriter(
            new FileOutputStream(out), StandardCharsets.UTF_8));
        w.println("// " + goName);
        w.println("// fichero:linea: " + loc);
        w.println("// VA (ELF): " + f.getEntryPoint());
        w.println("// tamano: " + f.getBody().getNumAddresses() + " bytes");
        w.println("// firma: " + sig);
        w.println();
        w.println(c);
        w.println();
        w.println("// --- cadenas referenciadas ---");
        for (Long r : refs) {
            String t = resolvedStrings.get(String.format("0x%016x", r.longValue()));
            if (t != null) {
                w.println("// 0x" + Long.toHexString(r.longValue()) + " : " + t);
            }
        }
        w.close();
        return out.getPath();
    }

    private BufferedReader openReader(String path) throws Exception {
        File f = new File(path);
        if (!f.exists()) {
            throw new java.io.FileNotFoundException(path);
        }
        return new BufferedReader(new java.io.InputStreamReader(
            new FileInputStream(f), StandardCharsets.UTF_8));
    }

    private static Long parseHex(String s) {
        if (s == null) {
            return null;
        }
        String t = s.trim();
        if (t.startsWith("0x") || t.startsWith("0X")) {
            t = t.substring(2);
        }
        try {
            return Long.parseUnsignedLong(t, 16);
        }
        catch (NumberFormatException e) {
            return null;
        }
    }

    private static String shortErr(String s) {
        if (s == null) {
            return "";
        }
        String t = s.replace('\n', ' ').replace('\t', ' ');
        return t.length() > 160 ? t.substring(0, 160) : t;
    }

    private static String join(String sep, String... parts) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < parts.length; i++) {
            if (i > 0) {
                sb.append(sep);
            }
            sb.append(parts[i] == null ? "" : parts[i]);
        }
        return sb.toString();
    }
}
