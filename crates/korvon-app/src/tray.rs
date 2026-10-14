//! Ícone na área de notificação (ícones ocultos) enquanto a gravação está em andamento.

use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

/// Bolinha vermelha 32×32.
fn dot() -> Option<Icon> {
    const N: u32 = 32;
    let mut rgba = Vec::with_capacity((N * N * 4) as usize);
    for y in 0..N {
        for x in 0..N {
            let (dx, dy) = (x as f32 + 0.5 - 16.0, y as f32 + 0.5 - 16.0);
            let d = (dx * dx + dy * dy).sqrt();
            let alpha = ((15.0 - d) * 255.0).clamp(0.0, 255.0) as u8;
            let edge = d > 12.0;
            let (r, g, b) = if edge { (0xc6, 0x28, 0x28) } else { (0xef, 0x53, 0x50) };
            rgba.extend_from_slice(&[r, g, b, alpha]);
        }
    }
    Icon::from_rgba(rgba, N, N).ok()
}

pub fn show() -> Option<TrayIcon> {
    TrayIconBuilder::new()
        .with_tooltip("Korvon Macro — gravando… (ESC para parar)")
        .with_icon(dot()?)
        .build()
        .ok()
}
