// Importa el export de go-retool en un programa de Ghidra y lo VERIFICA.
//
// Que hace (tres modos, se elige con el primer argumento):
//
//   import <simbolos.tsv> <informe.tsv> <mapa.txt>
//       Crea una FUNCION por cada fila del export de go-retool, en la
//       direccion REAL, con el nombre REAL de Go y con el cuerpo EXACTO que
//       da la pclntab (end_va - va).  El comentario de placa lleva el
//       fichero:linea de origen y el tamano.
//       El informe dice cuantos se crearon, cuantos hubo que renombrar,
//       cuantos hubo que desambiguar (mismo nombre Go en dos funciones) y
//       cuantos fallaron.
//
//   export <funciones.tsv> <mapa.txt>
//       Vuelca la lista de FUNCIONES que tiene ahora el programa de Ghidra
//       (direccion ELF, nombre, tamano del cuerpo) para compararla con el
//       TSV de go-retool.
//
//   decompile <lista.tsv> <directorio> <informe.tsv>
//       Desensambla y descompila las funciones de la lista y escribe el
//       pseudocodigo en un .c por funcion.
//
// Formato del TSV de simbolos (7 columnas, el que produce
// `go-retool export tsv`):
//   0x<va>\t<nombre real de Go>\t<fichero>:<linea>\t0x<va_fin>\t<paquete>\t<tamano>\t<entryoff>
//
// Formato del TSV de la lista a descompilar:
//   0x<va>\t<nombre>\t<etiqueta>
//
// MEDIDO (2026-11-22, liblanguage_server.so de Antigravity):
// * El ELF es PIE y Ghidra lo carga con image base 0x100000: la direccion de
//   Ghidra es `vaddr_ELF + imageBase`.  El script lo comprueba: prueba las
//   dos formas y se queda con la que cae en memoria inicializada.
// * Los nombres reales de Go son validos como simbolo de Ghidra: el unico
//   caracter que Ghidra prohibe es el espacio (SymbolUtilities.INVALIDCHARS)
//   y el limite es de 2000 caracteres (el nombre mas largo del binario
//   real mide 1104).  Los nombres repetidos (90 nombres, 204 filas) se
//   desambiguan con el sufijo "@0x<va>" y el informe lo cuenta.
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
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;

import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.cmd.function.CreateFunctionCmd;
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileOptions;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.listing.CodeUnit;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;
import ghidra.program.model.listing.Listing;
import ghidra.program.model.listing.Program;
import ghidra.program.model.mem.MemoryBlock;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.symbol.Symbol;
import ghidra.program.model.symbol.SymbolTable;
import ghidra.program.model.symbol.SymbolUtilities;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

public class ImportGoRetoolSymbols extends GhidraScript {

    private SymbolTable st;
    private FunctionManager fm;
    private Listing listing;
    private Program prog;
    private long imageBase = 0;

    /** Llamadas del pseudocodigo a direcciones que no son entrada de funcion. */
    private static final Pattern CALL_RE = Pattern.compile("func_0x([0-9a-fA-F]{4,16})");
    /** Referencias a datos del pseudocodigo. */
    private static final Pattern DAT_RE =
        Pattern.compile("(?:&|\\()?(DAT|LST|UNK|SUB)_([0-9a-fA-F]{4,16})");

    /** Funciones Go indexadas por direccion (para nombrar los call targets). */
    private long[] goStarts = new long[0];
    private String[] goNames = new String[0];
    private long[] goEnds = new long[0];

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 1) {
            println("ImportGoRetoolSymbols: uso: import|export|decompile|blocks ...");
            return;
        }
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
        println("GOSYM: mode=" + args[0] + " program=" + currentProgram.getName() +
            " lang=" + currentProgram.getLanguageID() + " imageBase=0x" +
            Long.toHexString(imageBase));

        if ("import".equals(args[0])) {
            doImport(args[1], args[2], args.length > 3 ? args[3] : "");
        }
        else if ("export".equals(args[0])) {
            doExport(args[1], args.length > 2 ? args[2] : "");
        }
        else if ("decompile".equals(args[0])) {
            doDecompile(args[1], args[2], args.length > 3 ? args[3] : "",
                args.length > 4 ? args[4] : "");
        }
        else if ("blocks".equals(args[0])) {
            doBlocks();
        }
        else {
            println("GOSYM: modo desconocido: " + args[0]);
            return;
        }
        println("GOSYM: DONE functions=" + fm.getFunctionCount() +
            " symbols=" + st.getNumSymbols());
    }

    // ------------------------------------------------------------------ direcciones

    /**
     * Resuelve una vaddr del ELF a una direccion de Ghidra.
     *
     * MEDIDO 2026-11-22 (liblanguage_server.so): Ghidra carga este ELF (PIE)
     * con image base 0x100000, de modo que la direccion de una funcion es
     * `vaddr_ELF + imageBase`.  NO se acepta la vaddr sin sumar: la vaddr
     * cruda cae dentro del bloque .rodata de Ghidra (que esta mas abajo en el
     * mapa) y crearia la funcion en el sitio equivocado (0x100000 de
     * diferencia).  Ademas solo se aceptan direcciones en un bloque
     * EJECUTABLE: hay un hueco sin mapear entre .rodata y .text (el ELF lo
     * rellena con relleno) y ahi no se puede crear ninguna funcion.
     */
    private Address addrFor(long off) {
        Address a = safeAddr(off + imageBase);
        if (ejecutable(a)) {
            return a;
        }
        a = safeAddr(off);
        if (ejecutable(a)) {
            return a;
        }
        return null;
    }

    /** Direccion en un bloque inicializado y ejecutable. */
    private boolean ejecutable(Address a) {
        if (a == null) {
            return false;
        }
        try {
            MemoryBlock b = currentProgram.getMemory().getBlock(a);
            return b != null && b.isInitialized() && b.isExecute();
        }
        catch (Throwable t) {
            return false;
        }
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

    // ------------------------------------------------------------------ bloques

    /** Vuelca el mapa de memoria del programa: sirve para verificar el mapeo
     *  entre la vaddr del ELF y la direccion de Ghidra. */
    private void doBlocks() {
        for (MemoryBlock b : currentProgram.getMemory().getBlocks()) {
            println("GOSYM: bloque " + b.getName() + " inicio=0x" +
                Long.toHexString(b.getStart().getOffset()) + " fin=0x" +
                Long.toHexString(b.getEnd().getOffset()) + " tam=0x" +
                Long.toHexString(b.getSize()) + " init=" + b.isInitialized() +
                " ejec=" + b.isExecute() + " escritura=" + b.isWrite() +
                " fuente=" + b.getSourceName());
        }
        println("GOSYM: image_base=0x" + Long.toHexString(imageBase));
    }

    // ------------------------------------------------------------------ import

    private void doImport(String symbolsTsv, String reportTsv, String mapFile) throws Exception {
        long t0 = System.currentTimeMillis();
        long filas = 0, creadas = 0, renombradas = 0, yaThere = 0, desambiguadas = 0;
        long fallos = 0, fuera = 0, sinNombre = 0, desplazadas = 0, saneados = 0;
        long bytesCuerpo = 0;
        Set<String> usados = new HashSet<String>();
        Map<String, Integer> motivos = new HashMap<String, Integer>();
        PrintWriter rep = null;
        if (reportTsv != null && reportTsv.length() > 0) {
            rep = new PrintWriter(new OutputStreamWriter(
                new FileOutputStream(reportTsv), StandardCharsets.UTF_8));
            rep.println("filas\tcreadas\trenombradas\tya_existentes\tdesambiguadas\t" +
                "fallos\tfuera_de_memoria\tsin_nombre\tnombres_saneados\tfunciones_desplazadas\tbytes_cuerpo\tms");
        }
        BufferedReader br = openReader(symbolsTsv);
        String line;
        while ((line = br.readLine()) != null) {
            if (line.isEmpty()) {
                continue;
            }
            String[] p = line.split("\t", 7);
            if (p.length < 4) {
                fallos++;
                continue;
            }
            filas++;
            Long va = parseHex(p[0]);
            Long fin = parseHex(p[3]);
            if (va == null || fin == null || fin <= va) {
                fallos++;
                continue;
            }
            String nombre = p[1];
            String loc = p[2];
            if (nombre == null || nombre.isEmpty()) {
                sinNombre++;
                continue;
            }
            Address a = addrFor(va.longValue());
            if (a == null) {
                fuera++;
                continue;
            }
            long tam = fin.longValue() - va.longValue();
            AddressSet cuerpo = new AddressSet(a, a.add(tam - 1));

            // MEDIDO 2026-11-22: Ghidra rechaza como simbolo cualquier
            // nombre con un espacio (SymbolUtilities.isInvalidChar: c < ' ' o
            // c == ' ').  3.786 de los 161.682 nombres REALES de Go del
            // binario tienen espacios: son las instanciaciones genericas
            // (`slices.partialInsertionSortCmpFunc[go.shape.struct { ... }]`).
            // Para esas se usa el nombre saneado que da el propio Ghidra
            // (replaceInvalidChars) y el nombre REAL queda en el comentario.
            String nombreFinal = SymbolUtilities.replaceInvalidChars(nombre, true);
            if (!nombreFinal.equals(nombre)) {
                saneados++;
            }
            // Desambiguacion: dos funciones de Go pueden llamarse igual
            // (cierres y metodos genericos).  Ghidra exige nombres unicos.
            if (!usados.add(nombreFinal)) {
                nombreFinal = nombreFinal + "@0x" + Long.toHexString(va.longValue());
                desambiguadas++;
            }

            try {
                Function f = fm.getFunctionAt(a);
                if (f != null) {
                    yaThere++;
                    try {
                        f.setBody(cuerpo);
                    }
                    catch (Exception e) {
                        /* el cuerpo ya era valido: se deja */
                    }
                    if (!setNameSafe(f, nombreFinal)) {
                        fallos++;
                        Integer nm = motivos.get("no-se-pudo-renombrar");
                        motivos.put("no-se-pudo-renombrar", nm == null ? 1 : nm + 1);
                        continue;
                    }
                    renombradas++;
                }
                else {
                    // Ghidra NO admite dos funciones que se solapen.  El
                    // cargador de ELF deja funciones de 1 byte (entry,
                    // _INIT_xx, _FINI_xx) EN MEDIO del texto de Go, asi que
                    // hay que apartarlas antes de crear la funcion Go.
                    desplazadas += apartarSolapadas(cuerpo, a);
                    f = fm.createFunction(nombreFinal, a, cuerpo, SourceType.IMPORTED);
                    if (f == null) {
                        // Segundo intento: sin fijar el cuerpo (Ghidra lo calcula).
                        CreateFunctionCmd cfc = new CreateFunctionCmd(a);
                        cfc.applyTo(prog, monitor);
                        f = fm.getFunctionAt(a);
                        if (f == null) {
                            fallos++;
                            Integer nf = motivos.get("create-function-devolvio-nulo");
                            motivos.put("create-function-devolvio-nulo", nf == null ? 1 : nf + 1);
                            continue;
                        }
                        if (!setNameSafe(f, nombreFinal)) {
                            fallos++;
                            Integer nm = motivos.get("no-se-pudo-renombrar");
                            motivos.put("no-se-pudo-renombrar", nm == null ? 1 : nm + 1);
                            continue;
                        }
                    }
                    creadas++;
                }
                bytesCuerpo += tam;
                try {
                    listing.setComment(a, CodeUnit.PLATE_COMMENT,
                        "go-retool: " + nombre +
                        "\n" + (loc == null || loc.isEmpty() || "?".equals(loc) ? "sin fichero" : loc) +
                        "\ntamano=" + tam + " bytes (extent de la pclntab)");
                }
                catch (Exception e) {
                    /* el comentario es informativo */
                }
            }
            catch (Throwable t) {
                fallos++;
                String motivo = t.getClass().getSimpleName() + ": " + shortErr(t.getMessage());
                Integer n = motivos.get(motivo);
                motivos.put(motivo, n == null ? 1 : n + 1);
            }
            if ((filas % 20000) == 0) {
                println("GOSYM: import progress filas=" + filas + " creadas=" + creadas +
                    " renombradas=" + renombradas + " fallos=" + fallos +
                    " desplazadas=" + desplazadas +
                    " ms=" + (System.currentTimeMillis() - t0));
            }
        }
        br.close();
        long ms = System.currentTimeMillis() - t0;
        for (Map.Entry<String, Integer> e : motivos.entrySet()) {
            println("GOSYM: motivo de fallo x" + e.getValue() + " -> " + e.getKey());
        }
        println("GOSYM: import done filas=" + filas + " creadas=" + creadas +
            " renombradas=" + renombradas + " ya_existentes=" + yaThere +
            " desambiguadas=" + desambiguadas + " fallos=" + fallos +
            " fuera=" + fuera + " sin_nombre=" + sinNombre + " saneados=" + saneados +
            " desplazadas=" + desplazadas + " bytes_cuerpo=" + bytesCuerpo + " ms=" + ms);
        if (rep != null) {
            rep.println(filas + "\t" + creadas + "\t" + renombradas + "\t" + yaThere + "\t" +
                desambiguadas + "\t" + fallos + "\t" + fuera + "\t" + sinNombre + "\t" +
                saneados + "\t" + desplazadas + "\t" + bytesCuerpo + "\t" + ms);
            rep.close();
        }
        if (mapFile != null && mapFile.length() > 0) {
            PrintWriter w = new PrintWriter(new OutputStreamWriter(
                new FileOutputStream(mapFile), StandardCharsets.UTF_8));
            w.println("image_base=0x" + Long.toHexString(imageBase));
            w.println("delta=0x" + Long.toHexString(imageBase));
            w.println("funciones=" + fm.getFunctionCount());
            w.close();
        }
    }

    /**
     * Quita las funciones que se solapan con el cuerpo que se quiere crear.
     *
     * El cargador de ELF de Ghidra crea funciones de 1 byte para el simbolo
     * `entry` y para los punteros de _INIT_ARRAY/_FINI_ARRAY, y esos caen EN
     * MEDIO del texto de Go.  Ghidra no admite dos funciones solapadas, asi
     * que se apartan (se borran) y su nombre se conserva como etiqueta.
     *
     * @return numero de funciones apartadas.
     */
    private int apartarSolapadas(AddressSet cuerpo, Address propio) {
        List<Function> victims = new ArrayList<Function>();
        java.util.Iterator<Function> it = fm.getFunctionsOverlapping(cuerpo);
        while (it.hasNext()) {
            Function v = it.next();
            if (!v.getEntryPoint().equals(propio)) {
                victims.add(v);
            }
        }
        int n = 0;
        for (Function v : victims) {
            String nombreViejo = v.getName();
            try {
                if (fm.removeFunction(v.getEntryPoint())) {
                    try {
                        st.createLabel(v.getEntryPoint(), nombreViejo, SourceType.IMPORTED);
                    }
                    catch (Exception e) {
                        /* la etiqueta es un extra: no es critico */
                    }
                    n++;
                }
            }
            catch (Exception e) {
                /* si no se puede apartar, se deja y sera un fallo mas adelante */
            }
        }
        return n;
    }

    /** Cambia el nombre de una funcion; si Ghidra lo rechaza, deja el real en el comentario. */
    private boolean setNameSafe(Function f, String nombre) {
        try {
            if (!f.getName().equals(nombre)) {
                f.setName(nombre, SourceType.IMPORTED);
            }
            return true;
        }
        catch (Exception e) {
            return false;
        }
    }

    // ------------------------------------------------------------------ export

    /** Vuelca las funciones del programa (para comparar con el TSV de go-retool). */
    private void doExport(String outTsv, String mapFile) throws Exception {
        PrintWriter w = new PrintWriter(new OutputStreamWriter(
            new FileOutputStream(outTsv), StandardCharsets.UTF_8));
        w.println("0x<va_elf>\t<nombre>\t<tamano>\t<fuente>");
        long n = 0;
        for (Function f : fm.getFunctions(true)) {
            Address a = f.getEntryPoint();
            long va = a.getOffset() - imageBase;
            String fuente = f.getSymbol().getSource().toString();
            w.println("0x" + Long.toHexString(va) + "\t" + f.getName() + "\t" +
                f.getBody().getNumAddresses() + "\t" + fuente);
            n++;
        }
        // Las funciones EXTERNAS (thunks a librerias del sistema) no salen de
        // getFunctions(): se recorren aparte para que el recuento sea honesto.
        java.util.Iterator<Function> ext = fm.getExternalFunctions();
        long nExt = 0;
        while (ext.hasNext()) {
            Function f = ext.next();
            Address a = f.getEntryPoint();
            w.println("0x" + Long.toHexString(a.getOffset() - imageBase) + "\t" + f.getName() +
                "\t" + f.getBody().getNumAddresses() + "\tEXTERNAL");
            nExt++;
        }
        w.close();
        println("GOSYM: export done funciones=" + n + " externas=" + nExt +
            " total_programa=" + fm.getFunctionCount() + " -> " + outTsv);
        if (mapFile != null && mapFile.length() > 0) {
            PrintWriter m = new PrintWriter(new OutputStreamWriter(
                new FileOutputStream(mapFile), StandardCharsets.UTF_8));
            m.println("image_base=0x" + Long.toHexString(imageBase));
            m.println("funciones_listadas=" + n);
            m.println("funciones_externas=" + nExt);
            m.println("funciones_totales=" + fm.getFunctionCount());
            m.close();
        }
    }

    // ------------------------------------------------------------------ decompile

    private void doDecompile(String listaTsv, String outDir, String reportTsv,
            String symbolsTsv) throws Exception {
        new File(outDir).mkdirs();
        if (symbolsTsv != null && symbolsTsv.length() > 0) {
            cargarSimbolosGo(symbolsTsv);
            println("GOSYM: tabla de funciones Go cargada, entradas=" + goStarts.length);
        }
        DecompInterface ifc = new DecompInterface();
        ifc.setOptions(new DecompileOptions());
        ifc.setSimplificationStyle("decompile");
        ifc.toggleCCode(true);
        ifc.toggleSyntaxTree(true);
        if (!ifc.openProgram(prog)) {
            println("GOSYM: ERROR no se pudo abrir el programa en el decompilador");
            return;
        }
        PrintWriter rep = null;
        if (reportTsv != null && reportTsv.length() > 0) {
            rep = new PrintWriter(new OutputStreamWriter(
                new FileOutputStream(reportTsv), StandardCharsets.UTF_8));
            rep.println("addr\tname\ttag\tghidra_name\tsig\tbody_size\tdec_ok\tdec_err\tout_file");
        }
        BufferedReader br = openReader(listaTsv);
        String line;
        int idx = 0;
        while ((line = br.readLine()) != null) {
            if (line.isEmpty()) {
                continue;
            }
            String[] p = line.split("\t", 3);
            if (p.length < 2) {
                continue;
            }
            idx++;
            Long va = parseHex(p[0]);
            String nombre = p[1];
            String tag = p.length > 2 ? p[2] : "fn";
            Address a = va == null ? null : addrFor(va.longValue());
            String ghidraName = "";
            String sig = "";
            String decErr = "";
            long bodySize = 0;
            boolean decOk = false;
            String outFile = "";
            if (a == null) {
                decErr = "direccion no parseada";
            }
            else {
                try {
                    Function f = fm.getFunctionAt(a);
                    if (f == null) {
                        CreateFunctionCmd cfc = new CreateFunctionCmd(a);
                        cfc.applyTo(prog, monitor);
                        f = fm.getFunctionAt(a);
                    }
                    // MEDIDO 2026-11-22: el desensamblado SIN restringir el
                    // flujo se sale del cuerpo de la funcion y mezcla el
                    // codigo de otras (Go comparte colas y bloques), con lo
                    // que el pseudocodigo sale con cadenas de otras funciones.
                    // Se restringe al cuerpo exacto que da la pclntab.
                    AddressSet cuerpo =
                        (f == null) ? new AddressSet(a, a) : new AddressSet(f.getBody());
                    DisassembleCommand dc = new DisassembleCommand(a,
                        (ghidra.program.model.address.AddressSetView) cuerpo, true);
                    dc.applyTo(prog, monitor);
                    if (f != null) {
                        try {
                            f.setBody(cuerpo);
                        }
                        catch (Exception e) {
                            /* el cuerpo ya era el correcto */
                        }
                    }
                    if (f == null) {
                        decErr = "no se pudo crear la funcion";
                    }
                    else {
                        ghidraName = f.getName();
                        bodySize = f.getBody().getNumAddresses();
                        try {
                            DecompileResults r = ifc.decompileFunction(f, 300, monitor);
                            if (r != null && r.decompileCompleted() && r.getDecompiledFunction() != null) {
                                sig = r.getDecompiledFunction().getSignature();
                                String c = r.getDecompiledFunction().getC();
                                if (c != null && c.length() > 0) {
                                    // Segunda pasada: con las cadenas definidas y
                                    // las etiquetas de los call targets, el
                                    // pseudocodigo es legible.
                                    int nCad = resolverCadenas(c);
                                    int nEti = etiquetarLlamadas(c);
                                    if (nCad > 0 || nEti > 0) {
                                        DecompileResults r2 = ifc.decompileFunction(f, 300, monitor);
                                        if (r2 != null && r2.decompileCompleted() &&
                                            r2.getDecompiledFunction() != null) {
                                            String c2 = r2.getDecompiledFunction().getC();
                                            if (c2 != null && c2.length() > 0) {
                                                c = c2;
                                                sig = r2.getDecompiledFunction().getSignature();
                                            }
                                        }
                                    }
                                    decOk = true;
                                    outFile = writeC(outDir, tag, nombre, f, sig, c);
                                }
                                else {
                                    decErr = "pseudocodigo vacio";
                                }
                            }
                            else if (r != null) {
                                decErr = shortErr(r.getErrorMessage());
                            }
                            else {
                                decErr = "sin resultado";
                            }
                        }
                        catch (Exception e) {
                            decErr = shortErr(e.toString());
                        }
                    }
                }
                catch (Throwable t) {
                    decErr = shortErr(t.toString());
                }
            }
            if (rep != null) {
                rep.println(join("\t", p[0], nombre, tag, ghidraName, sig,
                    Long.toString(bodySize), Boolean.toString(decOk),
                    decErr.replace("\t", " "), outFile));
                rep.flush();
            }
            println("GOSYM: fn " + idx + " " + nombre + " dec=" + decOk +
                (decErr.isEmpty() ? "" : (" err=" + decErr)));
        }
        br.close();
        if (rep != null) {
            rep.close();
        }
        ifc.dispose();
    }

    /**
     * Carga el export de go-retool para poder nombrar los call targets que
     * caen EN MEDIO de una funcion Go (Ghidra no puede crear una funcion
     * dentro del cuerpo de otra, pero si una etiqueta).
     */
    private void cargarSimbolosGo(String path) throws Exception {
        List<long[]> starts = new ArrayList<long[]>();
        List<String> names = new ArrayList<String>();
        BufferedReader br = openReader(path);
        String line;
        while ((line = br.readLine()) != null) {
            if (line.isEmpty()) {
                continue;
            }
            String[] p = line.split("\t", 7);
            if (p.length < 4) {
                continue;
            }
            Long va = parseHex(p[0]);
            Long fin = parseHex(p[3]);
            if (va == null || fin == null) {
                continue;
            }
            starts.add(new long[] { va.longValue(), fin.longValue() });
            names.add(p[1]);
        }
        br.close();
        int n = starts.size();
        goStarts = new long[n];
        goEnds = new long[n];
        goNames = new String[n];
        for (int i = 0; i < n; i++) {
            goStarts[i] = starts.get(i)[0];
            goEnds[i] = starts.get(i)[1];
            goNames[i] = names.get(i);
        }
    }

    /** Nombre Go de la funcion que contiene una direccion, o null. */
    private String nombreContenedor(long off) {
        int lo = 0, hi = goStarts.length - 1, res = -1;
        while (lo <= hi) {
            int mid = (lo + hi) >>> 1;
            if (goStarts[mid] <= off) {
                res = mid;
                lo = mid + 1;
            }
            else {
                hi = mid - 1;
            }
        }
        if (res < 0 || off >= goEnds[res]) {
            return null;
        }
        return goNames[res];
    }

    /**
     * Define como cadena los datos que el decompilador referencia, para que la
     * SEGUNDA pasada los muestre en linea.  Sin esto el pseudocodigo sale con
     * UNK_08eec128 en vez de "admin_settings.json".
     */
    private int resolverCadenas(String pseudocode) {
        int n = 0;
        Matcher m = DAT_RE.matcher(pseudocode);
        Set<Long> vistas = new HashSet<Long>();
        while (m.find()) {
            Long v = parseHex(m.group(2));
            if (v == null || !vistas.add(v.longValue())) {
                continue;
            }
            Address a = safeAddr(v.longValue());
            if (a == null || !initialized(a)) {
                continue;
            }
            String texto = leerImprimible(a);
            if (texto == null || texto.length() < 4) {
                continue;
            }
            try {
                if (listing.getDefinedDataAt(a) != null) {
                    continue;
                }
                int len = texto.length();
                if (len > 160) {
                    len = 160;
                }
                listing.createData(a, ghidra.program.model.data.StringDataType.dataType, len);
                n++;
            }
            catch (Exception e) {
                /* la cadena se deja como estaba */
            }
        }
        return n;
    }

    /** Pone etiqueta a los call targets que caen dentro de una funcion Go. */
    private int etiquetarLlamadas(String pseudocode) {
        int n = 0;
        Matcher m = CALL_RE.matcher(pseudocode);
        Set<Long> vistas = new HashSet<Long>();
        while (m.find()) {
            Long v = parseHex(m.group(1));
            if (v == null || !vistas.add(v.longValue())) {
                continue;
            }
            long off = v.longValue() - imageBase;
            String padre = nombreContenedor(off);
            if (padre == null) {
                continue;
            }
            Address a = safeAddr(v.longValue());
            if (a == null || !initialized(a)) {
                continue;
            }
            long ini = 0;
            int lo = 0, hi = goStarts.length - 1;
            while (lo <= hi) {
                int mid = (lo + hi) >>> 1;
                if (goStarts[mid] <= off) {
                    ini = goStarts[mid];
                    lo = mid + 1;
                }
                else {
                    hi = mid - 1;
                }
            }
            String etiqueta = padre + "+0x" + Long.toHexString(off - ini);
            try {
                if (st.getPrimarySymbol(a) != null) {
                    continue;
                }
                st.createLabel(a, etiqueta, SourceType.USER_DEFINED);
                n++;
            }
            catch (Exception e) {
                /* la etiqueta es un extra */
            }
        }
        return n;
    }

    /** Lee la cadena imprimible que empieza en una direccion. */
    private String leerImprimible(Address a) {
        try {
            byte[] buf = new byte[200];
            int leidos = currentProgram.getMemory().getBytes(a, buf, 0, buf.length);
            if (leidos <= 0) {
                return null;
            }
            StringBuilder sb = new StringBuilder();
            for (int i = 0; i < leidos; i++) {
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

    private String writeC(String outDir, String tag, String nombre, Function f, String sig,
            String c) throws Exception {
        File out = new File(outDir, sanea(tag) + "__" + sanea(nombre) + ".c");
        PrintWriter w = new PrintWriter(new OutputStreamWriter(
            new FileOutputStream(out), StandardCharsets.UTF_8));
        w.println("// " + nombre);
        w.println("// VA ELF: 0x" + Long.toHexString(f.getEntryPoint().getOffset() - imageBase));
        w.println("// tamano del cuerpo: " + f.getBody().getNumAddresses() + " bytes");
        w.println("// firma: " + sig);
        w.println();
        w.println(c);
        w.close();
        return out.getPath();
    }

    // ------------------------------------------------------------------ utilidades

    private static String sanea(String s) {
        StringBuilder sb = new StringBuilder();
        for (char ch : s.toCharArray()) {
            boolean ok = Character.isLetterOrDigit(ch) || ch == '_' || ch == '.' || ch == '-';
            sb.append(ok ? ch : '_');
        }
        String r = sb.toString();
        return r.length() > 120 ? r.substring(0, 120) : r;
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
        return t.length() > 200 ? t.substring(0, 200) : t;
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
