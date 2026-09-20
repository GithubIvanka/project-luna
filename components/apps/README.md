# Applications

`components/apps/` — место для обычных пользовательских приложений Luna.

Здесь не должны появляться:
- PID 1/system runtime components;
- system managers;
- external provider adapters.

Niri, Noctalia, Ghostty, Yazi и другие desktop providers в исходном внешнем виде не являются Luna applications: их runtime packages живут в DATA/provider layer.
