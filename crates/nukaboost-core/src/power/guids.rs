//! GUIDs de configurações de energia usadas pelo NukaBoost.
//!
//! Constantes transcritas literalmente da seção 6 do plano do produto.
//! Cada valor `0x________` usa `_` no lugar dos traços da forma canônica
//! `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`, o que torna a transcrição
//! mecânica e fácil de conferir visualmente contra a documentação da
//! Microsoft. **Não alterar estes valores** sem conferir contra
//! [a documentação de configurações de energia](https://learn.microsoft.com/en-us/windows-hardware/customize/power-settings/power-settings).

use windows::core::GUID;

/// Subgrupo `SUB_BUTTONS` (Botões de energia e tampa).
pub const SUB_BUTTONS: GUID = GUID::from_u128(0x4f971e89_eebd_4455_a8de_9e59040e7347);

/// Configuração `LIDACTION` (ação ao fechar a tampa) dentro de `SUB_BUTTONS`.
pub const LIDACTION: GUID = GUID::from_u128(0x5ca83367_6e45_459f_a27b_476b1d01c936);

/// Subgrupo `SUB_SLEEP` (Suspensão).
pub const SUB_SLEEP: GUID = GUID::from_u128(0x238c9fa8_0aad_41ed_83f4_97be242c8f20);

/// Configuração `STANDBYIDLE` (tempo de espera antes de suspender por
/// inatividade) dentro de `SUB_SLEEP`. `0` significa "nunca suspender".
pub const STANDBYIDLE: GUID = GUID::from_u128(0x29f6c1db_86da_48c5_9fdb_f2b67b1f44da);

/// Configuração `SYSTEMREQUIRED` (permitir que aplicativos impeçam a
/// suspensão automática) dentro de `SUB_SLEEP`.
pub const SYSTEMREQUIRED: GUID = GUID::from_u128(0xa4b195f5_8225_47d8_8012_9d41369786e2);

/// Subgrupo `SUB_BATTERY` (Bateria).
pub const SUB_BATTERY: GUID = GUID::from_u128(0xe73a048d_bf27_4f12_9731_8b2076e8891f);

/// Configuração `BATACTIONLOW` (ação ao atingir o nível de bateria baixa,
/// **não** o nível crítico) dentro de `SUB_BATTERY`.
pub const BATACTIONLOW: GUID = GUID::from_u128(0xd8742dcb_3e6a_4b3c_b3fe_374623cdcf06);

/// Formata um GUID na forma canônica `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`
/// (minúsculo), usada ao persistir GUIDs de plano no journal de recuperação
/// — `windows::core::GUID` não implementa `serde`, então o journal guarda
/// planos como texto e converte nas bordas com esta função e [`parse_guid`].
pub fn format_guid(guid: GUID) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        guid.data1,
        guid.data2,
        guid.data3,
        guid.data4[0],
        guid.data4[1],
        guid.data4[2],
        guid.data4[3],
        guid.data4[4],
        guid.data4[5],
        guid.data4[6],
        guid.data4[7],
    )
}

/// Erro retornado por [`parse_guid`] quando a string de entrada não segue o
/// formato canônico `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`.
#[derive(Debug, Clone, thiserror::Error)]
#[error("'{0}' is not a valid GUID in the xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx format")]
pub struct InvalidGuid(pub String);

/// Interpreta uma string no formato canônico `xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`
/// de volta para um [`GUID`]. Inverso de [`format_guid`].
pub fn parse_guid(text: &str) -> Result<GUID, InvalidGuid> {
    let invalid = || InvalidGuid(text.to_string());

    let parts: Vec<&str> = text.split('-').collect();
    let [p1, p2, p3, p4, p5] = parts.as_slice() else {
        return Err(invalid());
    };
    if p1.len() != 8 || p2.len() != 4 || p3.len() != 4 || p4.len() != 4 || p5.len() != 12 {
        return Err(invalid());
    }

    let data1 = u32::from_str_radix(p1, 16).map_err(|_| invalid())?;
    let data2 = u16::from_str_radix(p2, 16).map_err(|_| invalid())?;
    let data3 = u16::from_str_radix(p3, 16).map_err(|_| invalid())?;

    let tail = format!("{p4}{p5}");
    let mut data4 = [0u8; 8];
    for (i, byte) in data4.iter_mut().enumerate() {
        let hex_byte = tail.get(i * 2..i * 2 + 2).ok_or_else(invalid)?;
        *byte = u8::from_str_radix(hex_byte, 16).map_err(|_| invalid())?;
    }

    Ok(GUID::from_values(data1, data2, data3, data4))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Confere cada GUID contra sua representação textual canônica, para
    /// detectar qualquer erro de transcrição nos literais acima.
    #[test]
    fn guids_match_canonical_string_form() {
        let cases: &[(GUID, &str)] = &[
            (SUB_BUTTONS, "4f971e89-eebd-4455-a8de-9e59040e7347"),
            (LIDACTION, "5ca83367-6e45-459f-a27b-476b1d01c936"),
            (SUB_SLEEP, "238c9fa8-0aad-41ed-83f4-97be242c8f20"),
            (STANDBYIDLE, "29f6c1db-86da-48c5-9fdb-f2b67b1f44da"),
            (SYSTEMREQUIRED, "a4b195f5-8225-47d8-8012-9d41369786e2"),
            (SUB_BATTERY, "e73a048d-bf27-4f12-9731-8b2076e8891f"),
            (BATACTIONLOW, "d8742dcb-3e6a-4b3c-b3fe-374623cdcf06"),
        ];

        for (guid, text) in cases {
            assert_eq!(format_guid(*guid), *text);
            assert_eq!(parse_guid(text).unwrap(), *guid);
        }
    }

    #[test]
    fn parse_rejects_malformed_input() {
        assert!(parse_guid("not-a-guid").is_err());
        assert!(parse_guid("4f971e89-eebd-4455-a8de-9e59040e73").is_err());
        assert!(parse_guid("4f971e89eebd4455a8de9e59040e7347").is_err());
    }
}
