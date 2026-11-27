//CodexSimbolos.java
//Aplica al programa los simbolos REALES del `.debug` de Codex y decompila el
//subconjunto pedido. Motor sancionado: Ghidra headless.
//
// MEDIDO 2026-11-27: el binario `codex-app-server-x86_64-unknown-linux-musl`
// (249.083.328 B) va stripped (0 nombres de funcion). Su debug file
// `codex-app-server.debug` (1.085.115.680 B, CRC32 ad295a15 = el que declara
// el propio binario) trae la symtab completa: 616.013 simbolos, 356.133
// funciones. Este script mete esos nombres en Ghidra y decompila.
//
// uso:
//   -process <bin> -scriptPath <dir> -postScript CodexSimbolos.java \
//       <simbolos.tsv> <salidaDir> <maxFuncs> <filtroCrates> [soloDecompilar]
//   <simbolos.tsv>: addr(vaddr) \t bytes \t tipo \t crate \t legible \t mangled
//   <filtroCrates>: coma-separado; "" = todos; "codex" = prefijos
// @category megadaemon
import java.io.BufferedReader;
import java.io.File;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.OutputStreamWriter;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;

import ghidra.app.cmd.disassemble.DisassembleCommand;
import ghidra.app.cmd.function.CreateFunctionCmd;
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileOptions;
import ghidra.app.decompiler.DecompileResults;
import ghidra.app.script.GhidraScript;
import ghidra.program.model.address.Address;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionManager;
import ghidra.program.model.listing.Program;
import ghidra.program.model.symbol.SourceType;
import ghidra.util.task.ConsoleTaskMonitor;

public class CodexSimbolos extends GhidraScript {

    private long imageBase = 0;

    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        if (args.length < 4) {
            println("CodexSimbolos: se esperan >=4 argumentos, llegaron " + args.length);
            return;
        }
        String tsv = args[0];
        String outDir = args[1];
        int maxFuncs = Integer.parseInt(args[2]);
        String filtro = args.length > 3 ? args[3] : "";
        boolean soloTexto = args.length > 4 && args[4].equals("soloTexto");

        Program prog = currentProgram;
        FunctionManager fm = prog.getFunctionManager();
        try {
            imageBase = prog.getImageBase().getOffset();
        }
        catch (Exception e) {
            imageBase = 0;
        }
        new File(outDir).mkdirs();
        println("CODEXSIMBOLOS programa=" + prog.getName()
            + " lang=" + prog.getLanguageID()
            + " imageBase=0x" + Long.toHexString(imageBase)
            + " filtro='" + filtro + "' maxFuncs=" + maxFuncs);

        List<String[]> filas = new ArrayList<String[]>();
        BufferedReader br = new BufferedReader(
            new java.io.InputStreamReader(new FileInputStream(tsv), StandardCharsets.UTF_8),
            1 << 20);
        String cab = br.readLine();
        String linea;
        while ((linea = br.readLine()) != null) {
            String[] c = linea.split("\t");
            if (c.length < 5) {
                continue;
            }
            if (!"T".equals(c[2]) && !"t".equals(c[2])) {
                continue;  // solo funciones
            }
            if (!filtro.isEmpty()) {
                boolean ok = false;
                for (String p : filtro.split(",")) {
                    if (!p.isEmpty() && c[3].startsWith(p)) {
                        ok = true;
                        break;
                    }
                }
                if (!ok) {
                    continue;
                }
            }
            filas.add(c);
            if (filas.size() >= maxFuncs) {
                break;
            }
        }
        br.close();
        println("CODEXSIMBOLOS funciones candidatas=" + filas.size());

        DecompInterface decomp = null;
        if (!soloTexto) {
            decomp = new DecompInterface();
            DecompileOptions opt = new DecompileOptions();
            decomp.setOptions(opt);
            decomp.openProgram(prog);
        }

        PrintWriter indice = new PrintWriter(new OutputStreamWriter(
            new FileOutputStream(new File(outDir, "indice.tsv"), true), StandardCharsets.UTF_8));
        indice.println("addr_ghidra\taddr_vaddr\tbytes\tcrate\tnombre\tpseudocodio_bytes");

        int creadas = 0, decompiladas = 0, fallidas = 0;
        for (String[] c : filas) {
            if (monitor.isCancelled()) {
                break;
            }
            long vaddr;
            try {
                vaddr = Long.parseLong(c[0].trim(), 16);
            }
            catch (Exception e) {
                continue;
            }
            long tam = 0;
            try {
                tam = Long.parseLong(c[1].trim(), 16);
            }
            catch (Exception e) {
                tam = 0;
            }
            Address addr = prog.getAddressFactory().getDefaultAddressSpace()
                .getAddress(vaddr + imageBase);
            if (addr == null) {
                continue;
            }
            String nombre = c[4];
            if (nombre.length() > 180) {
                nombre = nombre.substring(0, 180);
            }
            Function f = fm.getFunctionAt(addr);
            if (f == null) {
                // desensambla en la direccion si no hay codigo
                try {
                    // MEDIDO 2026-11-27: con follow=true el desensamblado de
                    // las funciones Rust recorre decenas de KB y tarda ~45 s
                    // cada una; con follow=false va mucho mas rapido.
                    DisassembleCommand dc = new DisassembleCommand(addr, null, false);
                    dc.applyTo(prog, monitor);
                }
                catch (Exception e) {
                    // se ignora: puede que ya este desensamblado
                }
                try {
                    CreateFunctionCmd cfc = new CreateFunctionCmd(addr);
                    cfc.applyTo(prog, monitor);
                }
                catch (Exception e) {
                    // puede que la funcion ya exista o el rango sea invalido
                }
                f = fm.getFunctionAt(addr);
            }
            if (f == null) {
                continue;
            }
            try {
                f.setName(nombre, SourceType.IMPORTED);
            }
            catch (Exception e) {
                try {
                    f.setName(nombre.substring(0, 60) + "_" + Long.toHexString(vaddr),
                        SourceType.IMPORTED);
                }
                catch (Exception e2) {
                    // nombre no aplicable
                }
            }
            if (f.getBody() == null || f.getBody().getNumAddresses() < 2) {
                try {
                    f.setBody(null);
                    CreateFunctionCmd cfc = new CreateFunctionCmd(addr);
                    cfc.applyTo(prog, monitor);
                }
                catch (Exception e) {
                    // no se puede fijar el cuerpo
                }
            }
            creadas++;

            int nbytes = 0;
            if (decomp != null) {
                try {
                    DecompileResults res = decomp.decompileFunction(f, 20, monitor);
                    if (res != null && res.decompileCompleted()
                            && res.getDecompiledFunction() != null) {
                        String pc = res.getDecompiledFunction().getC();
                        StringBuilder sb = new StringBuilder();
                        sb.append("// addr=0x").append(Long.toHexString(vaddr + imageBase));
                        sb.append("  vaddr=0x").append(Long.toHexString(vaddr));
                        sb.append("  bytes=").append(tam);
                        sb.append("  crate=").append(c[3]);
                        sb.append("\n// ").append(nombre).append("\n");
                        sb.append(pc == null ? "" : pc);
                        sb.append("\n");
                        String sane = c[3].replaceAll("[^A-Za-z0-9_.-]", "_");
                        String fichero = new File(outDir,
                            Long.toHexString(vaddr) + "_" + sane + "_"
                            + Integer.toHexString(nombre.hashCode()) + ".c").getPath();
                        PrintWriter w = new PrintWriter(new OutputStreamWriter(
                            new FileOutputStream(fichero), StandardCharsets.UTF_8));
                        w.print(sb.toString());
                        w.close();
                        nbytes = sb.length();
                        decompiladas++;
                    }
                    else {
                        fallidas++;
                    }
                }
                catch (Exception e) {
                    fallidas++;
                }
            }
            indice.println("0x" + Long.toHexString(vaddr + imageBase) + "\t0x"
                + Long.toHexString(vaddr) + "\t" + tam + "\t" + c[3] + "\t" + nombre
                + "\t" + nbytes);
        }
        indice.close();
        if (decomp != null) {
            decomp.dispose();
        }
        println("CODEXSIMBOLOS creadas=" + creadas + " decompiladas=" + decompiladas
            + " fallidas=" + fallidas + " -> " + outDir);
    }
}
