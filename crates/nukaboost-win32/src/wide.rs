//! Utilitários de conversão para strings UTF-16 usadas pelas APIs Win32.

use windows::core::PCWSTR;

/// Converte `text` em um `Vec<u16>` terminado em nulo, adequado para
/// construir uma [`PCWSTR`] cuja vida útil é controlada pelo chamador (o
/// vetor precisa sobreviver por toda a chamada Win32 que consumir o ponteiro).
pub fn to_wide_null(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Constrói uma [`PCWSTR`] a partir de um identificador numérico de
/// recurso, reproduzindo a macro `MAKEINTRESOURCEW` do Win32: se o valor
/// coubesse em 16 bits e for passado como ponteiro, o Windows o interpreta
/// como um ID em vez de um endereço de string.
pub const fn resource_id(id: u16) -> PCWSTR {
    PCWSTR(id as usize as *const u16)
}

/// Copia `text` para um buffer UTF-16 de tamanho fixo (como `szTip` de
/// `NOTIFYICONDATAW`), truncando se necessário e sempre terminando em nulo.
///
/// `dst` deve ter pelo menos 1 elemento; o texto ocupa no máximo
/// `dst.len() - 1` posições para deixar espaço para o terminador nulo.
pub fn copy_to_fixed_buffer(dst: &mut [u16], text: &str) {
    debug_assert!(!dst.is_empty(), "destination buffer must not be empty");

    for slot in dst.iter_mut() {
        *slot = 0;
    }

    let max_chars = dst.len() - 1;
    for (slot, unit) in dst.iter_mut().zip(text.encode_utf16().take(max_chars)) {
        *slot = unit;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_wide_null_appends_terminator() {
        let wide = to_wide_null("ok");
        assert_eq!(wide, vec![b'o' as u16, b'k' as u16, 0]);
    }

    #[test]
    fn copy_to_fixed_buffer_truncates_and_terminates() {
        let mut buf = [0u16; 4];
        copy_to_fixed_buffer(&mut buf, "hello");
        // Apenas 3 caracteres cabem (índice 3 reservado para o nulo).
        assert_eq!(buf, [b'h' as u16, b'e' as u16, b'l' as u16, 0]);
    }

    #[test]
    fn copy_to_fixed_buffer_clears_previous_content() {
        let mut buf = [b'x' as u16; 4];
        copy_to_fixed_buffer(&mut buf, "ok");
        assert_eq!(buf, [b'o' as u16, b'k' as u16, 0, 0]);
    }
}
