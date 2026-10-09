//! Utilidades de texto con índices de *char* (los que usa el cursor de egui).

/// Índice de char → (línea, columna), ambos desde 1.
pub fn char_to_line_col(text: &str, char_idx: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for c in text.chars().take(char_idx) {
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// (línea, columna) desde 1 → índice de char. Se ajusta al final de la línea
/// o del texto si se pasa.
pub fn line_col_to_char(text: &str, line: usize, col: usize) -> usize {
    let mut idx = 0;
    let mut cur_line = 1;
    let mut chars = text.chars().peekable();
    while cur_line < line {
        match chars.next() {
            Some('\n') => {
                cur_line += 1;
                idx += 1;
            }
            Some(_) => idx += 1,
            None => return idx,
        }
    }
    for _ in 1..col.max(1) {
        match chars.peek() {
            Some('\n') | None => break,
            Some(_) => {
                chars.next();
                idx += 1;
            }
        }
    }
    idx
}

/// Índice de char → byte. Si se pasa del final, devuelve `text.len()`.
pub fn char_to_byte(text: &str, char_idx: usize) -> usize {
    text.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(text.len())
}

/// Todas las coincidencias de `query` como rangos de char `[start, end)`.
/// Sin distinción de mayúsculas compara char a char en minúscula (1:1, así
/// los índices siguen valiendo para el texto original). Tope de 10 000.
pub fn find_all(text: &str, query: &str, match_case: bool) -> Vec<(usize, usize)> {
    const CAP: usize = 10_000;
    if query.is_empty() {
        return Vec::new();
    }
    let fold = |c: char| {
        if match_case {
            c
        } else {
            c.to_lowercase().next().unwrap_or(c)
        }
    };
    let q: Vec<char> = query.chars().map(fold).collect();
    let t: Vec<char> = text.chars().map(fold).collect();
    let mut out = Vec::new();
    if q.len() > t.len() {
        return out;
    }
    let mut i = 0;
    while i + q.len() <= t.len() && out.len() < CAP {
        if t[i..i + q.len()] == q[..] {
            out.push((i, i + q.len()));
            i += q.len();
        } else {
            i += 1;
        }
    }
    out
}

pub fn content_stats(text: &str) -> (usize, usize) {
    (text.lines().count().max(1), text.split_whitespace().count())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_col_roundtrip() {
        let t = "ab\ncdé\n\nf";
        for i in 0..=t.chars().count() {
            let (l, c) = char_to_line_col(t, i);
            assert_eq!(line_col_to_char(t, l, c), i, "idx {i} → {l}:{c}");
        }
        assert_eq!(line_col_to_char(t, 2, 99), 6);
        assert_eq!(line_col_to_char(t, 99, 1), t.chars().count());
    }

    #[test]
    fn find_case() {
        let t = "Hola hola HOLA";
        assert_eq!(find_all(t, "hola", true), vec![(5, 9)]);
        assert_eq!(find_all(t, "hola", false).len(), 3);
        assert!(find_all(t, "", false).is_empty());
    }
}
