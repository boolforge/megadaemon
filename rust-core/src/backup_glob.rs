//! Comodín simple (`*` = cualquier secuencia, `?` = un carácter).
//!
//! POR QUÉ ESTE MÓDULO
//! -------------------
//! `glob_match` estaba dentro de `backup_engine`, que arrastra `battery_saver`
//! y `task_scheduler` (y con ellos, media docena de módulos más). El motor de
//! backup (`backup_apps`) sólo necesita esta función ⇒ se extrae aquí para
//! que el binario `md-backup` se pueda compilar **sólido** con el motor de
//! backup (sin el resto del crate). `backup_engine` la re-exporta, así que su
//! API pública no cambia.

/// Comodín simple: `*` = cualquier secuencia, `?` = un carácter.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    // Caso rápido: sin comodines ⇒ subcadena (comportamiento histórico).
    if !pattern.contains('*') && !pattern.contains('?') {
        return text.contains(pattern);
    }
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (usize::MAX, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = pi;
            mark = ti;
            pi += 1;
        } else if star != usize::MAX {
            pi = star + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sin_comodines_es_subcadena() {
        assert!(glob_match("abc", "xxabcxx"));
        assert!(!glob_match("abc", "xxabxx"));
    }

    #[test]
    fn el_asterisco_cubre_cualquier_secuencia() {
        assert!(glob_match("*.tmp", "fichero.tmp"));
        assert!(glob_match("/data/*/cache", "/data/user/0/cache"));
        assert!(!glob_match("*.tmp", "fichero.txt"));
    }

    #[test]
    fn el_interrogante_cubre_un_caracter() {
        assert!(glob_match("a?c", "abc"));
        assert!(!glob_match("a?c", "abbc"));
    }

    #[test]
    fn el_retroceso_del_asterisco_funciona() {
        assert!(glob_match("*ab", "xxabyy"));
        assert!(glob_match("a*b*c", "axxbyyc"));
    }
}
