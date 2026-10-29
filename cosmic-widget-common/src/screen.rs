//! Logiczny rozmiar ekranu — potrzebny, żeby przeciągany widget nie
//! uciekł poza krawędź.
//!
//! Skąd to w ogóle problem: powierzchnia `zwlr_layer_shell_v1` nie ma
//! współrzędnych, tylko kotwicę i margines. Kompozytor sam przycina
//! pozycję do ekranu, więc przy krawędzi powierzchnia PRZESTAJE jechać —
//! a pętla przeciągania (`WidgetPos::on_moved`) mierzy odchylenie kursora
//! względem powierzchni i dalej dolicza margines. Efekt: margines rośnie
//! w nieskończoność (widziane 2026-08-26: `margin_x = 12691` przy ekranie
//! 1920 px), widget przykleja się do krawędzi i „nie da się go przeciągnąć
//! z powrotem” — trzeba by ciągnąć myszką tyle samo w drugą stronę.
//! Lekarstwo: znać rozmiar ekranu i twardo ograniczyć margines.
//!
//! Rozmiar bierzemy z `cosmic-randr list --kdl` (część COSMIC-a, więc jest
//! wszędzie tam, gdzie te widgety mają sens). Gdy się nie uda — `None`
//! i wtedy po prostu nie ograniczamy, czyli zachowanie jak dawniej.

/// Logiczny (po uwzględnieniu skalowania) rozmiar pierwszego włączonego
/// ekranu w px.
#[must_use]
pub fn size() -> Option<(u32, u32)> {
    let out = std::process::Command::new("cosmic-randr")
        .args(["list", "--kdl"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse(&String::from_utf8_lossy(&out.stdout))
}

/// Wydzielone z `size()`, żeby dało się to przetestować bez kompozytora.
fn parse(kdl: &str) -> Option<(u32, u32)> {
    let mut in_enabled = false;
    let mut scale = 1.0_f64;

    for line in kdl.lines() {
        let t = line.trim();
        if t.starts_with("output ") {
            // Kolejny blok — interesuje nas pierwszy włączony.
            in_enabled = t.contains("enabled=#true");
            scale = 1.0;
            continue;
        }
        if !in_enabled {
            continue;
        }
        if let Some(rest) = t.strip_prefix("scale ") {
            if let Ok(v) = rest.trim().parse::<f64>() {
                if v > 0.0 {
                    scale = v;
                }
            }
        } else if t.starts_with("mode ") && t.contains("current=#true") {
            let mut parts = t.split_whitespace().skip(1);
            let w: f64 = parts.next()?.parse().ok()?;
            let h: f64 = parts.next()?.parse().ok()?;
            return Some(((w / scale) as u32, (h / scale) as u32));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::parse;

    const KDL: &str = r#"output "eDP-1" enabled=#true {
  description make="BOE" model="0x07BB"
  position 0 0
  scale 1.00
  modes {
    mode 1920 1080 59999 current=#true preferred=#true
    mode 1680 1050 59999
  }
}
"#;

    #[test]
    fn czyta_biezacy_tryb() {
        assert_eq!(parse(KDL), Some((1920, 1080)));
    }

    #[test]
    fn uwzglednia_skalowanie() {
        let kdl = KDL.replace("scale 1.00", "scale 2.00");
        assert_eq!(parse(&kdl), Some((960, 540)));
    }

    /// Wyłączony ekran przed włączonym — bierzemy ten włączony.
    #[test]
    fn pomija_wylaczone_ekrany() {
        let kdl = format!(
            "output \"HDMI-1\" enabled=#false {{\n  scale 1.00\n  modes {{\n    mode 800 600 60000 current=#true\n  }}\n}}\n{KDL}"
        );
        assert_eq!(parse(&kdl), Some((1920, 1080)));
    }

    #[test]
    fn brak_danych_to_none() {
        assert_eq!(parse("nic tu nie ma"), None);
    }
}
