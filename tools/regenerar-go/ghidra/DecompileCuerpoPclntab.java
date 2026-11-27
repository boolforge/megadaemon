// DecompileCuerpoPclntab.java — 4.º motor: Ghidra con el CUERPO FORZADO
// desde el pclntab, en vez de la suposición de Ghidra.
//
// @category RE
//
// MOTIVO (medido el 2026-11-27, ver docs/RE_FUNCIONES_IRRECONSTRUIBLES.md):
// el camino de decompilación de `ImportGoRetoolSymbols.java` restringe el
// desensamblado a `f.getBody()` — la extensión que Ghidra ADIVINA.  Para las
// 103 funciones que ningun motor conseguia, esa adivinacion es erroneous:
// Ghidra se atribuia un cuerpo 6,54 veces mayor que el real (mediana), y el
// decompilador acababa con "Cannot properly adjust input varnodes" o con
// "Overlapping input varnodes" (literal: se ha tragado el codigo de las
// funciones siguientes).
//
// El camino de IMPORTacion del mismo script ya hace bien (linea 261):
//     AddressSet cuerpo = new AddressSet(a, a.add(tam - 1));
// con `tam` = siguiente VA - VA, segun el pclntab.  Este script hace lo mismo
// en el camino de DECOMPILACION.
//
// uso:
//   analyzeHeadless <proj> <name> -process <program> -noanalysis -max-cpu 1 \
//     -scriptPath <dir> -postScript DecompileCuerpoPclntab.java \
//       <lista.tsv> <outDir> <report.tsv> [maxSegundos]
//
// lista.tsv: addr \t nombre \t tag \t tamano
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileOptions;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.address.AddressSet;
import ghidra.program.model.address.AddressSetView;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;
import ghidra.program.model.listing.Listing;
import ghidra.program.model.symbol.SourceType;
import ghidra.program.model.symbol.SymbolTable;
import ghidra.app.cmd.function.CreateFunctionCmd;
import ghidra.app.cmd.disassemble.DisassembleCommand;

import java.io.BufferedReader;
import java.io.File;
import java.io.FileOutputStream;
import java.io.FileReader;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;

public class DecompileCuerpoPclntab extends GhidraScript {

    private SymbolTable st;
    private FunctionManager fm;
    private Listing listing;
    private long imageBase = 0;

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 3) {
            println("uso: DecompileCuerpoPclntab <lista.tsv> <outDir> <report.tsv>");
            return;
        }
        String listaTsv = args[0];
        String outDir = args[1];
        String reportTsv = args[2];
        int maxSeg = args.length > 3 ? Integer.parseInt(args[3]) : 300;

        st = currentProgram.getSymbolTable();
        fm = currentProgram.getFunctionManager();
        listing = currentProgram.getListing();
        try {
            imageBase = currentProgram.getImageBase().getOffset();
        }
        catch (Exception e) {
            imageBase = 0;
        }
        new File(outDir).mkdirs();
        println("GOSYM: mode=pclntab-body program=" + currentProgram.getName() +
            " imageBase=0x" + Long.toHexString(imageBase));

        DecompInterface ifc = new DecompInterface();
        ifc.setOptions(new DecompileOptions());
        ifc.setSimplificationStyle("decompile");
        ifc.toggleCCode(true);
        ifc.toggleSyntaxTree(true);
        if (!ifc.openProgram(currentProgram)) {
            println("GOSYM: ERROR no se pudo abrir el programa en el decompilador");
            return;
        }

        PrintWriter rep = new PrintWriter(new OutputStreamWriter(
            new FileOutputStream(reportTsv), StandardCharsets.UTF_8));
        rep.println("addr\tname\tbody_pclntab\tbody_ghidra\tsig\tdec_ok\tdec_err\tout_file");

        BufferedReader br = new BufferedReader(new FileReader(listaTsv));
        String line;
        int ok = 0, fallo = 0, n = 0;
        while ((line = br.readLine()) != null) {
            if (line.isEmpty()) {
                continue;
            }
            String[] p = line.split("\t", 4);
            if (p.length < 4) {
                continue;
            }
            n++;
            long va = parseHex(p[0]);
            String nombre = p[1];
            String tag = p[2];
            long tam = parseHex(p[3]);
            Address a = addrFor(va);
            String sig = "";
            String decErr = "";
            long bodyGhidra = 0;
            boolean decOk = false;
            String outFile = "";
            if (a == null) {
                decErr = "direccion no resoluble";
            }
            else {
                try {
                    Function f = fm.getFunctionAt(a);
                    if (f == null) {
                        CreateFunctionCmd cfc = new CreateFunctionCmd(a);
                        cfc.applyTo(currentProgram, monitor);
                        f = fm.getFunctionAt(a);
                    }
                    if (f == null) {
                        decErr = "no se pudo crear la funcion";
                    }
                    else {
                        // ---- AQUI ESTA LA DIFERENCIA con el script original:
                        // el cuerpo NO es el de Ghidra, es el del pclntab.
                        AddressSet cuerpo = new AddressSet(a, a.add(tam - 1));
                        apartarSolapadas(cuerpo, a);
                        DisassembleCommand dc = new DisassembleCommand(a,
                            (AddressSetView) cuerpo, true);
                        dc.applyTo(currentProgram, monitor);
                        try {
                            f.setBody(cuerpo);
                        }
                        catch (Exception e) {
                            decErr = "setBody: " + e.toString();
                        }
                        bodyGhidra = f.getBody().getNumAddresses();
                        DecompileResults r = ifc.decompileFunction(f, maxSeg, monitor);
                        if (r != null && r.decompileCompleted() &&
                                r.getDecompiledFunction() != null) {
                            sig = r.getDecompiledFunction().getSignature();
                            String c = r.getDecompiledFunction().getC();
                            if (c != null && c.length() > 0) {
                                decOk = true;
                                outFile = writeC(outDir, tag, nombre, f, sig, c, tam);
                            }
                            else {
                                decErr = "pseudocodigo vacio";
                            }
                        }
                        else if (r != null) {
                            decErr = r.getErrorMessage();
                        }
                        else {
                            decErr = "sin resultado";
                        }
                    }
                }
                catch (Throwable t) {
                    decErr = String.valueOf(t);
                }
            }
            if (decOk) {
                ok++;
            }
            else {
                fallo++;
            }
            rep.println("0x" + Long.toHexString(va) + "\t" + nombre + "\t" + tam + "\t" +
                bodyGhidra + "\t" + sig.replace("\t", " ") + "\t" + decOk + "\t" +
                decErr.replace("\t", " ") + "\t" + outFile);
            rep.flush();
        }
        br.close();
        rep.close();
        println("GOSYM: DONE total=" + n + " ok=" + ok + " fallo=" + fallo);
    }

    private int apartarSolapadas(AddressSet cuerpo, Address propio) {
        List<Function> victims = new ArrayList<Function>();
        Iterator<Function> it = fm.getFunctionsOverlapping(cuerpo);
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
                        /* la etiqueta es un extra */
                    }
                    n++;
                }
            }
            catch (Exception e) {
                /* si no se puede apartar, se deja */
            }
        }
        return n;
    }

    private long parseHex(String s) {
        if (s == null) {
            return -1;
        }
        String t = s.trim();
        if (t.startsWith("0x") || t.startsWith("0X")) {
            t = t.substring(2);
        }
        try {
            return Long.parseLong(t, 16);
        }
        catch (Exception e) {
            return -1;
        }
    }

    private Address addrFor(long off) {
        Address a = safe(off + imageBase);
        if (ejecutable(a)) {
            return a;
        }
        a = safe(off);
        if (ejecutable(a)) {
            return a;
        }
        return null;
    }

    private Address safe(long v) {
        try {
            return toAddr(v);
        }
        catch (Throwable t) {
            return null;
        }
    }

    private boolean ejecutable(Address a) {
        if (a == null) {
            return false;
        }
        try {
            return currentProgram.getMemory().getBlock(a) != null &&
                currentProgram.getMemory().getBlock(a).isInitialized() &&
                currentProgram.getMemory().getBlock(a).isExecute();
        }
        catch (Throwable t) {
            return false;
        }
    }

    private String writeC(String outDir, String tag, String nombre, Function f, String sig,
            String c, long tam) throws Exception {
        File out = new File(outDir, sanea(tag) + "__" + sanea(nombre) + ".c");
        PrintWriter w = new PrintWriter(new OutputStreamWriter(
            new FileOutputStream(out), StandardCharsets.UTF_8));
        w.println("// " + nombre);
        w.println("// VA ELF: 0x" + Long.toHexString(f.getEntryPoint().getOffset() - imageBase));
        w.println("// cuerpo forzado desde el pclntab: " + tam + " bytes");
        w.println("// firma: " + sig);
        w.println();
        w.println(c);
        w.close();
        return out.getPath();
    }

    private static String sanea(String s) {
        StringBuilder sb = new StringBuilder();
        for (char ch : s.toCharArray()) {
            boolean ok = Character.isLetterOrDigit(ch) || ch == '_' || ch == '.' || ch == '-';
            sb.append(ok ? ch : '_');
        }
        return sb.toString();
    }
}
