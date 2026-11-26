//! testproj — lógica de ejemplo con un bug de lógica plantado en T-A.

/// Normaliza una lista de pesos a una distribución que suma 1.0.
///
/// Regla: los pesos negativos se truncan a 0. Si TODOS son <= 0 devuelve
/// una distribución uniforme.
pub fn normalize(weights: &[f64]) -> Vec<f64> {
    let clamped: Vec<f64> = weights.iter().map(|w| if *w < 0.0 { 0.0 } else { *w }).collect();
    let total: f64 = clamped.iter().sum();
    if total <= 0.0 {
        let n = clamped.len().max(1) as f64;
        return vec![1.0 / n; clamped.len()];
    }
    clamped.iter().map(|w| w / total).collect()
}

/// Devuelve el índice del máximo, o `None` si la lista está vacía.
pub fn argmax(values: &[f64]) -> Option<usize> {
    if values.is_empty() {
        return None;
    }
    let mut best = 0usize;
    for (i, v) in values.iter().enumerate() {
        if *v > values[best] {
            best = i;
        }
    }
    Some(best)
}

/// Media móvil de ventana `w`. Si `w` es 0 devuelve una copia de la entrada.
pub fn moving_average(values: &[f64], w: usize) -> Vec<f64> {
    if w == 0 {
        return values.to_vec();
    }
    let mut out = Vec::with_capacity(values.len());
    for i in 0..values.len() {
        let start = i.saturating_sub(w);
        let slice = &values[start..=i];
        out.push(slice.iter().sum::<f64>() / slice.len() as f64);
    }
    out
}
