//! Палитра Alpha применяется только к настоящему клиенту Files.
pub const CSS: &str = r#"
window.luna-files { background: #111d30; color: #e9f1ff; font-size: 13px; }
.luna-files label, .luna-files button, .luna-files entry { font-family: "Adwaita Sans"; }
.luna-files headerbar, .luna-files .toolbar { background: #18263d; border-bottom: 1px solid #31435f; }
.luna-files button { background: #18263d; color: #e9f1ff; border: 1px solid #31435f; border-radius: 7px; padding: 7px 10px; }
.luna-files button:hover { background: #25486e; }
.luna-files button:disabled { color: #7185a4; }
.luna-files button:focus-visible, .luna-files entry:focus { outline: 2px solid #74d5f3; outline-offset: 1px; }
.luna-files entry { background: #080f20; color: #e9f1ff; border: 1px solid #31435f; border-radius: 8px; padding: 6px; }
.luna-files list { background: #111d30; }
.luna-files row { color: #e9f1ff; border-radius: 6px; padding: 8px 10px; margin: 2px 8px; }
.luna-files row label { color: #e9f1ff; }
.luna-files row:selected { background: #25486e; color: #e9f1ff; }
.luna-files row image { color: #74d5f3; }
.luna-files .sidebar { background: #18263d; padding: 12px; border-right: 1px solid #31435f; }
.luna-files .sidebar button { border: none; background: transparent; }
.luna-files .sidebar button:hover { background: #25486e; }
.luna-files .status { color: #a0b2cf; padding: 8px 12px; border-top: 1px solid #31435f; }
"#;
